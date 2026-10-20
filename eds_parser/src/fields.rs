//! Reading the fields of an entry in file order: `Fields`, `FromFields` for a struct read from
//! consecutive fields, and `FromField` for the value of one field.

use anyhow::{Context, Result, anyhow, bail};

use crate::connection_manager::{ConnectionParameters, TriggerAndTransport};
use crate::params::{CipDataType, ParamDescriptor};
use crate::syntax::Entry;

/// A struct read from consecutive fields of an entry, each struct field from the next entry field
/// in the order the struct declares them. `#[derive(FromFields)]` implements it, and a field marked
/// `#[rest]` reads the rest of the entry as a `Vec`.
pub(crate) trait FromFields: Sized {
    fn from_fields(fields: &mut Fields) -> Result<Self>;
}

/// The fields of one entry, read one after another in file order. A field that does not read as
/// its type is an error naming the entry, the field and the line it is on:
///
/// ```text
/// Connection1: field 4
///
/// Caused by:
///     0: line 2389: Param999,x,Assem101,  $ O->T RPI, size, format
///     1: expected a number or a ParamN, found "x"
/// ```
pub(crate) struct Fields<'a> {
    entry: Entry<'a>,
    /// How many fields are read; the next one is field `read + 1`
    read: usize,
}

// ======= Start of Fields impl ========

impl<'a> Fields<'a> {
    pub fn new(entry: Entry<'a>) -> Fields<'a> {
        Fields { entry, read: 0 }
    }

    /// The next field, as the type it is assigned to: `0x20` as a `u16`, `Param1` as a
    /// `NumberOrParam`. A field past the end of a short entry is empty.
    pub fn next<T: FromField>(&mut self) -> Result<T> {
        let index = self.read;
        self.read += 1;
        T::from_field(&self.entry.text(index))
            .with_context(|| self.entry.line(index))
            .with_context(|| self.name(index))
    }

    /// A `T` after another up to the last non-empty field: the `16,Param1, 8,` member pairs of an
    /// `AssemN`
    pub fn rest<T: FromFields>(&mut self) -> Result<Vec<T>> {
        let mut values = Vec::new();
        while self.next_non_empty().is_some() {
            values.push(T::from_fields(self)?);
        }
        Ok(values)
    }

    /// Fails on a non-empty field past the last one read
    pub fn finish(self) -> Result<()> {
        match self.next_non_empty() {
            Some(index) => Err(anyhow!("is past the last field of this kind of entry"))
                .with_context(|| self.entry.line(index))
                .with_context(|| self.name(index)),
            None => Ok(()),
        }
    }

    /// The first non-empty field not yet read
    fn next_non_empty(&self) -> Option<usize> {
        (self.read..self.entry.field_count()).find(|&index| !self.entry.text(index).is_empty())
    }

    /// `Connection1: field 4`
    fn name(&self, index: usize) -> String {
        format!("{}: field {}", self.entry.keyword, index + 1)
    }
}

// ^^^^^^^^ End of Fields impl ^^^^^^^^

/// A value one field holds, read from the field's text: `0x20` as a `u16`, `Param1` as a
/// `NumberOrParam`, `"Input " "Assembly"` as a `String`
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

/// `4`, `0x04`, `0b100`
impl FromField for u8 {
    fn from_field(text: &str) -> Result<u8> {
        parse_int::parse(text).with_context(|| format!("expected a u8, found {text:?}"))
    }
}

/// `4`, `0x0004`, `0b100`
impl FromField for u16 {
    fn from_field(text: &str) -> Result<u16> {
        parse_int::parse(text).with_context(|| format!("expected a u16, found {text:?}"))
    }
}

/// `4`, `0x00000004`, `0b100`
impl FromField for u32 {
    fn from_field(text: &str) -> Result<u32> {
        parse_int::parse(text).with_context(|| format!("expected a u32, found {text:?}"))
    }
}

/// `FromField` for bitfields and enums read from an integer field: `TriggerAndTransport: u32`
/// reads `0x04010002` as a `u32`, then as a `TriggerAndTransport`
macro_rules! from_integer_field {
    ($($type:ty: $integer:ty),* $(,)?) => {$(
        impl FromField for $type {
            fn from_field(text: &str) -> Result<$type> {
                <$integer>::from_field(text).map(<$type>::from)
            }
        }
    )*};
}

from_integer_field! {
    ParamDescriptor: u16,
    CipDataType: u8,
    TriggerAndTransport: u32,
    ConnectionParameters: u32,
}

/// The N of a keyword that is `prefix` followed by digits (`Param12` -> 12), case ignored
pub(crate) fn numbered(keyword: &str, prefix: &str) -> Option<u32> {
    let (head, digits) = keyword.split_at_checked(prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
