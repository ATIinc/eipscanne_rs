//! Reads a device's EDS file into its params, assemblies and connections, and one connection into
//! the values of the Forward_Open that opens it from the `scanner` crate.
//!
//! An EDS file is `[Section]` headers, each followed by `Keyword = field, field, ...;` entries.
//! Each kind of entry read here is a struct whose fields are the entry's fields in file order, in
//! the module of its section:
//!
//! ```text
//! Module              Holds
//! ------------------  -----------------------------------------------------------------------
//! syntax              the text split into sections and entries by the grammar in eds.pest, and an
//!                     entry's fields read in file order
//! fields              FromEntry (derived by the eds_parser_derive crate) for a struct read from
//!                     consecutive fields, and FromField (derived there for bilge types) for the
//!                     value of one field
//! values              the kinds of value a field holds beyond strings and integers
//!                     (NumberOrParam, DataFormat, Path, ...)
//! params              [Params]: Param, its descriptor and data type
//! assembly            [Assembly]: Assembly and its members
//! connection_manager  [Connection Manager]: Connection and its two mask words
//! forward_open        a Connection as the fields of a ForwardOpenRequest, or why it is refused
//! ```
//!
//! Nothing here touches the network: the `eds-implicit-io` example writes the
//! `ForwardOpenRequest` and hands it to the scanner.

pub mod assembly;
pub mod connection_manager;
mod fields;
mod forward_open;
pub mod params;
mod syntax;
pub mod values;

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, bail};

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;

use assembly::Assembly;
use connection_manager::Connection;
use fields::numbered;
use params::Param;
use values::{DataFormat, NumberOrParam, ParamValue, Path, PathPart};

/// The params, assemblies and connections of an EDS file, by their N. In this file, `params` holds
/// `Param1`, `assemblies` holds `Assem100` and `connections` holds `Connection1`:
///
/// ```text
/// [Params]
///     Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;
/// [Assembly]
///     Object_Name = "Assembly Object";            $ not read
///     Assem100 = "Input", "20 04 24 64 30 03", 32, 0x0000,,;
/// [Connection Manager]
///     Connection1 = 0x04010002, 0x44440405, Param1,32,, Param1,,Assem100, ,,,,
///                   "Exclusive Owner", "", "20 04 24 01 2C 96 2C 64";
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Eds {
    pub params: BTreeMap<u32, Param>,
    pub assemblies: BTreeMap<u32, Assembly>,
    pub connections: BTreeMap<u32, Connection>,
}

// ======= Start of Eds impl ========

impl Eds {
    /// Reads every `ParamN`, `AssemN` and `ConnectionN` entry of the text. Other sections and
    /// keywords (`Object_Name`, vendor keywords) are not read.
    pub fn parse(text: &str) -> Result<Eds> {
        let mut eds = Eds::default();
        for section in syntax::sections(text)? {
            let section_name = section.name.to_ascii_lowercase();
            for entry in section.entries {
                let keyword = entry.keyword;
                let defined_twice = match section_name.as_str() {
                    "params" if let Some(number) = numbered(keyword, "Param") => {
                        eds.params.insert(number, entry.read()?).is_some()
                    }
                    "assembly" if let Some(number) = numbered(keyword, "Assem") => {
                        eds.assemblies.insert(number, entry.read()?).is_some()
                    }
                    "connection manager" if let Some(number) = numbered(keyword, "Connection") => {
                        eds.connections.insert(number, entry.read()?).is_some()
                    }
                    _ => false,
                };
                if defined_twice {
                    bail!("{keyword}: is defined twice");
                }
            }
        }
        Ok(eds)
    }

    /// `ConnectionN`: `eds.connection(1)` is `Connection1`
    pub fn connection(&self, number: u32) -> Result<&Connection> {
        self.connections
            .get(&number)
            .with_context(|| format!("Connection{number}: the EDS has no such connection"))
    }

    /// The N of the first `ConnectionN` this scanner can open, or why each one cannot be opened;
    /// `large` as for `forward_open_request`
    pub fn first_openable_connection(&self, large: Option<bool>) -> Result<u32> {
        if self.connections.is_empty() {
            bail!("[Connection Manager]: the EDS has no ConnectionN entries");
        }
        let mut refusals = String::new();
        for &number in self.connections.keys() {
            match self.forward_open_request(number, large) {
                Ok(_) => return Ok(number),
                Err(refusal) => refusals.push_str(&format!("\n  {refusal:#}")),
            }
        }
        bail!("[Connection Manager]: no connection this scanner can open:{refusals}")
    }

