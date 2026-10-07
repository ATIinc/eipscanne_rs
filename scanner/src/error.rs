//! The one error type of the scanner: everything that can stop a call, from a broken socket to an
//! adapter that says no. An application wraps it however it handles errors; `Discarded` is not
//! here because a discarded input packet stops nothing.

use std::fmt;
use std::net::IpAddr;

use eipscanne_rs::cip::connection_manager::parameters::ConnectionSizeType;
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::command::EncapsStatusCode;

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
    /// The adapter answered `service` with a general status other than success
    Rejected {
        service: ServiceCode,
        general_status: ResponseStatusCode,
        additional_status: Vec<CipUint>,
    },
    /// The reply parsed, but is not the reply to what was sent
    UnexpectedReply(String),
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

impl Error {
    /// The rejection of `service` that `response` carries
    pub(crate) fn rejected(service: ServiceCode, response: &MessageRouterResponse) -> Self {
        Error::Rejected {
            service,
            general_status: response.response_data.status,
            additional_status: response.response_data.additional_status.clone(),
        }
    }
}

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
            Error::Rejected {
                service,
                general_status,
                additional_status,
            } => {
                write!(f, "the adapter rejected {service:?}: {general_status}")?;
                // The Connection Manager says why in its extended status; any other object's
                // additional status is its own, so it is printed as it came
                let connection_manager = matches!(
                    service,
                    ServiceCode::ForwardOpen
                        | ServiceCode::LargeForwardOpen
                        | ServiceCode::ForwardClose
                );
                match ConnectionManagerExtendedStatus::from_additional_status(additional_status) {
                    Some(extended_status) if connection_manager => write!(f, ", {extended_status}"),
                    _ if additional_status.is_empty() => Ok(()),
                    _ => write!(f, ", additional status {additional_status:#06x?}"),
                }
            }
            Error::UnexpectedReply(what) => write!(f, "unexpected reply: {what}"),
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

// ^^^^^^^^ End of Error impl ^^^^^^^^
