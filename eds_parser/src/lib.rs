//! Reads a device's EDS file into its params, assemblies and connections, and resolves what a
//! connection refers to. The `scanner` crate's `implicit::eds` writes a connection's Forward_Open.
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
//!                     consecutive fields, FromField (derived there for bilge types) for the value
//!                     of one field, and the values of more than one section (NumberOrParam, Path)
//! params              [Params]: ParamEntry, its descriptor, data type and values
//! assembly            [Assembly]: AssemblyEntry, its members and what they refer to
//! connection_manager  [Connection Manager]: ConnectionEntry, its two mask words and data format
//! error               Error: Syntax, Entry or Reference, with its message
//! ```

pub mod assembly;
pub mod connection_manager;
pub mod error;
pub mod fields;
pub mod params;
mod syntax;

use std::any::type_name;
use std::collections::BTreeMap;

use assembly::AssemblyEntry;
use connection_manager::{ConnectionEntry, DataFormat};
use error::{Error, Result};
use fields::{NumberOrParam, Path, PathPart, numbered};
use params::{ParamEntry, ParamValue};

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
    pub params: BTreeMap<u32, ParamEntry>,
    pub assemblies: BTreeMap<u32, AssemblyEntry>,
    pub connections: BTreeMap<u32, ConnectionEntry>,
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
                    return Err(Error::Entry(format!("{keyword}: is defined twice")));
                }
            }
        }
        Ok(eds)
    }

    /// A direction's RPI in microseconds: the number, or the default of the `ParamN` it names
    pub fn requested_packet_interval(&self, rpi: Option<NumberOrParam>) -> Result<u32> {
        self.resolve_value(
            rpi.ok_or_else(|| Error::Reference("has no requested packet interval".to_string()))?,
        )
    }

    /// A direction's bytes of data, without the sequence count and real-time header: its size
    /// field, else the size of its `AssemN` format. `Param1,32,Assem150` is 32, `Param1,,Assem100`
    /// the size of Assem100.
    pub fn data_size(
        &self,
        size: Option<NumberOrParam>,
        format: Option<DataFormat>,
    ) -> Result<u16> {
        match (size, format) {
            (Some(size), _) => self.resolve_value(size),
            (None, Some(DataFormat::Assem(number))) => self.assembly_size(number),
            (None, _) => Err(Error::Reference(
                "has no size, and no AssemN format that gives one".to_string(),
            )),
        }
    }

    /// The bytes of a path, each `ParamN` replaced by its default in as many little-endian bytes
    /// as its data size: `"20 04 24 [Param2] 2C 96"` is `20 04 24 97 2C 96` for a 1-byte `Param2`
    /// (field 6) defaulting to 0x97
    pub fn path_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        for part in &path.0 {
            match *part {
                PathPart::Byte(byte) => bytes.push(byte),
                PathPart::Param(number) => {
                    let data_size = usize::from(self.param(number)?.data_size);
                    let default = self.param_default(number)?.to_le_bytes();
                    let value = default.get(..data_size).ok_or_else(|| {
                        Error::Reference(format!(
                            "Param{number} is {data_size} bytes, too large for a path"
                        ))
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
    fn assembly_size(&self, number: u32) -> Result<u16> {
        let assembly = self.assemblies.get(&number).ok_or_else(|| {
            Error::Reference(format!(
                "refers to Assem{number}, which the file does not define"
            ))
        })?;
        if let Some(size) = assembly.size {
            return self.resolve_value(size);
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
            .ok_or_else(|| {
                Error::Reference(format!(
                    "Assem{number} has no size, and its members add up to {bits} bits"
                ))
            })
    }

    /// The number a field holds, or the default of the `ParamN` it names, as a `T`: `32` is 32,
    /// and `Param1` is 1000000 for `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`
    /// (field 12). A number `T` cannot hold is an error naming where it comes from and `T`:
    /// `70000 is out of range for u16`, `Param3's default 70000 is out of range for u16`
    fn resolve_value<T: TryFrom<i64>>(&self, value: NumberOrParam) -> Result<T> {
        let type_name = type_name::<T>();
        match value {
            NumberOrParam::Number(number) => T::try_from(number)
                .map_err(|_| Error::Reference(format!("{number} is out of range for {type_name}"))),
            NumberOrParam::Param(param) => {
                let default = self.param_default(param)?;
                T::try_from(default).map_err(|_| {
                    Error::Reference(format!(
                        "Param{param}'s default {default} is out of range for {type_name}"
                    ))
                })
            }
        }
    }

    fn param_default(&self, number: u32) -> Result<i64> {
        match self.param(number)?.default {
            Some(ParamValue::Integer(default)) => Ok(default),
            _ => Err(Error::Reference(format!(
                "refers to Param{number}, which has no integer default"
            ))),
        }
    }

    fn param(&self, number: u32) -> Result<&ParamEntry> {
        self.params.get(&number).ok_or_else(|| {
            Error::Reference(format!(
                "refers to Param{number}, which the file does not define"
            ))
        })
    }
}

// ^^^^^^^^ End of Eds impl ^^^^^^^^
