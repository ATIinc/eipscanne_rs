//! Stages 1 and 5: the encapsulation session over TCP port 44818.
//!
//! Every explicit message of a connection's life (Forward_Open, Forward_Close) travels inside this
//! session, so it is opened first and closed last.

use std::fmt;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::EncapsStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

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
    /// A reply that should have carried a Message Router response with data did not
    NoResponseData,
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

        if header.status_code != EncapsStatusCode::Success {
            return Err(SessionError::Status(header.status_code));
        }

        Ok(EnIpPacket::read_response(&mut Cursor::new(&bytes))?)
    }

    /// Reads one reply and decodes the data of its Message Router response as a `T`
    pub async fn read_typed_reply<T>(&mut self) -> Result<(EnIpPacket, T), SessionError>
    where
        T: for<'a> BinRead<Args<'a> = ()> + CipData,
    {
        let reply = self.read_reply().await?;

        let Some(response) = reply.response() else {
            return Err(SessionError::NoResponseData);
        };
        // A reply read from the wire always holds its data raw
        let CipDataOpt::Raw(raw) = &response.response_data.data else {
            return Err(SessionError::NoResponseData);
        };
        let typed = T::read_le(&mut Cursor::new(raw))?;

        Ok((reply, typed))
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
            SessionError::NoResponseData => {
                write!(f, "the reply carried no Message Router response data")
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
