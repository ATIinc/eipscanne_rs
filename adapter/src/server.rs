//! The server: one task and one encapsulation session per TCP connection, handing Send RR Data to
//! unconnected and Send Unit Data to connected explicit messaging.

use std::io::ErrorKind;
use std::net::IpAddr;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tracing::Instrument;

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::ENCAPSULATION_PROTOCOL_VERSION;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};
use crate::explicit::connected::ExplicitConnection;
use crate::explicit::connected::send_unit_data::handle_send_unit_data;
use crate::explicit::unconnected::handle_send_rr_data;
use crate::implicit::ImplicitDevice;
use crate::implicit::message_router::ImplicitMessageRouter;
use crate::message_router::MessageRouter;
use crate::packet::{read_packet, write_packet};

/// Serves every connecting scanner with `message_router` until accepting fails; dropping it closes
/// every connection
pub async fn serve(message_router: Arc<impl MessageRouter>, listener: TcpListener) -> Result<()> {
    let mut sessions = JoinSet::new();
    let mut next_session_handle: CipUdint = 1;
    loop {
        let (stream, peer) = listener.accept().await?;
        let session_handle = next_session_handle;
        next_session_handle = next_session_handle.wrapping_add(1);
        let message_router = Arc::clone(&message_router);
        sessions.spawn(
            async move {
                if let Err(error) =
                    serve_session(message_router, stream, session_handle, peer.ip()).await
                {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("session", %peer)),
        );
        while sessions.try_join_next().is_some() {}
    }
}

/// [`serve`], plus one class 1 connection at a time on an I/O socket at the listener's address
pub async fn serve_implicit(device: Arc<impl ImplicitDevice>, listener: TcpListener) -> Result<()> {
    let message_router = ImplicitMessageRouter::bind(device, listener.local_addr()?.ip()).await?;
    serve(Arc::new(message_router), listener).await
}

/// Answers the packets of the scanner at `originator_ip` as session `session_handle` until it
/// unregisters or closes the stream; its class 3 connection ends with it
#[tracing::instrument(skip(message_router, stream))]
async fn serve_session(
    message_router: Arc<impl MessageRouter>,
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    originator_ip: IpAddr,
) -> Result<()> {
    let mut registered = false;
    let mut explicit_connection: Option<ExplicitConnection> = None;
    loop {
        let (header, command_specific_data) = match read_packet(&mut stream).await {
            Ok(packet) => (packet.header, Some(packet.command_specific_data)),
            Err(Error::Undecodable { header, error }) => {
                tracing::warn!(%error, "packet did not decode");
                (header, None)
            }
            Err(Error::Io(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                tracing::info!("scanner closed the stream");
                return Ok(());
            }
            Err(error) => return Err(error),
        };

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
                let reply = handle_register_session(
                    &header,
                    command_specific_data.as_ref(),
                    session_handle,
                );
                if reply.header.status_code == EncapsStatusCode::Success {
                    tracing::info!("session registered");
                    registered = true;
                }
                reply
            }
            EnIpCommand::SendRrData => {
                if registered && header.session_handle == session_handle {
                    handle_send_rr_data(
                        message_router.as_ref(),
                        &mut explicit_connection,
                        &header,
                        command_specific_data.as_ref(),
                        originator_ip,
                    )
                    .unwrap_or_else(|status_code| error_reply(&header, status_code))
                } else {
                    error_reply(&header, EncapsStatusCode::InvalidSessionHandle)
                }
            }
            EnIpCommand::SendUnitData => {
                if registered && header.session_handle == session_handle {
                    // A discarded request has no reply
                    let Some(reply) = handle_send_unit_data(
                        message_router.as_ref(),
                        &mut explicit_connection,
                        &header,
                        command_specific_data.as_ref(),
                        originator_ip,
                    ) else {
                        continue;
                    };
                    reply
                } else {
                    error_reply(&header, EncapsStatusCode::InvalidSessionHandle)
                }
            }
            _ => error_reply(&header, EncapsStatusCode::UnsupportedCommand),
        };
        reply.header.sender_context = header.sender_context;
        write_packet(&mut stream, &reply).await?;
    }
}

/// The Register Session reply granting `session_handle`, or refusing another protocol version with ours
fn handle_register_session(
    header: &EncapsulationHeader,
    command_specific_data: Option<&CommandSpecificData>,
    session_handle: CipUdint,
) -> EnIpPacket {
    let Some(CommandSpecificData::RegisterSession(register_data)) = command_specific_data else {
        return error_reply(header, EncapsStatusCode::InvalidFormatOrData);
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

/// The reply refusing the packet with `header`
fn error_reply(header: &EncapsulationHeader, status_code: EncapsStatusCode) -> EnIpPacket {
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
