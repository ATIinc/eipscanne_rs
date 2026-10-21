//! The text of an EDS file as sections and entries, by the grammar in `eds.pest`. An entry is one
//! `Keyword = field, field, ...;` statement over any number of lines, and a field is one of its
//! comma-separated values:
//!
//! ```text
//! [Assembly]                                          <- section
//!     Assem100 = "Input", "20 04 24 64 30 03", 32;    <- entry
//!                ^^^^^^^  ^^^^^^^^^^^^^^^^^^^  ^^     <- its three fields
//!     Assem101 = "Out" "put",     $ a comment, up to the end of the line
//!                "20 04 24 65 30 03",
//!                16;
//! ```

use pest::iterators::Pair;
use pest::{Parser, Position};
use pest_derive::Parser;

use crate::error::{Error, Result};
use crate::fields::{FromEntry, FromField};

#[derive(Parser)]
#[grammar = "eds.pest"]
struct EdsGrammar;

/// `[Connection Manager]` and its entries
pub(crate) struct Section<'a> {
    pub name: &'a str,
    pub entries: Vec<Entry<'a>>,
}

/// `Connection1 = 0x04010002, 0x77750405, ...;`, its fields read one after another in file order.
/// A field that does not read as its type is an error naming the entry, the field and the line it
/// is on: `Connection1: field 4: line 3: Param999,x,Assem101,: expected a number or a ParamN, found
/// "x": invalid digit found in string`.
pub(crate) struct Entry<'a> {
    pub keyword: &'a str,
    /// Each field as the grammar found it, with where it is in the file
    fields: Vec<Pair<'a, Rule>>,
    /// Just past the entry's `;`
    end: Position<'a>,
    /// How many fields are read; the next one is `fields[fields_read]`
    fields_read: usize,
}

// ======= Start of Entry impl ========

impl<'a> Entry<'a> {
    /// The entry as a `T`, its fields read in file order. A non-empty field after the last one
    /// `T` reads is an error.
    pub fn read<T: FromEntry>(mut self) -> Result<T> {
        let value = T::from_entry(&mut self)?;
        match self.next_non_empty() {
            Some(index) => Err(self.in_field(
                index,
                Error::Entry("is past the last field of this kind of entry".to_string()),
            )),
            None => Ok(value),
        }
    }

    /// The next field, as the type it is assigned to: `0x20` as a `u16`, `Param1` as a
    /// `NumberOrParam`. A field past the end of a short entry is empty.
    pub fn next<T: FromField>(&mut self) -> Result<T> {
        let index = self.fields_read;
        self.fields_read += 1;
        T::from_field(&self.field_text(index)).map_err(|error| self.in_field(index, error))
    }

    /// Whether a non-empty field is left to read
    pub fn has_more(&self) -> bool {
        self.next_non_empty().is_some()
    }

    /// The first non-empty field not yet read
    fn next_non_empty(&self) -> Option<usize> {
        (self.fields_read..self.fields.len()).find(|&index| !self.field_text(index).is_empty())
    }

    /// The text of a field: its strings, groups and bare text, without the comments between
    /// them. `"Input " $ note` and `"Assembly"` on the next line is `"Input " "Assembly"`. A field
    /// past the end of a short entry is empty.
    fn field_text(&self, index: usize) -> String {
        let Some(field) = self.fields.get(index) else {
            return String::new();
        };
        let pieces: Vec<&str> = field
            .clone()
            .into_inner()
            .map(|piece| piece.as_str())
            .collect();
        pieces.join(" ")
    }

    /// `error` in field `index`, with the line the field is on: `Connection1: field 4: line 2389:
    /// Param999,x,Assem101,  $ O->T RPI: ...`. A field past the end of a short entry is on the line
    /// of the entry's `;`.
    fn in_field(&self, index: usize, error: Error) -> Error {
        let position = match self.fields.get(index) {
            Some(field) => field.as_span().start_pos(),
            None => self.end,
        };
        error.context(format_args!(
            "{}: field {}: line {}: {}",
            self.keyword,
            index + 1,
            position.line_col().0,
            position.line_of().trim()
        ))
    }
}

// ^^^^^^^^ End of Entry impl ^^^^^^^^

/// Splits the text of an EDS file into its sections and entries. A syntax error names where it
/// is:
///
/// ```text
///  --> 3:3
///   |
/// 3 | L = 2;
///   |   ^---
///   |
///   = expected `,`, `;`, a string, a { } group, or a value
/// ```
pub(crate) fn sections(text: &str) -> Result<Vec<Section<'_>>> {
    Ok(EdsGrammar::parse(Rule::file, text)
        .map_err(|error| Error::Syntax(error.renamed_rules(rule_description).to_string()))?
        .flat_map(|file| file.into_inner())
        .filter(|pair| pair.as_rule() == Rule::section)
        .map(section)
        .collect())
}

fn section(pair: Pair<Rule>) -> Section {
    let mut children = pair.into_inner();
    let header = children
        .next()
        .expect("eds.pest: section = { header ~ entry* }");
    let name = header
        .into_inner()
        .next()
        .expect(r#"eds.pest: header = ${ "[" ~ name ~ "]" }"#);
    Section {
        name: name.as_str().trim(),
        entries: children.map(entry).collect(),
    }
}

fn entry(pair: Pair<Rule>) -> Entry {
    let end = pair.as_span().end_pos();
    let mut children = pair.into_inner();
    let keyword = children
        .next()
        .expect(r#"eds.pest: entry = { keyword ~ "=" ~ field ~ (comma ~ field)* ~ semicolon }"#);
    Entry {
        keyword: keyword.as_str(),
        fields: children
            .filter(|child| child.as_rule() == Rule::field)
            .collect(),
        end,
        fields_read: 0,
    }
}

/// How a syntax error names what it expected
fn rule_description(rule: &Rule) -> String {
    match rule {
        Rule::file | Rule::section | Rule::header => "a [Section] header",
        Rule::EOI => "the end of the file",
        Rule::keyword | Rule::entry => "a keyword",
        Rule::comma => "`,`",
        Rule::semicolon => "`;`",
        Rule::string | Rule::characters | Rule::strings => "a string",
        Rule::group => "a { } group",
        Rule::field | Rule::bare | Rule::name => "a value",
        Rule::WHITESPACE | Rule::COMMENT => "whitespace or a $ comment",
    }
    .to_string()
}

/// What is inside each quoted string of a field's text, when it is only quoted strings:
/// `"Input " L"Assembly"` is `Input ` and `Assembly`, and empty text is none of them
pub(crate) fn quoted_strings(text: &str) -> Option<Vec<&str>> {
    let strings = EdsGrammar::parse(Rule::strings, text).ok()?;
    Some(
        strings
            .flatten()
            .filter(|pair| pair.as_rule() == Rule::characters)
            .map(|pair| pair.as_str())
            .collect(),
    )
}
