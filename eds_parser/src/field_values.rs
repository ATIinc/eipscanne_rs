//! The kinds of value an EDS field holds, each read from the field's text with `FromStr`. Plain
//! integer fields (`u8`, `u16`, `u32`) are read with `number`.

use std::str::FromStr;

use serde_with::DeserializeFromStr;

use crate::error::Error;

/// A quoted string, or adjacent quoted strings joined: `"Input " "Assembly"` is `Input Assembly`.
/// `\n`, `\t` and `\r` are read as such, and a backslash before any other character keeps only that
/// character (`"a \"b\""` is `a "b"`).
#[derive(Debug, Clone, PartialEq, Default, DeserializeFromStr)]
pub struct QuotedString(pub String);

/// A number, or a `ParamN` that stands for its default: `32`, `0x20`, `Param3`
#[derive(Debug, Clone, Copy, PartialEq, DeserializeFromStr)]
pub enum NumberOrParam {
    Number(i64),
    Param(u32),
}

/// The data format of a connection direction: `Assem100`, `AssemExa100`, `Param4`
#[derive(Debug, Clone, Copy, PartialEq, DeserializeFromStr)]
pub enum DataFormat {
    Assem(u32),
    AssemExa(u32),
    Param(u32),
}

/// A value in a param's data type, its minimum, maximum or default: `1000`, `0x97`, `2.5`, `"on"`
#[derive(Debug, Clone, PartialEq, DeserializeFromStr)]
pub enum ParamValue {
    Integer(i64),
    Real(f64),
    Text(String),
}