    /// The Forward_Open fields `ConnectionN` decides: both RPIs, both network connection
    /// parameters, the transport class and trigger, and the connection path. Every other field is
    /// zero, for the caller to set:
    ///
    /// ```text
    /// let request = ForwardOpenRequest {
    ///     priority_time_tick: ...,
    ///     timeout_ticks: 5,
    ///     o2t_network_connection_id: 0,
    ///     t2o_network_connection_id: 0x1234_5678,
    ///     connection_triad: ConnectionTriad { ... },
    ///     connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
    ///     ..eds.forward_open_request(1, None)?
    /// };
    /// ```
    ///
    /// `large` picks a Large_Forward_Open (32-bit connection parameters) or a Forward_Open
    /// (16-bit); with `None`, a Large_Forward_Open only when a direction's connection size needs
    /// it.
    pub fn forward_open_request(
        &self,
        number: u32,
        large: Option<bool>,
    ) -> Result<ForwardOpenRequest> {
        let connection = self.connection(number)?;
        forward_open::request(self, connection, large)
            .with_context(|| format!("Connection{number}"))
    }

    /// Bytes of output data, without the sequence count and real-time header:
    /// `Param1,32,Assem150` is 32
    pub fn o2t_size(&self, number: u32) -> Result<u16> {
        let connection = self.connection(number)?;
        self.data_size(connection.o2t_size, connection.o2t_format)
            .with_context(|| format!("Connection{number}: O->T"))
    }

    /// A direction's RPI in microseconds: the number, or the default of the `ParamN` it names
    pub(crate) fn requested_packet_interval(&self, rpi: Option<NumberOrParam>) -> Result<u32> {
        self.resolve(rpi.context("has no requested packet interval")?)
    }

    /// A direction's bytes of data: its size field, else the size of its `AssemN` format
    pub(crate) fn data_size(
        &self,
        size: Option<NumberOrParam>,
        format: Option<DataFormat>,
    ) -> Result<u16> {
        match (size, format) {
            (Some(size), _) => self.resolve(size),
            (None, Some(DataFormat::Assem(number))) => self.assembly_size(number),
            (None, _) => bail!("has no size, and no AssemN format that gives one"),
        }
    }

    /// The number a field holds, or the default of the `ParamN` it names, as a `T`: `32` is 32,
    /// and `Param1` is 1000000 for `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`
    /// (field 12). A number `T` cannot hold is an error naming where it comes from and `T`:
    /// `70000 is out of range for u16`, `Param3's default 70000 is out of range for u16`
    pub(crate) fn resolve<T: TryFrom<i64>>(&self, value: NumberOrParam) -> Result<T> {
        let type_name = std::any::type_name::<T>();
        match value {
            NumberOrParam::Number(number) => {
                T::try_from(number).map_err(|_| anyhow!("{number} is out of range for {type_name}"))
            }
            NumberOrParam::Param(param) => {
                let default = self.param_default(param)?;
                T::try_from(default).map_err(|_| {
                    anyhow!("Param{param}'s default {default} is out of range for {type_name}")
                })
            }
        }
    }

    /// The bytes of a path, each `ParamN` replaced by its default in as many little-endian bytes
    /// as its data size: `"20 04 24 [Param2] 2C 96"` is `20 04 24 97 2C 96` for a 1-byte `Param2`
    /// (field 6) defaulting to 0x97
    pub(crate) fn path_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        for part in &path.0 {
            match *part {
                PathPart::Byte(byte) => bytes.push(byte),
                PathPart::Param(number) => {
                    let data_size = usize::from(self.param(number)?.data_size);
                    let default = self.param_default(number)?.to_le_bytes();
                    let value = default.get(..data_size).with_context(|| {
                        format!("Param{number} is {data_size} bytes, too large for a path")
                    })?;
                    bytes.extend_from_slice(value);
                }
            }
        }
        Ok(bytes)
    }

    /// The size in bytes of `AssemN`: its size field, or the sum of its member sizes
    ///
    /// ```text
    /// Assem100 = "In", "", 32, 0x0000,,;                      $ Assem100: 32 bytes
    /// Assem101 = "Out", "", , 0x0000,,, 16,Param2, 8,;        $ Assem101: 16 + 8 bits, 3 bytes
    /// ```
    pub(crate) fn assembly_size(&self, number: u32) -> Result<u16> {
        let assembly = self
            .assemblies
            .get(&number)
            .with_context(|| format!("refers to Assem{number}, which the file does not define"))?;
        if let Some(size) = assembly.size {
            return self.resolve(size);
        }

        let bits: u32 = assembly
            .members
            .iter()
            .filter_map(|member| member.size_bits)
            .map(u32::from)
            .sum();
        u16::try_from(bits / 8)
            .ok()
            .filter(|&bytes| bytes > 0 && bits.is_multiple_of(8))
            .with_context(|| {
                format!("Assem{number} has no size, and its members add up to {bits} bits")
            })
    }

    fn param(&self, number: u32) -> Result<&Param> {
        self.params
            .get(&number)
            .with_context(|| format!("refers to Param{number}, which the file does not define"))
    }

    fn param_default(&self, number: u32) -> Result<i64> {
        match self.param(number)?.default {
            Some(ParamValue::Integer(default)) => Ok(default),
            _ => bail!("refers to Param{number}, which has no integer default"),
        }
    }
}

// ^^^^^^^^ End of Eds impl ^^^^^^^^
