//! Reads a device's EDS file and turns one of its connections into the `ConnectionConfig` the
//! `scanner` crate opens. The code reads top to bottom in the order the work happens:
//!
//! ```text
//! Step  From                 To                                          Module
//! ----  -------------------  ------------------------------------------  --------------------
//! 1     the file's text      pest's parse tree                           eds.pest
//! 2     the parse tree       Document: sections, entries, fields         document
//! 3     the document         Param, Assembly, Connection (typed views    params, assembly,
//!                            with their references resolved)             connection
//! 4     one Connection       scanner::implicit::ConnectionConfig         to_connection_config
//! ```
//!
//! [`Eds::parse`] runs steps 1 to 3; [`to_connection_config`] is step 4. Nothing here touches
//! the network: the `eds-implicit-io` example feeds the result to the scanner.
//!
//! An [`Assembly`] also carries its members, the layout its `Display` prints. A caller's
//! assembly struct is written from that layout and checked against it with [`check_assembly`]
//! (module `check`), so the same struct decodes explicit replies and implicit inputs (the
//! `io-hub-implicit` example).

pub mod assembly;
pub mod check;
pub mod connection;
pub mod document;
pub mod error;
pub mod params;
pub mod to_connection_config;

pub use assembly::{Assembly, Member};
pub use check::{AssemblyMismatch, Finding, check_assembly};
pub use connection::{Connection, ConnectionParameters, DirectionSpec, TriggerAndTransport};
pub use document::{Document, Entry, Field, Section};
pub use error::{BridgeError, EdsError};
pub use params::{DataType, Param};
pub use to_connection_config::{OriginatorSettings, to_connection_config};

/// An EDS file read into its document and typed sections
#[derive(Debug, Clone, PartialEq)]
pub struct Eds {
    pub document: Document,
    pub params: Vec<Param>,
    pub assemblies: Vec<Assembly>,
    pub connections: Vec<Connection>,
}

// ======= Start of Eds impl ========

impl Eds {
    /// Parses the text of an EDS file and resolves its params, assemblies and connections
    pub fn parse(text: &str) -> Result<Eds, EdsError> {
        let document = Document::parse(text)?;
        let params = Param::all(&document)?;
        let assemblies = Assembly::all(&document, &params)?;
        let connections = Connection::all(&document, &params, &assemblies)?;
        Ok(Eds {
            document,
            params,
            assemblies,
            connections,
        })
    }

    /// The connection called `keyword_or_name`: its `ConnectionN` keyword or its name, case
    /// ignored
    pub fn connection(&self, keyword_or_name: &str) -> Option<&Connection> {
        self.connections.iter().find(|connection| {
            connection.keyword.eq_ignore_ascii_case(keyword_or_name)
                || connection.name.eq_ignore_ascii_case(keyword_or_name)
        })
    }

    /// The assembly with `keyword` (`Assem100`), case ignored: the layout of a connection
    /// direction's `format`
    pub fn assembly(&self, keyword: &str) -> Option<&Assembly> {
        self.assemblies
            .iter()
            .find(|assembly| assembly.keyword.eq_ignore_ascii_case(keyword))
    }

    /// The first connection the device offers to an exclusive owner, which is what this scanner
    /// opens when no connection is named
    pub fn first_exclusive_owner_connection(&self) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|connection| connection.trigger_and_transport.exclusive_owner())
    }
}

// ^^^^^^^^ End of Eds impl ^^^^^^^^
