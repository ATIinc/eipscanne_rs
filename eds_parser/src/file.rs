//! The text of an EDS file as sections and entries, and the serde `Deserializer` that reads an
//! entry into a struct:
//!
//! ```text
//! $ a comment, up to the end of the line
//! [Section Name]
//!     Keyword = field, field, "a string", "joined " "to it",
//!               field on the next line;
//! ```
//!
//! A field is kept as its raw text (`0x0004`, `Param1`, `"20 04 24 01"`) until an entry struct is
//! read from it. Quoted strings may hold `,`, `;` and `$`, and a `{ ... }` group is one field.
//!
//! `ConnectionEntry::deserialize(&entry)` reads each field of the entry into the next field of the
//! struct, as its type asks:
//!
//! * an integer (`u8`, `u16`, `u32`, ...) from decimal, `0x` or `0b` text;
//! * an `Option` as `None` when the field is empty;
//! * `IgnoredAny` without looking at it;
//! * a `Vec` from the rest of the entry's fields, each element taking one field, or as many as it
//!   has fields when it is a struct;
//! * anything else through its `FromStr`.
//!
//! Fields missing from the end of a short entry read as empty; a non-empty field after the
//! struct's last one is an error.

use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;

use crate::error::{Error, Result};
use crate::field_values::number;

/// `[name]` followed by its entries: `[Connection Manager]`, `[Params]`
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub name: String,
    pub entries: Vec<Entry>,
}

/// `keyword = field, field, ...;`: `Assem151 = "Configuration", "20 04 24 97 30 03", 0, 0x0000;`
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub keyword: String,
    /// The text of each field, trimmed; `""` for an empty field
    pub fields: Vec<String>,
}

/// The fields of an entry, read one after another into a struct. After `16,Param1,` of
/// `Assem100 = "In", "20 04 24 64 30 03", 5, 0x0000, , , 16,Param1, 8,;`, `read` is 8.
struct EntryFields {
    fields: Vec<String>,
    /// How many fields are read; the next one is field `read + 1`
    read: usize,
}

/// Splits the text of an EDS file into its sections and entries:
///
/// ```text
/// [Assembly]                              Section { name: "Assembly", entries: [
///     Assem151 = "Config",    $ name          Entry { keyword: "Assem151",
///                "20 04 24 97 30 03",                 fields: ["\"Config\"",
///                0, 0x0000;                                    "\"20 04 24 97 30 03\"", "0",
///                                                              "0x0000"] } ] }
/// ```
pub(crate) fn sections(text: &str) -> Result<Vec<Section>> {
    let text = without_comments(text);
    // Every piece but the last is the text of one entry, after the headers of any sections that
    // start before it
    let pieces = split_outside_strings(&text, ';');
    let last = pieces.len() - 1;

    let mut sections: Vec<Section> = Vec::new();
    for (index, piece) in pieces.into_iter().enumerate() {
        let mut rest = piece.trim_start();
        while let Some(header) = rest.strip_prefix('[') {
            let (name, after) = header
                .split_once(']')
                .ok_or_else(|| syntax_error(rest, "the section name has no closing ]"))?;
            sections.push(Section {
                name: name.trim().to_string(),
                entries: Vec::new(),
            });
            rest = after.trim_start();
        }
        if rest.is_empty() {
            continue;
        }

        let (keyword, fields) = rest
            .split_once('=')
            .ok_or_else(|| syntax_error(rest, "the entry has no ="))?;
        // Without its `;`, an entry runs into the next entry or section
        if index == last || outside_strings(fields).any(|(_, c)| c == '=' || c == '[') {
            return Err(syntax_error(keyword, "the entry has no closing ;"));
        }
        let section = sections
            .last_mut()
            .ok_or_else(|| syntax_error(keyword, "the entry comes before the first [Section]"))?;
        section.entries.push(Entry {
            keyword: keyword.trim().to_string(),
            fields: split_outside_strings(fields, ',')
                .into_iter()
                .map(|field| field.trim().to_string())
                .collect(),
        });
    }
    Ok(sections)
}

// ======= Start of Entry Deserializer impl ========

/// Reads the entry into a struct. An error names the entry and the field:
///
/// ```text
/// Connection1 = 0x04010002, 0x44440405, Param1,x,Assem150, ...;
/// Connection1: field 4: expected a number or a ParamN, found "x"
/// ```
impl<'de> de::Deserializer<'de> for &Entry {
    type Error = Error;

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        struct_fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let mut fields = EntryFields {
            fields: self.fields.clone(),
            read: 0,
        };
        let in_field = |field: usize, error: Error| Error::InEntry {
            keyword: self.keyword.clone(),
            field: Some(field),
            error: Box::new(error),
        };

        // Every error comes from reading a field, after `read` counts it
        let value = (&mut fields)
            .deserialize_struct(name, struct_fields, visitor)
            .map_err(|error| in_field(fields.read, error))?;

        let unread = &fields.fields[fields.read..];
        if let Some(extra) = unread.iter().position(|field| !field.is_empty()) {
            return Err(in_field(fields.read + extra + 1, Error::ExtraField));
        }
        Ok(value)
    }

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::Value("an entry reads into a struct".to_string()).in_entry(&self.keyword))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier
        ignored_any
    }
}

