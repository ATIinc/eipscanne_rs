//! The encapsulation session over TCP port 44818, shared by explicit and implicit messaging. It
//! carries the Forward_Open and Forward_Close too, so it is opened first and closed last.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::time::timeout;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::response::Rejection;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::EncapsStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::{Error, Result};

/// HACK for Claude: with `EIP_DUMP` set, every encapsulation packet is printed to stderr as hex
/// for `scripts/dissect.sh`, since the devcontainer cannot capture packets:
/// `EIP_DUMP=1 cargo run --example implicit-io -- ...`
const DUMP_VARIABLE: &str = "EIP_DUMP";

/// Size of the encapsulation header on the wire
const ENCAPSULATION_HEADER_LEN: usize = 24;

/// How long connecting to the adapter may take: a wrong or unreachable address fails after this
/// instead of after the operating system gives up (minutes on Linux)
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the adapter may take to answer one request
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// A registered encapsulation session with one adapter
#[derive(Debug)]
pub struct Session {
    stream: TcpStream,
    session_handle: CipUdint,
    peer_ip: Ipv4Addr,
}

// ======= Start of Session impl ========

impl Session {
    /// Stage 1: connects to the adapter and registers a session with it. Fails with
    /// [`Error::Timeout`] when the adapter does not accept the connection within
    /// [`CONNECT_TIMEOUT`] or does not answer within [`REPLY_TIMEOUT`].
    pub async fn register(address: impl ToSocketAddrs) -> Result<Session> {
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Timeout {
                waiting_for: "the TCP connection",
                after: CONNECT_TIMEOUT,
            })??;
        let peer_ip = match stream.peer_addr()?.ip() {
            IpAddr::V4(ip) => ip,
            other => return Err(Error::NotIpv4(other)),
        };

        let mut session = Session {
            stream,
            session_handle: 0,
            peer_ip,
        };
        session
            .send(&RequestObjectAssembly::new_registration())
            .await?;
        let reply = session.read_reply().await?;
        session.session_handle = reply.header.session_handle;

        Ok(session)
    }

    /// The handle the adapter gave this session, carried by every packet
    pub fn session_handle(&self) -> CipUdint {
        self.session_handle
    }

    /// The adapter's IP address, where its I/O packets come from
    pub fn peer_ip(&self) -> Ipv4Addr {
        self.peer_ip
    }

    /// Writes one encapsulation packet to the adapter
    pub async fn send(&mut self, packet: &EnIpPacket) -> Result<()> {
        let mut bytes = Cursor::new(Vec::new());
        packet.write(&mut bytes)?;
        print_packet_hex("REQUEST", bytes.get_ref());
        self.stream.write_all(bytes.get_ref()).await?;
        Ok(())
    }

    /// Reads one encapsulation packet: the header, then its Length in bytes. Fails on a
    /// non-success encapsulation status, and with [`Error::Timeout`] when the whole packet has
    /// not arrived within [`REPLY_TIMEOUT`]; the session is unusable after a timeout, since part
    /// of the packet may have been read.
    pub async fn read_reply(&mut self) -> Result<EnIpPacket> {
        timeout(REPLY_TIMEOUT, self.read_packet())
            .await
            .map_err(|_| Error::Timeout {
                waiting_for: "a reply",
                after: REPLY_TIMEOUT,
            })?
    }

    /// [`Session::read_reply`] without the timeout
    async fn read_packet(&mut self) -> Result<EnIpPacket> {
        let mut bytes = vec![0u8; ENCAPSULATION_HEADER_LEN];
        self.stream.read_exact(&mut bytes).await?;

        let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))?;
        let length = usize::from(header.length.unwrap_or(0));
        bytes.resize(ENCAPSULATION_HEADER_LEN + length, 0);
        self.stream
            .read_exact(&mut bytes[ENCAPSULATION_HEADER_LEN..])
            .await?;

        print_packet_hex("REPLY", &bytes);

        if header.status_code != EncapsStatusCode::Success {
            return Err(Error::EncapsulationStatus(header.status_code));
        }

        Ok(EnIpPacket::read(&mut Cursor::new(&bytes))?)
    }

    /// Sends a Message Router request and returns the accepted reply to the same service
    pub async fn request(&mut self, packet: &EnIpPacket) -> Result<EnIpPacket> {
        self.send(packet).await?;
        let reply = self.read_reply().await?;

        let response = reply.response().ok_or(Error::NoResponse)?;
        if let Some(CipMessage::Request(request)) = packet
            .command_specific_data
            .as_send_rr_data()
            .map(|rr_data| &rr_data.cip_message)
        {
            let requested = request.service_container.service();
            let answered = response.service_container.service();
            if answered != requested {
                return Err(Error::UnexpectedReply(format!(
                    "a {answered:?} reply answered {requested:?}"
                )));
            }
        }
        if let Some(rejection) = Rejection::from_response(response) {
            return Err(rejection.into());
        }

        Ok(reply)
    }

    /// Stage 5: tells the adapter the session is over and closes the connection
    pub async fn unregister(mut self) -> Result<()> {
        self.send(&RequestObjectAssembly::new_unregistration(
            self.session_handle,
        ))
        .await
        // The stream is dropped, which closes it
    }
}

// ^^^^^^^^ End of Session impl ^^^^^^^^

/// Prints `bytes` as hex after `direction` when `EIP_DUMP` is set (see `DUMP_VARIABLE`)
fn print_packet_hex(direction: &str, bytes: &[u8]) {
    if std::env::var_os(DUMP_VARIABLE).is_none() {
        return;
    }
    let hex: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    eprintln!("{direction} {}", hex.join(" "));
}
