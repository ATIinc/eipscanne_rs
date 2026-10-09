//! Step 3a: the `[Params]` section. A connection refers to params for its requested packet
//! interval and sizes, a path may embed a param's default, and an assembly's members are params.

use crate::document::{Document, Entry, Field, is_numbered};
use crate::error::Result;

/// A `ParamN` entry, raw: display scaling is ignored, so an RPI param is in microseconds whatever
/// its scaling fields say
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// `ParamN`, as written in the file
    pub keyword: String,
    pub name: String,
    /// The CIP data type code (`0xC7` is UINT)
    pub data_type: u8,
    /// Bytes the value occupies on the wire
    pub data_size: u8,
    pub units: String,
    pub help: String,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default: Option<i64>,
    /// The names of the `EnumN` entry with the param's number, as `(value, name)` pairs; the
    /// value is a bit number when the param is a bit string
    pub enum_names: Vec<(i64, String)>,
}

/// Field positions of a `ParamN` entry
const DATA_TYPE: usize = 4;
const DATA_SIZE: usize = 5;
const NAME: usize = 6;
const UNITS: usize = 7;
const HELP: usize = 8;
const MIN: usize = 9;
const MAX: usize = 10;
const DEFAULT: usize = 11;

// ======= Start of Param impl ========

impl Param {
    /// Every `ParamN` entry of the `[Params]` section, in file order; none when the section is
    /// absent
    pub(crate) fn all(document: &Document) -> Result<Vec<Param>> {
        let Some(section) = document.section("Params") else {
            return Ok(Vec::new());
        };
        section
            .numbered_entries("Param")
            .map(|entry| {
                // `Param4` takes its names from `Enum4`
                let number = &entry.keyword["Param".len()..];
                let names = section
                    .numbered_entries("Enum")
                    .find(|names| names.keyword["Enum".len()..] == *number);
                Param::from_entry(entry, names)
            })
            .collect()
    }

    fn from_entry(entry: &Entry, names: Option<&Entry>) -> Result<Param> {
        Ok(Param {
            keyword: entry.keyword.clone(),
            name: entry.text(NAME),
            data_type: entry.integer(DATA_TYPE, "a data type code")?,
            data_size: entry.integer(DATA_SIZE, "the data size in bytes")?,
            units: entry.text(UNITS),
            help: entry.text(HELP),
            min: entry.optional_integer(MIN, "a number or nothing")?,
            max: entry.optional_integer(MAX, "a number or nothing")?,
            default: entry.optional_integer(DEFAULT, "a number or nothing")?,
            enum_names: match names {
                None => Vec::new(),
                Some(names) => enum_names(names)?,
            },
        })
    }

    /// The name CIP gives the data type (`UINT`), or its code for a type that is not a number or
    /// a bit string
    pub fn type_name(&self) -> String {
        let name = match self.data_type {
            0xC1 => "BOOL",
            0xC2 => "SINT",
            0xC3 => "INT",
            0xC4 => "DINT",
            0xC5 => "LINT",
            0xC6 => "USINT",
            0xC7 => "UINT",
            0xC8 => "UDINT",
            0xC9 => "ULINT",
            0xCA => "REAL",
            0xCB => "LREAL",
            0xD1 => "BYTE",
            0xD2 => "WORD",
            0xD3 => "DWORD",
            0xD4 => "LWORD",
            other => return format!("{other:#04X}"),
        };
        name.to_string()
    }

    /// Whether the data type is a bit string (BYTE, WORD, DWORD, LWORD), whose enum names are bit
    /// numbers
    pub fn is_bit_string(&self) -> bool {
        matches!(self.data_type, 0xD1..=0xD4)
    }
}

// ^^^^^^^^ End of Param impl ^^^^^^^^

/// The `(value, name)` pairs of an `EnumN` entry
fn enum_names(entry: &Entry) -> Result<Vec<(i64, String)>> {
    entry
        .fields
        .chunks(2)
        .enumerate()
        .filter(|(_, pair)| !pair.iter().all(Field::is_empty))
        .map(|(pair_index, pair)| {
            let index = pair_index * 2;
            let value = pair[0]
                .as_integer()
                .ok_or_else(|| entry.bad_field(index, "an enum value"))?;
            Ok((value, entry.text(index + 1)))
        })
        .collect()
}

/// The param with `keyword` (case ignored); an error about `entry`, which refers to it, otherwise
pub(crate) fn lookup<'a>(params: &'a [Param], keyword: &str, entry: &Entry) -> Result<&'a Param> {
    params
        .iter()
        .find(|param| param.keyword.eq_ignore_ascii_case(keyword))
        .ok_or_else(|| {
            entry.error(format!(
                "refers to {keyword}, which the file does not define"
            ))
        })
}

/// Whether a word is a `ParamN` reference
pub(crate) fn is_param_reference(word: &str) -> bool {
    is_numbered(word, "Param")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMS: &str = r#"
[Params]
    Param1 =
        0,                      $ reserved
        ,,                      $ Link Path Size, Link Path
        0x0004,                 $ Descriptor
        0xC8,                   $ Data Type
        4,                      $ Data Size in bytes
        "RPI Range",            $ name
        "",                     $ units
        "limits the RPI",       $ help string
        1000,1000000,10000,     $ min, max, default
        1,1000,1,0,             $ scaling
        ,,,,                    $ links
        1;                      $ decimal places
    Enum1 = 0,"a";
    Param3 = 0,,,0x0000,0xC6,1,"Config instance","","",,,0x97,,,,,,,,,;
"#;

    #[test]
    fn params_are_read_with_their_types_defaults_and_enum_names_and_scaling_ignored() {
        let document = Document::parse(PARAMS).unwrap();
        let params = Param::all(&document).unwrap();

        assert_eq!(
            params,
            vec![
                Param {
                    keyword: "Param1".to_string(),
                    name: "RPI Range".to_string(),
                    data_type: 0xC8,
                    data_size: 4,
                    units: "".to_string(),
                    help: "limits the RPI".to_string(),
                    min: Some(1000),
                    max: Some(1_000_000),
                    default: Some(10_000),
                    enum_names: vec![(0, "a".to_string())],
                },
                Param {
                    keyword: "Param3".to_string(),
                    name: "Config instance".to_string(),
                    data_type: 0xC6,
                    data_size: 1,
                    units: "".to_string(),
                    help: "".to_string(),
                    min: None,
                    max: None,
                    default: Some(0x97),
                    enum_names: vec![],
                },
            ]
        );
    }

    #[test]
    fn a_missing_params_section_means_no_params() {
        let document = Document::parse("[File]\nRevision = 1.0;\n").unwrap();
        assert_eq!(Param::all(&document).unwrap(), vec![]);
    }

    #[test]
    fn a_data_size_that_is_not_a_number_is_a_bad_field() {
        let document = Document::parse("[Params]\nParam1 = 0,,,0,0xC8,\"four\",\"n\";\n").unwrap();
        assert_eq!(
            Param::all(&document).unwrap_err().to_string(),
            "Param1 (line 2): field 6 should be the data size in bytes, found the text \"four\""
        );
    }
}
