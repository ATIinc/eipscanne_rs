//! The server: one task and one encapsulation session per TCP connection, handing Send RR Data to
//! unconnected and Send Unit Data to connected explicit messaging.

use std::io::ErrorKind;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout};
use tracing::Instrument;

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::ENCAPSULATION_PROTOCOL_VERSION;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};
use crate::explicit::connected::Class3Connection;
use crate::explicit::connected::send_unit_data::handle_send_unit_data;
use crate::explicit::unconnected::handle_send_rr_data;
use crate::implicit::message_router::ImplicitMessageRouter;
use crate::implicit::{ImplicitDevice, ImplicitOptions};
use crate::message_router::{ExplicitOptions, MessageRouter};
use crate::packet::{error_reply, read_packet, write_packet};

/// [`serve`], plus the class 1 connections `implicit_options` allows on an I/O socket at the
/// listener's address
pub async fn serve_implicit(
    device: Arc<impl ImplicitDevice>,
    listener: TcpListener,
    explicit_options: ExplicitOptions,
    implicit_options: ImplicitOptions,
) -> Result<()> {
    let message_router =
        ImplicitMessageRouter::bind(device, listener.local_addr()?.ip(), implicit_options).await?;
    serve(Arc::new(message_router), listener, explicit_options).await
}

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
                    serve_session(message_router, options, stream, session_handle, peer.ip()).await
                {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("session", %peer)),
        );
        while sessions.try_join_next().is_some() {}
    }
}

/// Answers the packets of the scanner at `originator_ip` as session `session_handle` until it
/// unregisters, closes the stream or has no encapsulation activity for the `options` inactivity
/// timeout; its class 3 connection ends with it
#[tracing::instrument(skip(message_router, options, stream))]
async fn serve_session(
    message_router: Arc<impl MessageRouter>,
    options: ExplicitOptions,
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    originator_ip: IpAddr,
) -> Result<()> {
    let mut registered = false;
    let mut class_3_connection = Class3Connection {
        accepted: options.accepts_explicit_connections,
        established: None,
    };
    loop {
        let Ok(packet) = timeout(
            session_inactivity_timeout(options, &class_3_connection),
            read_packet(&mut stream),
        )
        .await
        else {
            tracing::warn!("the session timed out: no encapsulation activity in time");
            return Ok(());
        };
        // The session keeps its class 3 connection only until it times out
        class_3_connection
            .established
            .take_if(|connection| connection.deadline() <= Instant::now());
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
            // Send RR Data and Send Unit Data need the registered session
            EnIpCommand::SendRrData | EnIpCommand::SendUnitData
                if !registered || header.session_handle != session_handle =>
            {
                error_reply(&header, EncapsStatusCode::InvalidSessionHandle)
            }
            EnIpCommand::SendRrData => handle_send_rr_data(
                message_router.as_ref(),
                &header,
                &command_specific_data,
                originator_ip,
                &mut class_3_connection,
            ),
            EnIpCommand::SendUnitData => {
                let Some(reply) = handle_send_unit_data(
                    message_router.as_ref(),
                    &header,
                    &command_specific_data,
                    originator_ip,
                    &mut class_3_connection,
                ) else {
                    // A discarded request has no reply
                    continue;
                };
                reply
            }
            _ => error_reply(&header, EncapsStatusCode::UnsupportedCommand),
        };
        reply.header.sender_context = header.sender_context;
        write_packet(&mut stream, &reply).await?;
    }
}

/// How long the session waits for a packet: the `options` inactivity timeout, after the time
/// `class_3_connection` has left
fn session_inactivity_timeout(
    options: ExplicitOptions,
    class_3_connection: &Class3Connection,
) -> Duration {
    // An open class 3 connection is encapsulation activity until it times out
    let class_3_time_left = match &class_3_connection.established {
        Some(connection) => connection
            .deadline()
            .saturating_duration_since(Instant::now()),
        None => Duration::ZERO,
    };
    let inactivity_timeout = options
        .encapsulation_inactivity_timeout
        .unwrap_or(Duration::MAX);
    class_3_time_left.saturating_add(inactivity_timeout)
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