// ^^^^^^^^ End of Entry Deserializer impl ^^^^^^^^

// ======= Start of EntryFields impl ========

impl EntryFields {
    /// Reads the next field with `read`
    fn next_field<T>(&mut self, read: impl FnOnce(&str) -> Result<T>) -> Result<T> {
        let text = self.fields.get(self.read).map_or("", String::as_str);
        self.read += 1;
        read(text)
    }

    /// Whether the next field is empty: `,,` or `;`
    fn next_is_empty(&self) -> bool {
        self.fields
            .get(self.read)
            .is_none_or(|field| field.is_empty())
    }
}

/// Reads the next value: one field for an integer, an `Option`, `IgnoredAny` or a `FromStr` type,
/// a struct's fields for a struct, the rest of the fields for a `Vec`. `Param1,32,Assem150,` is
/// `Some(Param(1))`, `Some(Number(32))` and `Some(Assem(150))` for three `Option` fields.
impl<'de> de::Deserializer<'de> for &mut EntryFields {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| visitor.visit_str(text))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.next_is_empty() {
            true => self.next_field(|_| visitor.visit_none()),
            false => visitor.visit_some(self),
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|_| visitor.visit_unit())
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| integer(text, visitor))
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| integer(text, visitor))
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| integer(text, visitor))
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| integer(text, visitor))
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.next_field(|text| integer(text, visitor))
    }

    /// The fields missing from the end of a short entry are added as empty ones:
    /// `Connection1 = 0x04010002, 0x44440405;` reads as all 17 fields of a `ConnectionEntry`
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        struct_fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let end = self.read + struct_fields.len();
        if self.fields.len() < end {
            self.fields.resize(end, String::new());
        }
        visitor.visit_seq(self)
    }

    /// The empty fields at the end of the entry are not elements: `16,Param1, 8,,,;` is two
    /// `AssemblyMember`s
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let end = self
            .fields
            .iter()
            .rposition(|field| !field.is_empty())
            .map_or(0, |last| last + 1);
        self.fields.truncate(end.max(self.read));
        visitor.visit_seq(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i128 u128 f32 f64 char str string bytes byte_buf unit unit_struct
        newtype_struct tuple tuple_struct map enum identifier
    }
}

/// The fields of a struct or the elements of a `Vec`, until the entry's fields run out
impl<'de> SeqAccess<'de> for EntryFields {
    type Error = Error;

    fn next_element_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<Option<S::Value>> {
        match self.read < self.fields.len() {
            true => seed.deserialize(self).map(Some),
            false => Ok(None),
        }
    }
}

// ^^^^^^^^ End of EntryFields impl ^^^^^^^^

/// An integer field: `32`, `0x20`, `0b100000`, `-1`
fn integer<'de, V: Visitor<'de>>(text: &str, visitor: V) -> Result<V::Value> {
    match number(text) {
        Some(value) if value >= 0 => match u64::try_from(value) {
            Ok(value) => visitor.visit_u64(value),
            Err(_) => Err(Error::Value(format!("{value} is too large"))),
        },
        Some(value) => match i64::try_from(value) {
            Ok(value) => visitor.visit_i64(value),
            Err(_) => Err(Error::Value(format!("{value} is too small"))),
        },
        None => Err(Error::Value(format!("expected a number, found {text:?}"))),
    }
}

/// The text without its `$` comments
fn without_comments(text: &str) -> String {
    text.lines()
        .map(
            |line| match outside_strings(line).find(|&(_, c)| c == '$') {
                Some((position, _)) => &line[..position],
                None => line,
            },
        )
        .collect::<Vec<&str>>()
        .join("\n")
}

/// `text` cut at every `separator` outside quoted strings and `{ }` groups
fn split_outside_strings(text: &str, separator: char) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (position, c) in outside_strings(text) {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if c == separator && depth == 0 => {
                pieces.push(&text[start..position]);
                start = position + c.len_utf8();
            }
            _ => {}
        }
    }
    pieces.push(&text[start..]);
    pieces
}

/// The characters of `text` outside quoted strings, with their byte positions. A `\"` inside a
/// string does not end it.
fn outside_strings(text: &str) -> impl Iterator<Item = (usize, char)> {
    let mut in_string = false;
    let mut escaped = false;
    text.char_indices().filter(move |&(_, c)| {
        if !in_string {
            in_string = c == '"';
            return !in_string;
        }
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => in_string = false,
            _ => {}
        }
        false
    })
}

