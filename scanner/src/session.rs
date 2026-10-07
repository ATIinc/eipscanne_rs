//! The encapsulation session over TCP port 44818, shared by explicit and implicit messaging.
//!
//! Every explicit request travels inside the session, and so do the Forward_Open and
//! Forward_Close that bracket an I/O connection, so it is opened first and closed last.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::response::Rejection;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::EncapsStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::Error;

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

// ======= Start of Session impl ========

impl Session {
    /// Stage 1: connects to the adapter and registers a session with it
    pub async fn register(address: impl ToSocketAddrs) -> Result<Session, Error> {
        let stream = TcpStream::connect(address).await?;
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

    /// The handle the adapter gave this session; every packet sent over it carries this handle
    pub fn session_handle(&self) -> CipUdint {
        self.session_handle
    }

    /// The adapter's IP address, which is also where its I/O packets come from
    pub fn peer_ip(&self) -> Ipv4Addr {
        self.peer_ip
    }

    /// Writes one encapsulation packet to the adapter
    pub async fn send(&mut self, packet: &EnIpPacket) -> Result<(), Error> {
        let mut bytes = Cursor::new(Vec::new());
        packet.write(&mut bytes)?;
        dump_if_requested("REQUEST", bytes.get_ref());
        self.stream.write_all(bytes.get_ref()).await?;
        Ok(())
    }

    /// Reads one encapsulation packet from the adapter: the 24-byte header, then exactly as many
    /// bytes as its Length field says. Fails when the encapsulation status is not success.
    pub async fn read_reply(&mut self) -> Result<EnIpPacket, Error> {
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
            return Err(Error::EncapsulationStatus(header.status_code));
        }

        Ok(EnIpPacket::read_response(&mut Cursor::new(&bytes))?)
    }

    /// Sends a Message Router request and reads its reply: the reply to the same service, once
    /// the adapter accepted the request
    pub async fn request(&mut self, packet: &EnIpPacket) -> Result<EnIpPacket, Error> {
        self.send(packet).await?;
        let reply = self.read_reply().await?;

        let response = reply.response().ok_or(Error::NoResponse)?;
        if let Some(CipMessage::Request(request)) = packet.cip_message() {
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
    pub async fn unregister(mut self) -> Result<(), Error> {
        self.send(&RequestObjectAssembly::new_unregistration(
            self.session_handle,
        ))
        .await
        // The stream is dropped, which closes it
    }
}

// ^^^^^^^^ End of Session impl ^^^^^^^^

/// Prints `bytes` as hex after `direction` when `EIP_DUMP` is set (see `DUMP_VARIABLE`)
fn dump_if_requested(direction: &str, bytes: &[u8]) {
    if std::env::var_os(DUMP_VARIABLE).is_none() {
        return;
    }
    let hex: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    eprintln!("{direction} {}", hex.join(" "));
}