/// A path string: hex bytes and `ParamN` references, written with or without brackets, each
/// standing for the param's default: `"20 04 24 [Param2] 2C 96 2C 64"`
#[derive(Debug, Clone, PartialEq, DeserializeFromStr)]
pub struct PathTokens(pub Vec<PathToken>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathToken {
    Byte(u8),
    Param(u32),
}

/// What an assembly member holds: `Param1`, `Assem101`, `0x1F`, `"20 04 24 01"`
#[derive(Debug, Clone, PartialEq, DeserializeFromStr)]
pub enum MemberReference {
    Param(u32),
    Assem(u32),
    Number(i64),
    Path(PathTokens),
}

// ======= Start of FromStr impls ========

/// `"Input " "Assembly"` -> `QuotedString("Input Assembly")`
impl FromStr for QuotedString {
    type Err = Error;

    fn from_str(text: &str) -> Result<QuotedString, Error> {
        // `L"..."` is a 16-bit character string, read the same way
        let quoted = text.strip_prefix('L').unwrap_or(text);
        if !quoted.starts_with('"') {
            return Err(Error::Value(format!(
                "expected a quoted string, found {text:?}"
            )));
        }

        let mut string = String::new();
        let mut in_string = false;
        let mut chars = quoted.chars();
        while let Some(c) = chars.next() {
            match (in_string, c) {
                (false, '"') => in_string = true,
                (false, c) if c.is_whitespace() => {}
                (false, _) => {
                    return Err(Error::Value(format!(
                        "expected a quoted string, found {text:?}"
                    )));
                }
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
            return Err(Error::Value(format!("{text} has no closing quote")));
        }
        Ok(QuotedString(string))
    }
}

/// `0x20` -> `Number(32)`, `Param3` -> `Param(3)`
impl FromStr for NumberOrParam {
    type Err = Error;

    fn from_str(text: &str) -> Result<NumberOrParam, Error> {
        if let Some(param) = numbered(text, "Param") {
            return Ok(NumberOrParam::Param(param));
        }
        number(text)
            .and_then(|value| i64::try_from(value).ok())
            .map(NumberOrParam::Number)
            .ok_or_else(|| Error::Value(format!("expected a number or a ParamN, found {text:?}")))
    }
}

/// `Assem100` -> `Assem(100)`, `AssemExa7` -> `AssemExa(7)`, `Param4` -> `Param(4)`
impl FromStr for DataFormat {
    type Err = Error;

    fn from_str(text: &str) -> Result<DataFormat, Error> {
        if let Some(number) = numbered(text, "AssemExa") {
            Ok(DataFormat::AssemExa(number))
        } else if let Some(number) = numbered(text, "Assem") {
            Ok(DataFormat::Assem(number))
        } else if let Some(number) = numbered(text, "Param") {
            Ok(DataFormat::Param(number))
        } else {
            Err(Error::Value(format!(
                "expected an AssemN, AssemExaN or ParamN, found {text:?}"
            )))
        }
    }
}

/// `0x97` -> `Integer(151)`, `2.5` -> `Real(2.5)`, `"on"` -> `Text("on")`
impl FromStr for ParamValue {
    type Err = Error;

    fn from_str(text: &str) -> Result<ParamValue, Error> {
        if let Some(integer) = number(text).and_then(|value| i64::try_from(value).ok()) {
            return Ok(ParamValue::Integer(integer));
        }
        if let Ok(real) = text.parse::<f64>() {
            return Ok(ParamValue::Real(real));
        }
        match text.parse::<QuotedString>() {
            Ok(QuotedString(string)) => Ok(ParamValue::Text(string)),
            Err(_) => Err(Error::Value(format!(
                "expected a number or a quoted string, found {text:?}"
            ))),
        }
    }
}

/// `"20 04 [Param2]"` -> `[Byte(0x20), Byte(0x04), Param(2)]`
impl FromStr for PathTokens {
    type Err = Error;

    fn from_str(text: &str) -> Result<PathTokens, Error> {
        let QuotedString(path) = text.parse()?;
        path.split_whitespace()
            .map(|token| {
                let reference = token.trim_start_matches('[').trim_end_matches(']');
                if let Some(param) = numbered(reference, "Param") {
                    return Ok(PathToken::Param(param));
                }
                Some(token)
                    .filter(|token| token.len() == 2)
                    .and_then(|token| u8::from_str_radix(token, 16).ok())
                    .map(PathToken::Byte)
                    .ok_or_else(|| {
                        Error::Value(format!(
                            "path {path:?}: {token} is neither a hex byte nor a ParamN"
                        ))
                    })
            })
            .collect::<Result<Vec<PathToken>, Error>>()
            .map(PathTokens)
    }
}

/// `Param1` -> `Param(1)`, `Assem101` -> `Assem(101)`, `0x1F` -> `Number(31)`, `"24 01"` -> `Path`
impl FromStr for MemberReference {
    type Err = Error;

    fn from_str(text: &str) -> Result<MemberReference, Error> {
        if let Some(number) = numbered(text, "Param") {
            Ok(MemberReference::Param(number))
        } else if let Some(number) = numbered(text, "Assem") {
            Ok(MemberReference::Assem(number))
        } else if text.contains('"') {
            text.parse().map(MemberReference::Path)
        } else {
            number(text)
                .and_then(|value| i64::try_from(value).ok())
                .map(MemberReference::Number)
                .ok_or_else(|| {
                    Error::Value(format!(
                        "expected a ParamN, AssemN, number or path, found {text:?}"
                    ))
                })
        }
    }
}

// ^^^^^^^^ End of FromStr impls ^^^^^^^^

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_decimal_hex_or_binary() {
        assert_eq!(number("228"), Some(228));
        assert_eq!(number("-1"), Some(-1));
        assert_eq!(number("0x0401"), Some(0x0401));
        assert_eq!(number("0b101"), Some(5));
        assert_eq!(number("1.6"), None);
        assert_eq!(numbered("PARAM12", "Param"), Some(12));
        assert_eq!(numbered("AssemExa5", "Assem"), None);
    }

    #[test]
    fn text_joins_and_unescapes() {
        assert_eq!(
            "\"AAA\"\n  \"BBB\"".parse(),
            Ok(QuotedString("AAABBB".to_string()))
        );
        assert_eq!("L\"wide\"".parse(), Ok(QuotedString("wide".to_string())));
        assert_eq!(
            r#""a \"quote\" and \\""#.parse(),
            Ok(QuotedString(r#"a "quote" and \"#.to_string()))
        );
        assert_eq!(
            "Word".parse::<QuotedString>(),
            Err(Error::Value(
                "expected a quoted string, found \"Word\"".to_string()
            ))
        );
    }

    #[test]
    fn references_and_values_read_their_kind() {
        assert_eq!("Param999".parse(), Ok(NumberOrParam::Param(999)));
        assert_eq!("0x2710".parse(), Ok(NumberOrParam::Number(10_000)));
        assert_eq!("AssemExa7".parse(), Ok(DataFormat::AssemExa(7)));
        assert_eq!("Assem100".parse(), Ok(DataFormat::Assem(100)));
        assert!("Rx".parse::<DataFormat>().is_err());
        assert_eq!("0x97".parse(), Ok(ParamValue::Integer(0x97)));
        assert_eq!("2.5".parse(), Ok(ParamValue::Real(2.5)));
        assert_eq!("\"on\"".parse(), Ok(ParamValue::Text("on".to_string())));
        assert_eq!("Param1".parse(), Ok(MemberReference::Param(1)));
    }

    #[test]
    fn paths_hold_bytes_and_params() {
        assert_eq!(
            "\"20 04 24 [Param1] 2C Param2\"".parse(),
            Ok(PathTokens(vec![
                PathToken::Byte(0x20),
                PathToken::Byte(0x04),
                PathToken::Byte(0x24),
                PathToken::Param(1),
                PathToken::Byte(0x2C),
                PathToken::Param(2),
            ]))
        );
        assert_eq!(
            "\"20 04 2\"".parse::<PathTokens>(),
            Err(Error::Value(
                "path \"20 04 2\": 2 is neither a hex byte nor a ParamN".to_string()
            ))
        );
    }
}
