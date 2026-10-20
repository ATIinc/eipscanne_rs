//! Reads a device's EDS file into its params, assemblies and connections, and one connection into
//! the values of the Forward_Open that opens it from the `scanner` crate.
//!
//! An EDS file is `[Section]` headers, each followed by `Keyword = field, field, ...;` entries.
//! Each kind of entry read here is a struct whose fields are the keyword, then the entry's fields
//! in file order:
//!
//! ```text
//! Module         Holds
//! -------------  ----------------------------------------------------------------------------
//! file           the text split into sections and entries, each field as its raw text, and the
//!                serde Deserializer that fills an entry struct from an entry
//! field_values   the kinds of value a field holds (QuotedString, NumberOrParam, PathTokens,
//!                ...), each read with FromStr
//! error          Error: why a file cannot be read, or a connection cannot be opened
//! entries        ParamEntry, AssemblyEntry, ConnectionEntry
//! masks          the two mask words of a ConnectionEntry, a ParamEntry's descriptor and data
//!                type
//! connection     Connection: one ConnectionEntry as Forward_Open values, or why it is refused
//! ```
//!
//! Nothing here touches the network: the `eds-implicit-io` example writes the
//! `ForwardOpenRequest` from a `Connection` and hands it to the scanner.

pub mod connection;
pub mod entries;
pub mod error;
pub mod field_values;
mod file;
pub mod masks;

use std::collections::BTreeMap;

use serde::Deserialize;

