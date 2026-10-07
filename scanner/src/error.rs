//! The one error type of the scanner: everything that can stop a call, from a broken socket to an
//! adapter that says no, to an input packet that is not the next one of the connection. An
//! application wraps it however it handles errors.

use std::fmt;
use std::net::IpAddr;

use eipscanne_rs::cip::connection_manager::parameters::ConnectionSizeType;
use eipscanne_rs::cip::message::response::Rejection;
use eipscanne_rs::eip::command::EncapsStatusCode;

/// What every fallible call of the scanner returns
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// What can go wrong while talking to an adapter
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// A packet or a caller's assembly did not encode or decode
    Parse(binrw::Error),
    /// The adapter answered with an encapsulation status other than success
    EncapsulationStatus(EncapsStatusCode),
    /// The adapter's address is not an IPv4 address, which is all I/O connections support
    NotIpv4(IpAddr),
    /// The adapter refused the request
    Rejected(Rejection),
    /// The reply carries no Message Router response
    NoResponse,
    /// The reply parsed, but is not the reply to what was sent
    UnexpectedReply(String),
    /// An I/O packet that is not the next input of the connection; the caller reports it and
    /// waits for the next one
    UnexpectedPacket(String),
    /// The outputs do not fit the connection
    OutputSize {
        connection_size_type: ConnectionSizeType,
        data_size: u16,
        actual: usize,
    },
}

// The error must cross tasks and fit any application's error handling
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<Error>();
};

// ======= Start of Error impl ========

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {error}"),
            Error::Parse(error) => write!(f, "a packet did not encode or decode: {error}"),
            Error::EncapsulationStatus(status) => {
                write!(
                    f,
                    "the adapter answered with encapsulation status {status:?}"
                )
            }
            Error::NotIpv4(address) => {
                write!(f, "the adapter's address {address} is not an IPv4 address")
            }
            Error::Rejected(rejection) => {
                write!(
                    f,
                    "the adapter rejected {:?}: {:?}",
                    rejection.service, rejection.general_status
                )?;
                // The Connection Manager's reason by name; any other object's Additional Status
                // words as they came
                match rejection.extended_status() {
                    Some(extended_status) => write!(f, ", {extended_status:?}"),
                    None if rejection.additional_status.is_empty() => Ok(()),
                    None => write!(
                        f,
                        ", additional status {:#06x?}",
                        rejection.additional_status
                    ),
                }
            }
            Error::NoResponse => write!(f, "the reply carries no Message Router response"),
            Error::UnexpectedReply(what) => write!(f, "unexpected reply: {what}"),
            Error::UnexpectedPacket(what) => write!(f, "unexpected packet: {what}"),
            Error::OutputSize {
                connection_size_type,
                data_size,
                actual,
            } => {
                let expected = match connection_size_type {
                    ConnectionSizeType::Fixed => "exactly",
                    ConnectionSizeType::Variable => "at most",
                };
                write!(
                    f,
                    "{actual} output bytes given, the connection carries {expected} {data_size}"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Parse(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<binrw::Error> for Error {
    fn from(error: binrw::Error) -> Self {
        Error::Parse(error)
    }
}

impl From<Rejection> for Error {
    fn from(rejection: Rejection) -> Self {
        Error::Rejected(rejection)
    }
}

// ^^^^^^^^ End of Error impl ^^^^^^^^
