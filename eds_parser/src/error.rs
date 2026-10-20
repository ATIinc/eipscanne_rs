//! The one error type of the EDS parser: what kind of error it is, and a message that says where,
//! then what: `Connection1: field 4: line 3: Param999,x,Assem101,: expected a number or a ParamN,
//! found "x": invalid digit found in string`.

use std::fmt;

/// What every fallible call of the crate returns
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// What can go wrong reading an EDS file or resolving what it refers to, each with its message
#[derive(Debug)]
pub enum Error {
    /// Text that is not EDS syntax: pest's message, which shows where it is
    Syntax(String),
    /// An entry that does not read: a field that is not what its type reads, a non-empty field past
    /// the last one, or the same `ParamN`, `AssemN` or `ConnectionN` twice
    Entry(String),
    /// What a connection refers to is missing or unusable: a `ParamN` or `AssemN` the file does
    /// not define, a param without an integer default, a direction without an RPI or a size, a
    /// number out of range
    Reference(String),
}

// ======= Start of Error impl ========

impl Error {
    /// The same error, with where it happened in front of its message: `O->T`, `Connection1`
    pub fn context(self, place: impl fmt::Display) -> Error {
        match self {
            Error::Syntax(message) => Error::Syntax(format!("{place}: {message}")),
            Error::Entry(message) => Error::Entry(format!("{place}: {message}")),
            Error::Reference(message) => Error::Reference(format!("{place}: {message}")),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax(message) | Error::Entry(message) | Error::Reference(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for Error {}

// ^^^^^^^^ End of Error impl ^^^^^^^^

// The error must cross tasks and fit any application's error handling
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<Error>();
};
