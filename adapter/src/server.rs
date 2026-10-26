//! The explicit messaging server: one task and one session per connection.

use std::io::{Cursor, ErrorKind};
use std::sync::Arc;

use binrw::BinRead;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType, PathData};
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::{ENCAPSULATION_PROTOCOL_VERSION, NO_ENCAPSULATION_TIMEOUT};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};
use crate::handler::Handler;
use crate::packet::{read_frame, request_of, write_header, write_packet};
use crate::reply;

/// Serves every connecting scanner until accepting fails; dropping it closes every connection
pub async fn serve(listener: TcpListener, handler: Arc<dyn Handler>) -> Result<()> {
    let mut connections = JoinSet::new();
    let mut next_session_handle: CipUdint = 1;
    loop {
        let (stream, _) = listener.accept().await?;
        let session_handle = next_session_handle;
        next_session_handle = next_session_handle.wrapping_add(1);
        let handler = Arc::clone(&handler);
        connections.spawn(async move {
            // A connection's end has no one to report to
            let _ = serve_stream(stream, session_handle, handler.as_ref()).await;
        });
        while connections.try_join_next().is_some() {}
    }
}

/// Answers one scanner's packets as session `session_handle` until it unregisters or closes the stream
pub async fn serve_stream(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    handler: &dyn Handler,
) -> Result<()> {
    let mut registered = false;
    loop {
        let (header, bytes) = match read_frame(&mut stream).await {
            Err(Error::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            frame => frame?,
        };

        // Unregister Session has no reply
        if header.command == EnIpCommand::UnRegisterSession {
            return Ok(());
        }

        match answer(&header, &bytes, session_handle, &mut registered, handler) {
            Ok(mut reply) => {
                reply.header.sender_context = header.sender_context;
                write_packet(&mut stream, &reply).await?;
            }
            Err(status_code) => {
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

/// The reply to one packet, or the encapsulation status that refuses it
fn answer(
    header: &EncapsulationHeader,
    bytes: &[u8],
    session_handle: CipUdint,
    registered: &mut bool,
    handler: &dyn Handler,
) -> std::result::Result<EnIpPacket, EncapsStatusCode> {
    if !matches!(
        header.command,
        EnIpCommand::RegisterSession | EnIpCommand::SendRrData
    ) {
        return Err(EncapsStatusCode::UnsupportedCommand);
    }
    let packet = EnIpPacket::read(&mut Cursor::new(bytes))
        .map_err(|_| EncapsStatusCode::InvalidFormatOrData)?;

    match &packet.command_specific_data {
        CommandSpecificData::RegisterSession(register_data) => {
            if register_data.protocol_version != ENCAPSULATION_PROTOCOL_VERSION {
                return Err(EncapsStatusCode::UnsupportedProtocolVersion);
            }
            *registered = true;
            let mut reply = EnIpPacket::new_registration();
            reply.header.session_handle = session_handle;
            Ok(reply)
        }
        CommandSpecificData::SendRrData(_) => {
            if !*registered || header.session_handle != session_handle {
                return Err(EncapsStatusCode::InvalidSessionHandle);
            }
            let request = request_of(&packet).ok_or(EncapsStatusCode::InvalidFormatOrData)?;
            Ok(EnIpPacket::new_send_rr_data(
                session_handle,
                NO_ENCAPSULATION_TIMEOUT,
                respond(request, handler),
            ))
        }
        _ => Err(EncapsStatusCode::UnsupportedCommand),
    }
}

/// The Message Router's reply to `request`
fn respond(request: &MessageRouterRequest, handler: &dyn Handler) -> MessageRouterResponse {
    let service = request.service_container.service();
    match handle(service, request, handler) {
        Ok(data) => reply::success(service, data),
        Err(status) => reply::error(service, status),
    }
}

/// Hands a Get or Set to the handler; Single services need an attribute, All services none
fn handle(
    service: ServiceCode,
    request: &MessageRouterRequest,
    handler: &dyn Handler,
) -> std::result::Result<CipDataOpt, ResponseStatusCode> {
    let (class, instance, attribute) =
        logical_ids(&request.request_data.cip_path).ok_or(ResponseStatusCode::PathSegmentError)?;

    match (service, attribute) {
        (ServiceCode::GetAttributeAll, None) | (ServiceCode::GetAttributeSingle, Some(_)) => {
            handler.handle_get_attribute(class, instance, attribute)
        }
        (ServiceCode::SetAttributeAll, None) | (ServiceCode::SetAttributeSingle, Some(_)) => {
            // Data read from the wire is always raw
            let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
                return Err(ResponseStatusCode::InvalidParameterValue);
            };
            handler.handle_set_attribute(class, instance, attribute, data)?;
            Ok(CipDataOpt::Raw(vec![]))
        }
        (
            ServiceCode::GetAttributeAll
            | ServiceCode::GetAttributeSingle
            | ServiceCode::SetAttributeAll
            | ServiceCode::SetAttributeSingle,
            _,
        ) => Err(ResponseStatusCode::PathDestinationUnknown),
        _ => Err(ResponseStatusCode::ServiceNotSupported),
    }
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
