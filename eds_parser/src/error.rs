//! What can go wrong reading an EDS file or opening one of its connections.

use std::fmt;

/// An EDS file that cannot be read, or a connection this scanner cannot open
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    /// Where: `Assem100 (line 212)`, `line 4, column 7`, `[Connection Manager]`, `Connection2`
    pub entry: String,
    pub message: String,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// ======= Start of Error impl ========

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.entry, self.message)
    }
}

impl std::error::Error for Error {}

// ^^^^^^^^ End of Error impl ^^^^^^^^
