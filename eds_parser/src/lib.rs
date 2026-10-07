//! Reads a device's EDS file and turns one of its connections into the Forward_Open the
//! `scanner` crate sends. The code reads top to bottom in the order the work happens:
//!
//! ```text
//! Step  From                 To                                          Module
//! ----  -------------------  ------------------------------------------  --------------------
//! 1     the file's text      pest's parse tree                           eds.pest
//! 2     the parse tree       Document: sections, entries, fields         document
//! 3     the document         Param, Assembly, Connection (typed views    params, assembly,
//!                            with their references resolved)             connection
//! 4     one Connection       ForwardOpenRequest + real-time formats      to_forward_open
//! ```
//!
//! [`Eds::parse`] runs steps 1 to 3; [`to_forward_open`] is step 4. Nothing here touches
//! the network: the `eds-implicit-io` example feeds the result to the scanner.

pub mod assembly;
pub mod connection;
pub mod document;
pub mod error;
pub mod params;
pub mod to_forward_open;

pub use assembly::Assembly;
pub use connection::{Connection, ConnectionParameters, DirectionSpec, TriggerAndTransport};
pub use document::{Document, Entry, Field, Section};
pub use error::{BridgeError, EdsError};
pub use params::Param;
pub use to_forward_open::{OriginatorSettings, to_forward_open};

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
        let assemblies = Assembly::all(&document)?;
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

    /// The first connection the device offers to an exclusive owner, which is what this scanner
    /// opens when no connection is named
    pub fn first_exclusive_owner_connection(&self) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|connection| connection.trigger_and_transport.exclusive_owner())
    }
}

// ^^^^^^^^ End of Eds impl ^^^^^^^^
