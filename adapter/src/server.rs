//! The server: one task and one encapsulation session per TCP connection, handing Send RR Data to
//! unconnected explicit messaging.

use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::Instrument;

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::ENCAPSULATION_PROTOCOL_VERSION;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};
use crate::explicit::unconnected::handle_send_rr_data;
use crate::message_router::{ExplicitOptions, MessageRouter};
use crate::packet::{error_reply, read_packet, write_packet};

/// Serves every connecting scanner with `message_router` until accepting fails; dropping it closes
/// every connection
pub async fn serve(
    message_router: Arc<impl MessageRouter>,
    listener: TcpListener,
    options: ExplicitOptions,
) -> Result<()> {
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
                    serve_session(message_router, options, stream, session_handle).await
                {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("session", %peer)),
        );
        while sessions.try_join_next().is_some() {}
    }
}

/// Answers the packets of a scanner as session `session_handle` until it unregisters, closes the
/// stream or has no encapsulation activity for the `options` inactivity timeout
#[tracing::instrument(skip(message_router, options, stream))]
async fn serve_session(
    message_router: Arc<impl MessageRouter>,
    options: ExplicitOptions,
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
) -> Result<()> {
    let inactivity_timeout = options
        .encapsulation_inactivity_timeout
        .unwrap_or(Duration::MAX);
    let mut registered = false;
    loop {
        let Ok(packet) = timeout(inactivity_timeout, read_packet(&mut stream)).await else {
            tracing::warn!("the session timed out: no encapsulation activity in time");
            return Ok(());
        };
        let (header, command_specific_data) = match packet {
            Ok(packet) => (packet.header, packet.command_specific_data),
            // Answered as a packet without data
            Err(Error::Undecodable { header, error }) => {
                tracing::warn!(%error, "packet did not decode");
                (header, CommandSpecificData::NoData)
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
                let reply =
                    handle_register_session(session_handle, &header, &command_specific_data);
                if reply.header.status_code == EncapsStatusCode::Success {
                    tracing::info!("session registered");
                    registered = true;
                }
                reply
            }
            // Send RR Data needs the registered session
            EnIpCommand::SendRrData if !registered || header.session_handle != session_handle => {
                error_reply(&header, EncapsStatusCode::InvalidSessionHandle)
            }
            EnIpCommand::SendRrData => {
                handle_send_rr_data(message_router.as_ref(), &header, &command_specific_data)
            }
            _ => error_reply(&header, EncapsStatusCode::UnsupportedCommand),
        };
        reply.header.sender_context = header.sender_context;
        write_packet(&mut stream, &reply).await?;
    }
}

/// The Register Session reply granting `session_handle`, or refusing another protocol version with ours
fn handle_register_session(
    session_handle: CipUdint,
    header: &EncapsulationHeader,
    command_specific_data: &CommandSpecificData,
) -> EnIpPacket {
    let CommandSpecificData::RegisterSession(register_data) = command_specific_data else {
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
