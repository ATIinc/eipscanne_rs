//! What can go wrong reading an EDS file and turning a connection into a configuration.

use std::fmt;

/// An EDS file that cannot be read
#[derive(Debug, Clone, PartialEq)]
pub enum EdsError {
    /// The text does not follow the EDS syntax; `line` and `column` are 1-based
    Syntax {
        line: usize,
        column: usize,
        message: String,
    },
    /// A section the typed views need is not in the file
    MissingSection(String),
    /// A field of an entry has the wrong form; `index` is the field's 0-based position
    BadField {
        entry: String,
        index: usize,
        expected: &'static str,
        found: String,
    },
    /// An entry refers to a `ParamN` or `AssemN` that does not exist
    UnknownReference { entry: String, reference: String },
}

/// A connection the bridge cannot turn into a Forward_Open
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// The connection does not support what this scanner does (class 1, cyclic, exclusive owner,
    /// point-to-point, a known real-time format, a shared priority)
    Unsupported { connection: String, what: String },
    /// A direction has no requested packet interval in the EDS
    MissingRpi {
        connection: String,
        direction: &'static str,
    },
    /// The connection path is not `20 04 24 cc 2C oo 2C tt`
    UnsupportedPath { connection: String, path: Vec<u8> },
    /// A direction's connection size does not fit the 9 bits of a Forward_Open; a
    /// Large_Forward_Open carries up to 65535 bytes
    ConnectionSizeTooLarge {
        connection: String,
        direction: &'static str,
        size: u16,
    },
}

// ======= Start of EdsError impl ========

impl fmt::Display for EdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdsError::Syntax {
                line,
                column,
                message,
            } => write!(f, "line {line}, column {column}: {message}"),
            EdsError::MissingSection(section) => write!(f, "no [{section}] section"),
            EdsError::BadField {
                entry,
                index,
                expected,
                found,
            } => write!(
                f,
                "{entry}: field {} should be {expected}, found {found}",
                index + 1
            ),
            EdsError::UnknownReference { entry, reference } => {
                write!(
                    f,
                    "{entry}: refers to {reference}, which the file does not define"
                )
            }
        }
    }
}

impl std::error::Error for EdsError {}

// ^^^^^^^^ End of EdsError impl ^^^^^^^^

// ======= Start of BridgeError impl ========

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::Unsupported { connection, what } => {
                write!(f, "{connection}: {what}")
            }
            BridgeError::MissingRpi {
                connection,
                direction,
            } => write!(
                f,
                "{connection}: the EDS gives no {direction} requested packet interval"
            ),
            BridgeError::UnsupportedPath { connection, path } => {
                let hex: Vec<String> = path.iter().map(|byte| format!("{byte:02X}")).collect();
                write!(
                    f,
                    "{connection}: path {} is not configuration, O->T and T->O assembly instances as 8-bit segments",
                    hex.join(" ")
                )
            }
            BridgeError::ConnectionSizeTooLarge {
                connection,
                direction,
                size,
            } => write!(
                f,
                "{connection}: the {direction} connection size of {size} bytes needs a Large_Forward_Open"
            ),
        }
    }
}

impl std::error::Error for BridgeError {}

// ^^^^^^^^ End of BridgeError impl ^^^^^^^^