/// A syntax error, named by the first line of the text it is in: `K 1: the entry has no =`
fn syntax_error(text: &str, message: &'static str) -> Error {
    Error::Syntax {
        text: text.lines().next().unwrap_or_default().trim().to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde::de::IgnoredAny;

    use super::*;
    use crate::field_values::{NumberOrParam, QuotedString};

    /// `Sample1 = 0x20, Param1, anything, "n", 7;`
    #[derive(Debug, PartialEq, Deserialize)]
    struct Sample {
        size: u16,
        rpi: Option<NumberOrParam>,
        skipped: IgnoredAny,
        name: QuotedString,
        trailing: Option<u8>,
    }

    /// `Sample1 = "n", 1,2, 3,;`
    #[derive(Debug, PartialEq, Deserialize)]
    struct WithPairs {
        name: QuotedString,
        pairs: Vec<Pair>,
    }

    /// `1,2,` of `Sample1 = "n", 1,2, 3,;`
    #[derive(Debug, PartialEq, Deserialize)]
    struct Pair {
        first: u8,
        second: Option<u8>,
    }

    fn only_entry(text: &str) -> Entry {
        sections(text).unwrap()[0].entries[0].clone()
    }

    fn sample(fields: &str) -> Entry {
        only_entry(&format!("[S]\nSample1 = {fields};\n"))
    }

    #[test]
    fn sections_and_entries_keep_file_order() {
        let sections =
            sections("[File]\nDescText = \"x\";\n\n[TCP/IP Interface Class]\nA = 1;\nB =\n 2;\n")
                .unwrap();

        let names: Vec<&str> = sections
            .iter()
            .map(|section| section.name.as_str())
            .collect();
        assert_eq!(names, vec!["File", "TCP/IP Interface Class"]);
        let keywords: Vec<&str> = sections[1]
            .entries
            .iter()
            .map(|entry| entry.keyword.as_str())
            .collect();
        assert_eq!(keywords, vec!["A", "B"]);
    }

    #[test]
    fn fields_keep_their_raw_text_and_empty_positions() {
        let entry = only_entry("[S]\nK = 0x0401, ,Param1, \"a, b; $c\" \"d\",{1, {2, 3}},;\n");
        assert_eq!(
            entry.fields,
            vec![
                "0x0401",
                "",
                "Param1",
                "\"a, b; $c\" \"d\"",
                "{1, {2, 3}}",
                ""
            ]
        );
    }

    #[test]
    fn comments_and_line_breaks_may_sit_between_fields() {
        let entry = only_entry(
            "$ generated\r\n[S]\r\nK =\r\n  1,   $ first\r\n  $ a whole comment line\r\n  \"two\\\"$\", $ second\r\n  ;\r\n",
        );
        assert_eq!(entry.keyword, "K");
        assert_eq!(entry.fields, vec!["1", "\"two\\\"$\"", ""]);
    }

    #[test]
    fn syntax_errors_name_where_they_are() {
        let error = |text: &str| sections(text).unwrap_err().to_string();

        assert_eq!(error("[S]\nK = 1\n"), "K: the entry has no closing ;");
        assert_eq!(
            error("[S]\nK = 1\n[T]\nL = 2;\n"),
            "K: the entry has no closing ;"
        );
        assert_eq!(error("[S]\n\nK 1;\n"), "K 1: the entry has no =");
        assert_eq!(
            error("K = 1;\n"),
            "K: the entry comes before the first [Section]"
        );
        assert_eq!(
            error("[S\nK = 1;\n"),
            "[S: the section name has no closing ]"
        );
    }

    #[test]
    fn fields_fill_the_struct_in_order() {
        assert_eq!(
            Sample::deserialize(&sample("0x20, Param1, anything, \"n\"")),
            Ok(Sample {
                size: 32,
                rpi: Some(NumberOrParam::Param(1)),
                skipped: IgnoredAny,
                name: QuotedString("n".to_string()),
                trailing: None,
            }),
            "a missing trailing field reads as empty"
        );
        assert_eq!(
            Sample::deserialize(&sample("1, , , \"n\", , ,")).map(|sample| sample.rpi),
            Ok(None),
            "an empty field is None, and empty extra fields are allowed"
        );
    }

    #[test]
    fn a_vec_reads_the_rest_of_the_fields() {
        let pairs = |fields: &str| WithPairs::deserialize(&sample(fields)).map(|entry| entry.pairs);

        assert_eq!(
            pairs("\"n\", 1,2, 3,, , ,"),
            Ok(vec![
                Pair {
                    first: 1,
                    second: Some(2)
                },
                Pair {
                    first: 3,
                    second: None
                },
            ])
        );
        assert_eq!(pairs("\"n\""), Ok(vec![]));
        assert_eq!(
            pairs("\"n\", 1,2, x,4").unwrap_err().to_string(),
            "Sample1: field 4: expected a number, found \"x\""
        );
    }

    #[test]
    fn errors_name_the_entry_and_the_field() {
        let error = |fields: &str| {
            Sample::deserialize(&sample(fields))
                .unwrap_err()
                .to_string()
        };

        assert_eq!(
            error("x"),
            "Sample1: field 1: expected a number, found \"x\""
        );
        assert_eq!(
            error("70000"),
            "Sample1: field 1: invalid value: integer `70000`, expected u16"
        );
        assert_eq!(
            error("1, Rx"),
            "Sample1: field 2: expected a number or a ParamN, found \"Rx\""
        );
        assert_eq!(
            error("1, , , "),
            "Sample1: field 4: expected a quoted string, found \"\""
        );
        assert_eq!(
            error("1, , , \"n\", , 7"),
            "Sample1: field 6: is past the last field of this kind of entry"
        );
    }
}
