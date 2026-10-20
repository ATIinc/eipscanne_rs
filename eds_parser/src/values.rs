//! The kinds of value an EDS field holds beyond strings and integers, each read from the field's
//! text with `FromField`.

use crate::error::{Error, Result};
use crate::fields::{FromField, numbered};

/// A number, or a `ParamN` that stands for its default: `32`, `0x20`, `Param3`
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberOrParam {
    Number(i64),
    Param(u32),
}

/// `0x20` -> `Number(32)`, `Param3` -> `Param(3)`
impl FromField for NumberOrParam {
    fn from_field(text: &str) -> Result<NumberOrParam> {
        if let Some(param) = numbered(text, "Param") {
            return Ok(NumberOrParam::Param(param));
        }
        parse_int::parse(text)
            .map(NumberOrParam::Number)
            .map_err(|cause| {
                Error::Entry(format!(
                    "expected a number or a ParamN, found {text:?}: {cause}"
                ))
            })
    }
}

/// The data format of a connection direction: `Assem100`, `AssemExa100`, `Param4`
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataFormat {
    Assem(u32),
    AssemExa(u32),
    Param(u32),
}

/// `Assem100` -> `Assem(100)`, `AssemExa7` -> `AssemExa(7)`, `Param4` -> `Param(4)`
impl FromField for DataFormat {
    fn from_field(text: &str) -> Result<DataFormat> {
        if let Some(number) = numbered(text, "AssemExa") {
            Ok(DataFormat::AssemExa(number))
        } else if let Some(number) = numbered(text, "Assem") {
            Ok(DataFormat::Assem(number))
        } else if let Some(number) = numbered(text, "Param") {
            Ok(DataFormat::Param(number))
        } else {
            Err(Error::Entry(format!(
                "expected an AssemN, AssemExaN or ParamN, found {text:?}"
            )))
        }
    }
}

/// A value in a param's data type, its minimum, maximum or default: `1000`, `0x97`, `2.5`, `"on"`
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Integer(i64),
    Real(f64),
    Text(String),
}

/// `0x97` -> `Integer(151)`, `2.5` -> `Real(2.5)`, `"on"` -> `Text("on")`
impl FromField for ParamValue {
    fn from_field(text: &str) -> Result<ParamValue> {
        if let Ok(integer) = parse_int::parse(text) {
            return Ok(ParamValue::Integer(integer));
        }
        if let Ok(real) = text.parse::<f64>() {
            return Ok(ParamValue::Real(real));
        }
        String::from_field(text).map(ParamValue::Text).map_err(|_| {
            Error::Entry(format!(
                "expected a number or a quoted string, found {text:?}"
            ))
        })
    }
}

/// A path string: hex bytes and `ParamN` references, written with or without brackets, each
/// standing for the param's default: `"20 04 24 [Param2] 2C 96 2C 64"`
#[derive(Debug, Clone, PartialEq)]
pub struct Path(pub Vec<PathPart>);

/// `"20 04 [Param2]"` -> `[Byte(0x20), Byte(0x04), Param(2)]`
impl FromField for Path {
    fn from_field(text: &str) -> Result<Path> {
        let path = String::from_field(text)?;
        path.split_whitespace()
            .map(|token| {
                let reference = token.trim_start_matches('[').trim_end_matches(']');
                if let Some(param) = numbered(reference, "Param") {
                    return Ok(PathPart::Param(param));
                }
                Some(token)
                    .filter(|token| token.len() == 2)
                    .and_then(|token| u8::from_str_radix(token, 16).ok())
                    .map(PathPart::Byte)
                    .ok_or_else(|| {
                        Error::Entry(format!(
                            "path {path:?}: {token} is neither a hex byte nor a ParamN"
                        ))
                    })
            })
            .collect::<Result<Vec<PathPart>>>()
            .map(Path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathPart {
    Byte(u8),
    Param(u32),
}

/// What an assembly member holds: `Param1`, `Assem101`, `0x1F`, `"20 04 24 01"`
#[derive(Debug, Clone, PartialEq)]
pub enum MemberReference {
    Param(u32),
    Assem(u32),
    Number(i64),
    Path(Path),
}

/// `Param1` -> `Param(1)`, `Assem101` -> `Assem(101)`, `0x1F` -> `Number(31)`, `"24 01"` -> `Path`
impl FromField for MemberReference {
    fn from_field(text: &str) -> Result<MemberReference> {
        if let Some(number) = numbered(text, "Param") {
            Ok(MemberReference::Param(number))
        } else if let Some(number) = numbered(text, "Assem") {
            Ok(MemberReference::Assem(number))
        } else if text.contains('"') {
            Path::from_field(text).map(MemberReference::Path)
        } else {
            parse_int::parse(text)
                .map(MemberReference::Number)
                .map_err(|cause| {
                    Error::Entry(format!(
                        "expected a ParamN, AssemN, number or path, found {text:?}: {cause}"
                    ))
                })
        }
    }
}
