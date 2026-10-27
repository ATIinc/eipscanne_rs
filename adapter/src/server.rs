//! The explicit messaging server: one task and one session per connection.

use std::io::{Cursor, ErrorKind};
use std::sync::Arc;

use binrw::BinRead;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tracing::Instrument;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::{ENCAPSULATION_PROTOCOL_VERSION, NO_ENCAPSULATION_TIMEOUT};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};
use crate::handler::Handler;
use crate::packet::{read_frame, request_of, write_header, write_packet};

/// Serves every connecting scanner until accepting fails; dropping it closes every connection
pub async fn serve(listener: TcpListener, handler: Arc<dyn Handler>) -> Result<()> {
    let mut connections = JoinSet::new();
    let mut next_session_handle: CipUdint = 1;
    loop {
        let (stream, peer) = listener.accept().await?;
        let session_handle = next_session_handle;
        next_session_handle = next_session_handle.wrapping_add(1);
        let handler = Arc::clone(&handler);
        connections.spawn(
            async move {
                if let Err(error) = serve_stream(stream, session_handle, handler.as_ref()).await {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("connection", %peer)),
        );
        while connections.try_join_next().is_some() {}
    }
}

/// Answers one scanner's packets as session `session_handle` until it unregisters or closes the stream
#[tracing::instrument(skip(stream, handler))]
pub async fn serve_stream(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    handler: &dyn Handler,
) -> Result<()> {
    let mut registered = false;
    loop {
        let (header, bytes) = match read_frame(&mut stream).await {
            Err(Error::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                tracing::info!("scanner closed the stream");
                return Ok(());
            }
            frame => frame?,
        };

        let reply = match header.command {
            // Unregister Session has no reply
            EnIpCommand::UnRegisterSession => {
                tracing::info!("session unregistered");
                return Ok(());
            }
            EnIpCommand::RegisterSession => {
                let reply = handle_register_session(&bytes, session_handle);
                if reply.is_ok() {
                    tracing::info!("session registered");
                    registered = true;
                }
                reply
            }
            EnIpCommand::SendRrData if registered && header.session_handle == session_handle => {
                handle_send_rr_data(&bytes, session_handle, handler)
            }
            EnIpCommand::SendRrData => Err(EncapsStatusCode::InvalidSessionHandle),
            _ => Err(EncapsStatusCode::UnsupportedCommand),
        };

        match reply {
            Ok(mut reply) => {
                reply.header.sender_context = header.sender_context;
                write_packet(&mut stream, &reply).await?;
            }
            Err(status_code) => {
                tracing::warn!(command = ?header.command, status = ?status_code, "refused a packet");
                let reply = EncapsulationHeader {
                    length: None,
                    status_code,
                    ..header
                };
                write_header(&mut stream, &reply).await?;
            }
        }
    }
}

/// The Register Session reply granting `session_handle`
fn handle_register_session(
    bytes: &[u8],
    session_handle: CipUdint,
) -> std::result::Result<EnIpPacket, EncapsStatusCode> {
    let packet = EnIpPacket::read(&mut Cursor::new(bytes)).map_err(|error| {
        tracing::warn!(%error, "packet did not decode");
        EncapsStatusCode::InvalidFormatOrData
    })?;
    let CommandSpecificData::RegisterSession(register_data) = &packet.command_specific_data else {
        return Err(EncapsStatusCode::InvalidFormatOrData);
    };
    if register_data.protocol_version != ENCAPSULATION_PROTOCOL_VERSION {
        return Err(EncapsStatusCode::UnsupportedProtocolVersion);
    }
    let mut reply = EnIpPacket::new_registration();
    reply.header.session_handle = session_handle;
    Ok(reply)
}

/// The Send RR Data reply carrying the Message Router's response to the request inside
fn handle_send_rr_data(
    bytes: &[u8],
    session_handle: CipUdint,
    handler: &dyn Handler,
) -> std::result::Result<EnIpPacket, EncapsStatusCode> {
    let packet = EnIpPacket::read(&mut Cursor::new(bytes)).map_err(|error| {
        tracing::warn!(%error, "packet did not decode");
        EncapsStatusCode::InvalidFormatOrData
    })?;
    let request = request_of(&packet).ok_or(EncapsStatusCode::InvalidFormatOrData)?;
    let service = request.service_container.service();
    let (status, data) = match route(service, request, handler) {
        Ok(data) => (ResponseStatusCode::Success, data),
        Err(status) => (status, CipDataOpt::Raw(vec![])),
    };
    let response = MessageRouterResponse {
        service_container: ServiceContainer::new_response(service),
        response_data: ResponseData {
            status,
            additional_status_size: 0,
            additional_status: vec![],
            data,
        },
    };
    Ok(EnIpPacket::new_send_rr_data(
        session_handle,
        NO_ENCAPSULATION_TIMEOUT,
        response,
    ))
}

/// Hands a Get or Set to the handler; Single services need an attribute, All services none
fn route(
    service: ServiceCode,
    request: &MessageRouterRequest,
    handler: &dyn Handler,
) -> std::result::Result<CipDataOpt, ResponseStatusCode> {
    let Some((class, instance, attribute)) = logical_ids(&request.request_data.cip_path) else {
        tracing::warn!(
            ?service,
            "refused a request whose path is not class, instance and attribute"
        );
        return Err(ResponseStatusCode::PathSegmentError);
    };
    let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };

    let reply = match (service, attribute) {
        (ServiceCode::GetAttributeAll, None) => handler.handle_get_attributes_all(class, instance),
        (ServiceCode::GetAttributeSingle, Some(attribute)) => {
            handler.handle_get_attribute_single(class, instance, attribute)
        }
        (ServiceCode::SetAttributeAll, None) => handler
            .handle_set_attributes_all(class, instance, data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (ServiceCode::SetAttributeSingle, Some(attribute)) => handler
            .handle_set_attribute_single(class, instance, attribute, data)
            .map(|()| CipDataOpt::Raw(vec![])),
        (
            ServiceCode::GetAttributeAll
            | ServiceCode::GetAttributeSingle
            | ServiceCode::SetAttributeAll
            | ServiceCode::SetAttributeSingle,
            _,
        ) => Err(ResponseStatusCode::PathDestinationUnknown),
        _ => Err(ResponseStatusCode::ServiceNotSupported),
    };

    match &reply {
        Ok(_) => tracing::debug!(?service, class, instance, ?attribute, "answered a request"),
        Err(status) => {
            tracing::warn!(
                ?service,
                class,
                instance,
                ?attribute,
                ?status,
                "refused a request"
            )
        }
    }
    reply
}

/// The class, instance and optional attribute of a logical path
fn logical_ids(path: &CipPath) -> Option<(u16, u16, Option<u16>)> {
    match path.segments.as_slice() {
        [class, instance] => Some((
            value_of(class, LogicalSegmentType::ClassId)?,
            value_of(instance, LogicalSegmentType::InstanceId)?,
            None,
        )),
        [class, instance, attribute] => Some((
            value_of(class, LogicalSegmentType::ClassId)?,
            value_of(instance, LogicalSegmentType::InstanceId)?,
            Some(value_of(attribute, LogicalSegmentType::AttributeId)?),
        )),
        _ => None,
    }
}

/// The value of a segment of `logical_segment_type`
fn value_of(segment: &LogicalPathSegment, logical_segment_type: LogicalSegmentType) -> Option<u16> {
    if segment.path_definition.logical_segment_type() != logical_segment_type {
        return None;
    }
    match segment.data {
        PathData::FormatAsU8(value) => Some(value.into()),
        PathData::FormatAsU16(value) => Some(value),
        PathData::ElectronicKey(_) => None,
    }
}