use connection::Connection;
use entries::{AssemblyEntry, ConnectionEntry, ParamEntry};
use error::{Error, Result};
use field_values::{NumberOrParam, ParamValue, PathToken, PathTokens, numbered};

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
        for section in file::sections(text)? {
            for entry in &section.entries {
                let keyword = entry.keyword.as_str();
                let defined_twice = match section.name.to_ascii_lowercase().as_str() {
                    "params" if let Some(number) = numbered(keyword, "Param") => eds
                        .params
                        .insert(number, ParamEntry::deserialize(entry)?)
                        .is_some(),
                    "assembly" if let Some(number) = numbered(keyword, "Assem") => eds
                        .assemblies
                        .insert(number, AssemblyEntry::deserialize(entry)?)
                        .is_some(),
                    "connection manager" if let Some(number) = numbered(keyword, "Connection") => {
                        eds.connections
                            .insert(number, ConnectionEntry::deserialize(entry)?)
                            .is_some()
                    }
                    _ => false,
                };
                if defined_twice {
                    return Err(Error::DefinedTwice.in_entry(keyword));
                }
            }
        }
        Ok(eds)
    }

    /// Every `ConnectionN` by its N: the values of the Forward_Open that opens it, or why this
    /// scanner cannot open it.
    ///
    /// `large` picks a Large_Forward_Open (32-bit connection parameters) or a Forward_Open
    /// (16-bit); with `None`, a Large_Forward_Open only when a direction's connection size needs
    /// it.
    pub fn all_connections(&self, large: Option<bool>) -> BTreeMap<u32, Result<Connection>> {
        self.connections
            .iter()
            .map(|(number, entry)| (*number, Connection::new(entry, self, large)))
            .collect()
    }

    /// `ConnectionN`: `eds.connection(1, None)` is `Connection1`; `large` as for
    /// `all_connections`
    pub fn connection(&self, number: u32, large: Option<bool>) -> Result<Connection> {
        match self.all_connections(large).remove(&number) {
            Some(connection) => connection.map_err(|error| Error::InConnection {
                number,
                error: Box::new(error),
            }),
            None => Err(Error::NoSuchConnection(number)),
        }
    }

    /// The first connection this scanner can open, and its N; `large` as for `all_connections`
    pub fn first_connection(&self, large: Option<bool>) -> Result<(u32, Connection)> {
        let mut refusals: BTreeMap<u32, Error> = BTreeMap::new();
        for (number, connection) in self.all_connections(large) {
            match connection {
                Ok(connection) => return Ok((number, connection)),
                Err(refusal) => refusals.insert(number, refusal),
            };
        }
        Err(Error::NoConnection(refusals))
    }

    /// The number a field holds, or the default of the `ParamN` it names, as a `T`: `32` is 32,
    /// and `Param1` is 1000000 for `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`
    /// (field 12). A number `T` cannot hold is an error naming where it comes from and `T`:
    /// `70000 is out of range for u16`, `Param3's default 70000 is out of range for u16`
    pub(crate) fn value<T: TryFrom<i64>>(&self, value: NumberOrParam) -> Result<T> {
        let type_name = std::any::type_name::<T>();
        match value {
            NumberOrParam::Number(number) => T::try_from(number)
                .map_err(|_| Error::Value(format!("{number} is out of range for {type_name}"))),
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

    /// The bytes of a path, each `ParamN` replaced by its default in as many little-endian bytes
    /// as its data size: `"20 04 24 [Param2] 2C 96"` is `20 04 24 97 2C 96` for a 1-byte `Param2`
    /// (field 6) defaulting to 0x97
    pub(crate) fn path(&self, path: &PathTokens) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        for token in &path.0 {
            match *token {
                PathToken::Byte(byte) => bytes.push(byte),
                PathToken::Param(number) => {
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
    pub(crate) fn assembly_size(&self, number: u32) -> Result<u16> {
        let assembly = self.assemblies.get(&number).ok_or_else(|| {
            Error::Reference(format!(
                "refers to Assem{number}, which the file does not define"
            ))
        })?;
        if let Some(size) = assembly.size {
            return self.value(size);
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

    fn param(&self, number: u32) -> Result<&ParamEntry> {
        self.params.get(&number).ok_or_else(|| {
            Error::Reference(format!(
                "refers to Param{number}, which the file does not define"
            ))
        })
    }

    fn param_default(&self, number: u32) -> Result<i64> {
        match self.param(number)?.default {
            Some(ParamValue::Integer(default)) => Ok(default),
            _ => Err(Error::Reference(format!(
                "refers to Param{number}, which has no integer default"
            ))),
        }
    }
}

// ^^^^^^^^ End of Eds impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use super::*;

    const EDS: &str = r#"
[Params]
    Param1 = 0,,,0,0xC6,1,"Config","","",,,0x97;
    Param2 = 0,,,0,0xC7,2,"Point","","",,,0x0102;
    Param3 = 0,,,0,0xCA,4,"Gain","","",,,1.5;
[Assembly]
    Object_Name = "Assembly Object";
    Assem100 = "In", "", 32, 0, , ;
    Assem101 = "Out", "", , 0, , , 16,Param2, 1,, 7,;
    Assem102 = "Bits", "", , 0, , , 3,;
"#;

    #[test]
    fn references_resolve_to_param_defaults_and_assembly_sizes() {
        let eds = Eds::parse(EDS).unwrap();

        assert_eq!(eds.value::<u16>(NumberOrParam::Param(2)), Ok(0x0102));
        assert_eq!(
            eds.value::<u16>(NumberOrParam::Param(3)),
            Err(Error::Reference(
                "refers to Param3, which has no integer default".to_string()
            ))
        );
        assert_eq!(
            eds.value::<u8>(NumberOrParam::Number(300)),
            Err(Error::Value("300 is out of range for u8".to_string()))
        );
        assert_eq!(
            eds.value::<u8>(NumberOrParam::Param(2)),
            Err(Error::Reference(
                "Param2's default 258 is out of range for u8".to_string()
            ))
        );
        assert_eq!(
            eds.path(&"\"20 04 24 [Param1] 2D 00 Param2\"".parse().unwrap()),
            Ok(vec![0x20, 0x04, 0x24, 0x97, 0x2D, 0x00, 0x02, 0x01])
        );
        assert_eq!(eds.assembly_size(100), Ok(32));
        assert_eq!(eds.assembly_size(101), Ok(3));
        assert!(eds.assembly_size(102).is_err());
        assert_eq!(
            eds.assembly_size(9),
            Err(Error::Reference(
                "refers to Assem9, which the file does not define".to_string()
            ))
        );
    }

    #[test]
    fn a_keyword_twice_fails_the_file_and_a_missing_reference_fails_the_connection() {
        assert_eq!(
            Eds::parse(
                "[Params]\nParam1 = 0,,,0,0xC6,1,\"a\",\"\",\"\",,,1;\nParam1 = 0,,,0,0xC6,1,\"b\",\"\",\"\",,,1;\n"
            )
            .unwrap_err()
            .to_string(),
            "Param1: is defined twice"
        );
        assert_eq!(
            Eds::parse(
                "[Connection Manager]\nConnection1 = 0x04010002, 0x44440405, Param7, 32, , 1000, 32, , , , , , \"n\", \"\", \"20 04 24 01 2C 65 2C 64\";\n"
            )
            .unwrap()
            .connection(1, None)
            .unwrap_err()
            .to_string(),
            "Connection1: refers to Param7, which the file does not define"
        );
    }
}
