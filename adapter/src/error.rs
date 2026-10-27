//! The one error type of the adapter.

use std::fmt;

use eipscanne_rs::eip::packet::EncapsulationHeader;

/// What every fallible call of the adapter returns
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// What can go wrong while serving a scanner
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// A packet did not encode or decode
    Parse(binrw::Error),
    /// A packet whose header decoded but whose command specific data did not
    Undecodable {
        header: EncapsulationHeader,
        error: binrw::Error,
    },
}

// ======= Start of Error impl ========

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {error}"),
            Error::Parse(error) => write!(f, "a packet did not encode or decode: {error}"),
            Error::Undecodable { header, error } => {
                write!(f, "a {:?} packet did not decode: {error}", header.command)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::Parse(error) => Some(error),
            Error::Undecodable { error, .. } => Some(error),
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
