//! Reading the fields of an entry (see `syntax` for entries and fields): `FromEntry` for a struct
//! read from consecutive fields, `FromField` for a value read from one field, and the values a
//! field holds in more than one section: `NumberOrParam` and `Path`.

use crate::error::{Error, Result};
use crate::syntax::{Entry, quoted_strings};

/// A struct read from consecutive fields of an entry, each member in the order the struct declares
/// them. `#[derive(FromEntry)]` implements it.
pub(crate) trait FromEntry: Sized {
    fn from_entry(entry: &mut Entry) -> Result<Self>;
}

/// A member read from one field reads the next field
impl<T: FromField> FromEntry for T {
    fn from_entry(entry: &mut Entry) -> Result<T> {
        entry.next()
    }
}

/// The rest of the entry: one `T` after another up to the last non-empty field, such as the
/// `16,Param1, 8,` members of an `AssemN`. A `Vec` is the last member of its struct.
impl<T: FromEntry> FromEntry for Vec<T> {
    fn from_entry(entry: &mut Entry) -> Result<Vec<T>> {
        let mut values = Vec::new();
        while entry.has_more() {
            values.push(T::from_entry(entry)?);
        }
        Ok(values)
    }
}

/// A value read from the text of one field: `0x20` as a `u16`, `Param1` as a `NumberOrParam`,
/// `"Input " "Assembly"` as a `String`. `#[derive(FromField)]` implements it for a `bilge`
/// bitfield or enum, read as its backing integer: `0x04010002` as a `TriggerAndTransportMask`.
pub(crate) trait FromField: Sized {
    fn from_field(text: &str) -> Result<Self>;
}

/// `,,` is `None`, anything else a `T`
impl<T: FromField> FromField for Option<T> {
    fn from_field(text: &str) -> Result<Option<T>> {
        match text.is_empty() {
            true => Ok(None),
            false => T::from_field(text).map(Some),
        }
    }
}

/// A field this crate does not read: the link path of a `ParamN`, the configuration of a
/// `ConnectionN`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unread;

impl FromField for Unread {
    fn from_field(_text: &str) -> Result<Unread> {
        Ok(Unread)
    }
}

/// A quoted string, or adjacent quoted strings joined: `"Input " "Assembly"` is `Input Assembly`,
/// and `,,` is empty. Escapes such as `\n`, `\"` and `\x41` are read, and a malformed one is kept
/// as written. `L"..."`, a 16-bit character string, is read the same way.
impl FromField for String {
    fn from_field(text: &str) -> Result<String> {
        let strings = quoted_strings(text)
            .ok_or_else(|| Error::Entry(format!("expected a quoted string, found {text:?}")))?;
        Ok(strings
            .iter()
            .map(|characters| unescaper::unescape_lossy(characters))
            .collect())
    }
}

/// `4`, `0x04`, `0b100`
impl FromField for u8 {
    fn from_field(text: &str) -> Result<u8> {
        parse_int::parse(text)
            .map_err(|cause| Error::Entry(format!("expected a u8, found {text:?}: {cause}")))
    }
}

/// `4`, `0x0004`, `0b100`
impl FromField for u16 {
    fn from_field(text: &str) -> Result<u16> {
        parse_int::parse(text)
            .map_err(|cause| Error::Entry(format!("expected a u16, found {text:?}: {cause}")))
    }
}

/// `4`, `0x00000004`, `0b100`
impl FromField for u32 {
    fn from_field(text: &str) -> Result<u32> {
        parse_int::parse(text)
            .map_err(|cause| Error::Entry(format!("expected a u32, found {text:?}: {cause}")))
    }
}

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

/// The N of a keyword that is `prefix` followed by digits (`Param12` -> 12), case ignored
pub(crate) fn numbered(keyword: &str, prefix: &str) -> Option<u32> {
    let (head, digits) = keyword.split_at_checked(prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
