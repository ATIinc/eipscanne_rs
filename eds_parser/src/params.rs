//! Step 3a: the `[Params]` section. A connection refers to params for its requested packet
//! interval and sizes, and a path may embed a param's default.

use crate::document::{Document, Entry, Field};
use crate::error::EdsError;

/// A `ParamN` entry, raw: display scaling is ignored, so an RPI param is in microseconds whatever
/// its scaling fields say
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// `ParamN`, as written in the file
    pub keyword: String,
    pub name: String,
    pub data_type: DataType,
    /// Bytes the value occupies on the wire
    pub data_size: u8,
    pub units: String,
    pub help: String,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default: Option<i64>,
    /// The names of the `EnumN` entry with the param's number, as `(value, name)` pairs; the
    /// value is a bit number when the param is a bit string (BYTE, WORD, DWORD, LWORD)
    pub enum_names: Vec<(i64, String)>,
}

/// The CIP elementary data type of a param, from its data type code
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataType {
    Bool,
    Sint,
    Int,
    Dint,
    Lint,
    Usint,
    Uint,
    Udint,
    Ulint,
    Real,
    Lreal,
    Byte,
    Word,
    Dword,
    Lword,
    /// Any other code (strings, dates, structures)
    Other(u8),
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
    /// absent (a file without params is fine as long as nothing refers to one)
    pub fn all(document: &Document) -> Result<Vec<Param>, EdsError> {
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

    fn from_entry(entry: &Entry, names: Option<&Entry>) -> Result<Param, EdsError> {
        let data_type: u8 = integer_field(entry, DATA_TYPE, "the data type code")?;
        Ok(Param {
            keyword: entry.keyword.clone(),
            name: text_or_empty(entry, NAME),
            data_type: DataType::from_code(data_type),
            data_size: integer_field(entry, DATA_SIZE, "the data size in bytes")?,
            units: text_or_empty(entry, UNITS),
            help: text_or_empty(entry, HELP),
            min: optional_integer(entry, MIN)?,
            max: optional_integer(entry, MAX)?,
            default: optional_integer(entry, DEFAULT)?,
            enum_names: match names {
                None => Vec::new(),
                Some(names) => enum_names(names)?,
            },
        })
    }
}

// ^^^^^^^^ End of Param impl ^^^^^^^^

// ======= Start of DataType impl ========

impl DataType {
    pub fn from_code(code: u8) -> DataType {
        match code {
            0xC1 => DataType::Bool,
            0xC2 => DataType::Sint,
            0xC3 => DataType::Int,
            0xC4 => DataType::Dint,
            0xC5 => DataType::Lint,
            0xC6 => DataType::Usint,
            0xC7 => DataType::Uint,
            0xC8 => DataType::Udint,
            0xC9 => DataType::Ulint,
            0xCA => DataType::Real,
            0xCB => DataType::Lreal,
            0xD1 => DataType::Byte,
            0xD2 => DataType::Word,
            0xD3 => DataType::Dword,
            0xD4 => DataType::Lword,
            other => DataType::Other(other),
        }
    }

    /// The type's name as CIP writes it
    pub fn name(&self) -> String {
        match self {
            DataType::Bool => "BOOL".to_string(),
            DataType::Sint => "SINT".to_string(),
            DataType::Int => "INT".to_string(),
            DataType::Dint => "DINT".to_string(),
            DataType::Lint => "LINT".to_string(),
            DataType::Usint => "USINT".to_string(),
            DataType::Uint => "UINT".to_string(),
            DataType::Udint => "UDINT".to_string(),
            DataType::Ulint => "ULINT".to_string(),
            DataType::Real => "REAL".to_string(),
            DataType::Lreal => "LREAL".to_string(),
            DataType::Byte => "BYTE".to_string(),
            DataType::Word => "WORD".to_string(),
            DataType::Dword => "DWORD".to_string(),
            DataType::Lword => "LWORD".to_string(),
            DataType::Other(code) => format!("{code:#04X}"),
        }
    }

    /// Whether the type is a bit string, whose enum names are bit numbers
    pub fn is_bit_string(&self) -> bool {
        matches!(
            self,
            DataType::Byte | DataType::Word | DataType::Dword | DataType::Lword
        )
    }
}

// ^^^^^^^^ End of DataType impl ^^^^^^^^

/// The `(value, name)` pairs of an `EnumN` entry
fn enum_names(entry: &Entry) -> Result<Vec<(i64, String)>, EdsError> {
    entry
        .fields
        .chunks(2)
        .enumerate()
        .filter(|(_, pair)| !pair.iter().all(Field::is_empty))
        .map(|(pair_index, pair)| {
            let index = pair_index * 2;
            let value = pair[0].as_integer().ok_or_else(|| EdsError::BadField {
                entry: entry.keyword.clone(),
                index,
                expected: "an enum value",
                found: pair[0].describe(),
            })?;
            Ok((value, text_or_empty(entry, index + 1)))
        })
        .collect()
}

/// The param with `keyword` (case ignored), or an `UnknownReference` naming `entry`
pub(crate) fn lookup<'a>(
    params: &'a [Param],
    keyword: &str,
    entry: &str,
) -> Result<&'a Param, EdsError> {
    params
        .iter()
        .find(|param| param.keyword.eq_ignore_ascii_case(keyword))
        .ok_or_else(|| EdsError::UnknownReference {
            entry: entry.to_string(),
            reference: keyword.to_string(),
        })
}

/// Whether a word looks like a `ParamN` reference
pub(crate) fn is_param_reference(word: &str) -> bool {
    word.len() > 5
        && word[..5].eq_ignore_ascii_case("Param")
        && word[5..].bytes().all(|byte| byte.is_ascii_digit())
}

/// A field that must be an integer that fits the target type
pub(crate) fn integer_field<T: TryFrom<i64>>(
    entry: &Entry,
    index: usize,
    expected: &'static str,
) -> Result<T, EdsError> {
    let field = entry.field(index);
    field
        .as_integer()
        .and_then(|value| T::try_from(value).ok())
        .ok_or_else(|| EdsError::BadField {
            entry: entry.keyword.clone(),
            index,
            expected,
            found: field.describe(),
        })
}

/// A field that is an integer or empty
pub(crate) fn optional_integer(entry: &Entry, index: usize) -> Result<Option<i64>, EdsError> {
    match entry.field(index) {
        Field::Empty => Ok(None),
        Field::Integer(value) => Ok(Some(*value)),
        other => Err(EdsError::BadField {
            entry: entry.keyword.clone(),
            index,
            expected: "a number or nothing",
            found: other.describe(),
        }),
    }
}

/// A text field, or an empty string when the field is empty or not text
pub(crate) fn text_or_empty(entry: &Entry, index: usize) -> String {
    entry.field(index).as_text().unwrap_or("").to_string()
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
                    data_type: DataType::Udint,
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
                    data_type: DataType::Usint,
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
        assert_eq!(lookup(&params, "param3", "x").unwrap().data_size, 1);
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
            Param::all(&document).unwrap_err(),
            EdsError::BadField {
                entry: "Param1".to_string(),
                index: 5,
                expected: "the data size in bytes",
                found: "the text \"four\"".to_string(),
            }
        );
    }

    #[test]
    fn an_unknown_param_is_named_with_the_entry_that_refers_to_it() {
        assert_eq!(
            lookup(&[], "Param9", "Connection1").unwrap_err(),
            EdsError::UnknownReference {
                entry: "Connection1".to_string(),
                reference: "Param9".to_string(),
            }
        );
    }
}
