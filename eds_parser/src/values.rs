//! The kinds of value an EDS field holds, each read from the field's text with `FromField`.

use anyhow::{Context, Result, anyhow, bail};

/// A value one field holds, read from the field's text: `0x20` as a `u16`, `Param1` as a
/// `NumberOrParam`, `"Input " "Assembly"` as a `String`. `Option<T>` is `None` for an empty field.
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

/// A quoted string, or adjacent quoted strings joined: `"Input " "Assembly"` is `Input Assembly`.
/// `\n`, `\t` and `\r` are read as such, and a backslash before any other character keeps only that
/// character (`"a \"b\""` is `a "b"`). `L"..."`, a 16-bit character string, is read the same way.
impl FromField for String {
    fn from_field(text: &str) -> Result<String> {
        let quoted = text.strip_prefix('L').unwrap_or(text);
        if !quoted.starts_with('"') {
            bail!("expected a quoted string, found {text:?}");
        }

        let mut string = String::new();
        let mut in_string = false;
        let mut chars = quoted.chars();
        while let Some(c) = chars.next() {
            match (in_string, c) {
                (false, '"') => in_string = true,
                (false, c) if c.is_whitespace() => {}
                (false, _) => bail!("expected a quoted string, found {text:?}"),
                (true, '"') => in_string = false,
                (true, '\\') => string.push(match chars.next() {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('r') => '\r',
                    Some(other) => other,
                    None => '\\',
                }),
                (true, c) => string.push(c),
            }
        }
        if in_string {
            bail!("{text} has no closing quote");
        }
        Ok(string)
    }
}

/// `4`
impl FromField for u8 {
    fn from_field(text: &str) -> Result<u8> {
        integer(text)
    }
}

/// `0x0004`
impl FromField for u16 {
    fn from_field(text: &str) -> Result<u16> {
        integer(text)
    }
}

/// `0x04010002`
impl FromField for u32 {
    fn from_field(text: &str) -> Result<u32> {
        integer(text)
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
        number(text)
            .and_then(|value| i64::try_from(value).ok())
            .map(NumberOrParam::Number)
            .with_context(|| format!("expected a number or a ParamN, found {text:?}"))
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
            bail!("expected an AssemN, AssemExaN or ParamN, found {text:?}")
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
        if let Some(integer) = number(text).and_then(|value| i64::try_from(value).ok()) {
            return Ok(ParamValue::Integer(integer));
        }
        if let Ok(real) = text.parse::<f64>() {
            return Ok(ParamValue::Real(real));
        }
        String::from_field(text)
            .map(ParamValue::Text)
            .map_err(|_| anyhow!("expected a number or a quoted string, found {text:?}"))
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
                    .with_context(|| {
                        format!("path {path:?}: {token} is neither a hex byte nor a ParamN")
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
            number(text)
                .and_then(|value| i64::try_from(value).ok())
                .map(MemberReference::Number)
                .with_context(|| {
                    format!("expected a ParamN, AssemN, number or path, found {text:?}")
                })
        }
    }
}

/// An integer field as a `T`: `32`, `0x20`, `0b100000`
fn integer<T: TryFrom<i128>>(text: &str) -> Result<T> {
    let value = number(text).with_context(|| format!("expected a number, found {text:?}"))?;
    T::try_from(value)
        .map_err(|_| anyhow!("{value} is out of range for {}", std::any::type_name::<T>()))
}

/// A decimal (`-12`), `0x` hexadecimal (`0x0C`) or `0b` binary (`0b1100`) number
pub(crate) fn number(text: &str) -> Option<i128> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i128::from_str_radix(hex, 16).ok()
    } else if let Some(binary) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        i128::from_str_radix(binary, 2).ok()
    } else {
        text.parse().ok()
    }
}

/// The N of a keyword that is `prefix` followed by digits (`Param12` -> 12), case ignored
pub(crate) fn numbered(keyword: &str, prefix: &str) -> Option<u32> {
    let (head, digits) = keyword.split_at_checked(prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
