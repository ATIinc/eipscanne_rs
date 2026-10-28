//! The server: one task and one session per TCP connection; the Message Router in `explicit`
//! answers the requests.

use std::io::ErrorKind;
use std::net::IpAddr;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, UdpSocket};
use tokio::task::JoinSet;
use tracing::Instrument;

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::ENCAPSULATION_PROTOCOL_VERSION;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::device::Device;
use crate::error::{Error, Result};
use crate::explicit::handle_unconnected_message;
use crate::implicit::connection::ConnectionManager;
use crate::packet::{read_packet, write_packet};

/// Serves every connecting scanner until accepting fails, and one class 1 connection at a time
/// when the device does implicit messaging; dropping it closes every connection
pub async fn serve(device: Arc<dyn Device>, listener: TcpListener) -> Result<()> {
    // The I/O socket shares the listener's address
    let connection_manager = match device.as_implicit_device() {
        Some(implicit_device) => {
            let ip = listener.local_addr()?.ip();
            let io_socket = UdpSocket::bind((ip, implicit_device.io_port())).await?;
            tracing::info!(io = %io_socket.local_addr()?, "implicit messaging");
            Some(Arc::new(ConnectionManager::new(io_socket)))
        }
        None => None,
    };
    let mut connections = JoinSet::new();
    let mut next_session_handle: CipUdint = 1;
    loop {
        let (stream, peer) = listener.accept().await?;
        let session_handle = next_session_handle;
        next_session_handle = next_session_handle.wrapping_add(1);
        let device = Arc::clone(&device);
        let connection_manager = connection_manager.clone();
        connections.spawn(
            async move {
                if let Err(error) = serve_connection(
                    device,
                    stream,
                    session_handle,
                    peer.ip(),
                    connection_manager,
                )
                .await
                {
                    tracing::warn!(%error, "connection ended");
                }
            }
            .instrument(tracing::info_span!("connection", %peer)),
        );
        while connections.try_join_next().is_some() {}
    }
}

/// Answers the packets of the scanner at `originator_ip` as session `session_handle` until it
/// unregisters or closes the stream
#[tracing::instrument(skip(device, stream, connection_manager))]
async fn serve_connection(
    device: Arc<dyn Device>,
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    session_handle: CipUdint,
    originator_ip: IpAddr,
    connection_manager: Option<Arc<ConnectionManager>>,
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
                    handle_send_rr_data(
                        &device,
                        &packet,
                        originator_ip,
                        connection_manager.as_deref(),
                    )
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

/// The Send RR Data reply carrying the reply to the request inside
fn handle_send_rr_data(
    device: &Arc<dyn Device>,
    packet: &EnIpPacket,
    originator_ip: IpAddr,
    connection_manager: Option<&ConnectionManager>,
) -> EnIpPacket {
    let CommandSpecificData::SendRrData(rr_data) = &packet.command_specific_data else {
        return refusal(&packet.header, EncapsStatusCode::InvalidFormatOrData);
    };
    match handle_unconnected_message(device, rr_data, originator_ip, connection_manager) {
        Ok(reply) => EnIpPacket {
            header: EncapsulationHeader {
                length: None,
                ..packet.header
            },
            command_specific_data: CommandSpecificData::SendRrData(reply),
        },
        Err(status_code) => refusal(&packet.header, status_code),
    }
}
