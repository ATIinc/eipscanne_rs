//! Reads a device's EDS file and gives the fields of the Forward_Open that opens one of its
//! connections from the `scanner` crate. The code reads top to bottom in the order the work
//! happens:
//!
//! ```text
//! Step  From                 To                                          Module
//! ----  -------------------  ------------------------------------------  --------------------
//! 1     the file's text      pest's parse tree                           eds.pest
//! 2     the parse tree       sections, entries, fields                   document
//! 3     the sections         Param, Assembly, Connection (typed views    params, assembly,
//!                            with their references resolved)             connection
//! 4     one Connection       the Forward_Open fields and real-time       forward_open
//!                            formats
//! ```
//!
//! [`Eds::parse`] runs steps 1 to 3. Nothing here touches the network: the `io-hub-implicit`
//! example writes the `ForwardOpenRequest` from step 4 and hands it to the scanner.
//!
//! An [`Assembly`] also carries its members, the layout its `Display` prints: what a caller's
//! assembly struct is written from (the `eds-assemblies` example).

pub mod assembly;
pub mod connection;
pub(crate) mod document;
pub mod error;
pub mod forward_open;
pub mod params;

use assembly::Assembly;
use connection::Connection;
use document::Document;
use error::Result;
use params::Param;

/// An EDS file read into its typed sections
#[derive(Debug, Clone, PartialEq)]
pub struct Eds {
    pub params: Vec<Param>,
    pub assemblies: Vec<Assembly>,
    pub connections: Vec<Connection>,
}

// ======= Start of Eds impl ========

impl Eds {
    /// Parses the text of an EDS file and resolves its params, assemblies and connections
    pub fn parse(text: &str) -> Result<Eds> {
        let document = Document::parse(text)?;
        let params = Param::all(&document)?;
        let assemblies = Assembly::all(&document, &params)?;
        let connections = Connection::all(&document, &params, &assemblies)?;
        Ok(Eds {
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

    /// The assembly with `keyword` (`Assem100`), case ignored
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
