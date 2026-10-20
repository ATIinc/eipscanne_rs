//! What can go wrong reading an EDS file or opening one of its connections.

use std::collections::BTreeMap;
use std::fmt;

/// An EDS file that cannot be read, or a connection this scanner cannot open
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Text that is not EDS syntax, named by where it starts: `K 1: the entry has no =`
    Syntax { text: String, message: &'static str },
    /// A field's text that is not the value its place in the entry calls for:
    /// `expected a number or a ParamN, found "x"`
    Value(String),
    /// A `ParamN` or `AssemN` that is missing or gives no value:
    /// `refers to Param7, which the file does not define`
    Reference(String),
    /// Why this scanner cannot open a connection:
    /// `this scanner opens class 1, cyclic, exclusive-owner connections; this one offers ...`
    Refused(String),
    /// A keyword that is in its section twice
    DefinedTwice,
    /// A non-empty field after the last field of its kind of entry
    ExtraField,
    /// An error in an entry, and in which field when it is in one:
    /// `Connection1: field 4: expected a number or a ParamN, found "x"`, `Param1: is defined twice`
    InEntry {
        keyword: String,
        field: Option<usize>,
        error: Box<Error>,
    },
    /// An error opening `ConnectionN`: `Connection2: this scanner opens ...`
    InConnection { number: u32, error: Box<Error> },
    /// `connection(3, ..)` of a file without `Connection3`
    NoSuchConnection(u32),
    /// `first_connection` of a file with no connection this scanner can open: why each one cannot
    /// be opened, by its N
    NoConnection(BTreeMap<u32, Error>),
}

pub type Result<T> = std::result::Result<T, Error>;

// ======= Start of Error impl ========

impl Error {
    /// This error, in the entry `keyword`: `Param1: is defined twice`
    pub(crate) fn in_entry(self, keyword: &str) -> Error {
        Error::InEntry {
            keyword: keyword.to_string(),
            field: None,
            error: Box::new(self),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax { text, message } => write!(f, "{text}: {message}"),
            Error::Value(message) | Error::Reference(message) | Error::Refused(message) => {
                write!(f, "{message}")
            }
            Error::DefinedTwice => write!(f, "is defined twice"),
            Error::ExtraField => write!(f, "is past the last field of this kind of entry"),
            Error::InEntry {
                keyword,
                field: Some(field),
                error,
            } => write!(f, "{keyword}: field {field}: {error}"),
            Error::InEntry {
                keyword,
                field: None,
                error,
            } => write!(f, "{keyword}: {error}"),
            Error::InConnection { number, error } => write!(f, "Connection{number}: {error}"),
            Error::NoSuchConnection(number) => {
                write!(f, "Connection{number}: the EDS has no such connection")
            }
            Error::NoConnection(refusals) if refusals.is_empty() => {
                write!(
                    f,
                    "[Connection Manager]: the EDS has no ConnectionN entries"
                )
            }
            Error::NoConnection(refusals) => {
                write!(
                    f,
                    "[Connection Manager]: no connection this scanner can open:"
                )?;
                refusals.iter().try_for_each(|(number, refusal)| {
                    write!(f, "\n  Connection{number}: {refusal}")
                })
            }
        }
    }
}

impl std::error::Error for Error {}

/// A field that does not read as its type; reading the entry puts it in its entry and field
/// (`Connection1: field 4: ...`)
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Error {
        Error::Value(message.to_string())
    }
}

// ^^^^^^^^ End of Error impl ^^^^^^^^
