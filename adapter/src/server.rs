//! The explicit messaging server: one task and one session per connection.

use std::io::ErrorKind;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tracing::Instrument;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::ServiceContainer;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::{ENCAPSULATION_PROTOCOL_VERSION, NO_ENCAPSULATION_TIMEOUT};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::device::Device;
use crate::error::{Error, Result};
use crate::explicit::handle_request;
use crate::packet::{message_router_request, read_packet, write_packet};

/// Serves every connecting scanner until accepting fails; dropping it closes every connection
pub async fn serve(listener: TcpListener, device: Arc<dyn Device>) -> Result<()> {
    let mut connections = JoinSet::new();
    let mut next_session_handle: CipUdint = 1;
    loop {
        let (stream, peer) = listener.accept().await?;
        let session_handle = next_session_handle;
        next_session_handle = next_session_handle.wrapping_add(1);
        let device = Arc::clone(&device);
        connections.spawn(
            async move {
                if let Err(error) = serve_connection(stream, session_handle, device.as_ref()).await
                {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("connection", %peer)),
        );
        while connections.try_join_next().is_some() {}
    }
}

/// Answers one scanner's packets as session `session_handle` until it unregisters or closes the stream
#[tracing::instrument(skip(stream, device))]
pub async fn serve_connection(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    device: &dyn Device,
) -> Result<()> {
    let mut registered = false;
    loop {
        let packet = match read_packet(&mut stream).await {
            Ok(packet) => packet,
            // Answered as a packet without data
            Err(Error::Undecodable { header, error }) => {
                tracing::warn!(%error, "packet did not decode");
                EnIpPacket {
                    header,
                    command_specific_data: CommandSpecificData::NoData,
                }
            }
            Err(Error::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                tracing::info!("scanner closed the stream");
                return Ok(());
            }
            Err(error) => return Err(error),
        };

        let header = &packet.header;
        let mut reply = match header.command {
            // NOP has no reply
            EnIpCommand::NOP => continue,
            // Unregister Session has no reply
            EnIpCommand::UnRegisterSession => {
                tracing::info!("session unregistered");
                return Ok(());
            }
            // A request with a Status has no reply
            _ if header.status_code != EncapsStatusCode::Success => {
                tracing::warn!(command = ?header.command, status = ?header.status_code, "ignored a request with a Status");
                continue;
            }
            EnIpCommand::RegisterSession => {
                let reply = handle_register_session(&packet, session_handle);
                if reply.header.status_code == EncapsStatusCode::Success {
                    tracing::info!("session registered");
                    registered = true;
                }
                reply
            }
            EnIpCommand::SendRrData => {
                if registered && header.session_handle == session_handle {
                    handle_send_rr_data(&packet, session_handle, device)
                } else {
                    refusal(header, EncapsStatusCode::InvalidSessionHandle)
                }
            }
            _ => refusal(header, EncapsStatusCode::UnsupportedCommand),
        };
        reply.header.sender_context = header.sender_context;
        write_packet(&mut stream, &reply).await?;
    }
}

/// The reply refusing the packet with `header`
fn refusal(header: &EncapsulationHeader, status_code: EncapsStatusCode) -> EnIpPacket {
    tracing::warn!(command = ?header.command, status = ?status_code, "refused a packet");
    EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            status_code,
            ..*header
        },
        command_specific_data: CommandSpecificData::NoData,
    }
}

/// The Register Session reply granting `session_handle`, or refusing another protocol version with ours
fn handle_register_session(packet: &EnIpPacket, session_handle: CipUdint) -> EnIpPacket {
    let CommandSpecificData::RegisterSession(register_data) = &packet.command_specific_data else {
        return refusal(&packet.header, EncapsStatusCode::InvalidFormatOrData);
    };
    let mut reply = EnIpPacket::new_registration();
    if register_data.protocol_version != ENCAPSULATION_PROTOCOL_VERSION {
        tracing::warn!(
            protocol_version = register_data.protocol_version,
            "refused Register Session"
        );
        reply.header.status_code = EncapsStatusCode::UnsupportedProtocolVersion;
        return reply;
    }
    reply.header.session_handle = session_handle;
    reply
}

/// The Send RR Data reply carrying the Message Router's response to the request inside
fn handle_send_rr_data(
    packet: &EnIpPacket,
    session_handle: CipUdint,
    device: &dyn Device,
) -> EnIpPacket {
    let Some(request) = message_router_request(packet) else {
        return refusal(&packet.header, EncapsStatusCode::InvalidFormatOrData);
    };
    let service = request.service_container.service();
    let (status, data) = match handle_request(request, device) {
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
    EnIpPacket::new_send_rr_data(session_handle, NO_ENCAPSULATION_TIMEOUT, response)
}
