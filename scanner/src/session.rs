//! The encapsulation session over TCP port 44818, shared by explicit and implicit messaging.
//!
//! Every explicit request travels inside the session, and so do the Forward_Open and
//! Forward_Close that bracket an I/O connection, so it is opened first and closed last.

use std::fmt;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::EncapsStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

/// HACK for Claude: with `EIP_DUMP` set, every encapsulation packet the session sends or reads
/// is printed to stderr as hex (`REQUEST 6f 00 ...`, `REPLY 6f 00 ...`), ready for
/// `scripts/dissect.sh`. This is how Claude sees a session's traffic where it cannot capture
/// packets (the devcontainer has no capture permission):
/// `EIP_DUMP=1 cargo run --example io-hub-implicit -- ...`
const DUMP_VARIABLE: &str = "EIP_DUMP";

/// Size of the encapsulation header on the wire: command, length, session handle, status, sender
/// context and options
const ENCAPSULATION_HEADER_LEN: usize = 24;

/// A registered encapsulation session with one adapter
#[derive(Debug)]
pub struct Session {
    stream: TcpStream,
    session_handle: CipUdint,
    peer_ip: Ipv4Addr,
}

/// What can go wrong while talking to the adapter over the session
#[derive(Debug)]
pub enum SessionError {
    Io(std::io::Error),
    /// A reply did not parse as an encapsulation packet
    Parse(binrw::Error),
    /// The adapter answered with an encapsulation status other than success
    Status(EncapsStatusCode),
    /// The adapter's address is not an IPv4 address, which is all I/O connections support
    NotIpv4(IpAddr),
}

// ======= Start of Session impl ========

impl Session {
    /// Stage 1: connects to the adapter and registers a session with it
    pub async fn register(address: impl ToSocketAddrs) -> Result<Session, SessionError> {
        let stream = TcpStream::connect(address).await?;
        let peer_ip = match stream.peer_addr()?.ip() {
            IpAddr::V4(ip) => ip,
            other => return Err(SessionError::NotIpv4(other)),
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

    /// The handle the adapter gave this session; every packet sent over it carries this handle
    pub fn session_handle(&self) -> CipUdint {
        self.session_handle
    }

    /// The adapter's IP address, which is also where its I/O packets come from
    pub fn peer_ip(&self) -> Ipv4Addr {
        self.peer_ip
    }

    /// Writes one encapsulation packet to the adapter
    pub async fn send(&mut self, packet: &EnIpPacket) -> Result<(), SessionError> {
        let mut bytes = Cursor::new(Vec::new());
        packet.write(&mut bytes)?;
        dump_if_requested("REQUEST", bytes.get_ref());
        self.stream.write_all(bytes.get_ref()).await?;
        Ok(())
    }

    /// Reads one encapsulation packet from the adapter: the 24-byte header, then exactly as many
    /// bytes as its Length field says. Fails when the encapsulation status is not success.
    pub async fn read_reply(&mut self) -> Result<EnIpPacket, SessionError> {
        let mut bytes = vec![0u8; ENCAPSULATION_HEADER_LEN];
        self.stream.read_exact(&mut bytes).await?;

        let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))?;
        let length = usize::from(header.length.unwrap_or(0));
        bytes.resize(ENCAPSULATION_HEADER_LEN + length, 0);
        self.stream
            .read_exact(&mut bytes[ENCAPSULATION_HEADER_LEN..])
            .await?;

        dump_if_requested("REPLY", &bytes);

        if header.status_code != EncapsStatusCode::Success {
            return Err(SessionError::Status(header.status_code));
        }

        Ok(EnIpPacket::read_response(&mut Cursor::new(&bytes))?)
    }

    /// Stage 5: tells the adapter the session is over and closes the connection
    pub async fn unregister(mut self) -> Result<(), SessionError> {
        self.send(&RequestObjectAssembly::new_unregistration(
            self.session_handle,
        ))
        .await
        // The stream is dropped, which closes it
    }
}

// ^^^^^^^^ End of Session impl ^^^^^^^^

// ======= Start of SessionError impl ========

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Io(error) => write!(f, "I/O error on the session: {error}"),
            SessionError::Parse(error) => write!(f, "the reply did not parse: {error}"),
            SessionError::Status(status) => {
                write!(
                    f,
                    "the adapter answered with encapsulation status {status:?}"
                )
            }
            SessionError::NotIpv4(address) => {
                write!(f, "the adapter's address {address} is not an IPv4 address")
            }
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionError::Io(error) => Some(error),
            SessionError::Parse(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SessionError {
    fn from(error: std::io::Error) -> Self {
        SessionError::Io(error)
    }
}

impl From<binrw::Error> for SessionError {
    fn from(error: binrw::Error) -> Self {
        SessionError::Parse(error)
    }
}

// ^^^^^^^^ End of SessionError impl ^^^^^^^^

/// Prints `bytes` as hex after `direction` when `EIP_DUMP` is set (see `DUMP_VARIABLE`)
fn dump_if_requested(direction: &str, bytes: &[u8]) {
    if std::env::var_os(DUMP_VARIABLE).is_none() {
        return;
    }
    let hex: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    eprintln!("{direction} {}", hex.join(" "));
}
