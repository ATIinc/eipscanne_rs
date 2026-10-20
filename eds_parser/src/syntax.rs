//! The text of an EDS file as sections and entries, by the grammar in `eds.pest`:
//!
//! ```text
//! $ a comment, up to the end of the line
//! [Section Name]
//!     Keyword = field, field, "a string", "joined " "to it",
//!               field on the next line;
//! ```

use anyhow::Result;
use pest::Parser;
use pest::Position;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::fields::{Fields, FromFields};

#[derive(Parser)]
#[grammar = "eds.pest"]
struct EdsGrammar;

/// `[Connection Manager]` and its entries
pub(crate) struct Section<'a> {
    pub name: &'a str,
    pub entries: Vec<Entry<'a>>,
}

/// `Connection1 = 0x04010002, 0x77750405, ...;`
pub(crate) struct Entry<'a> {
    pub keyword: &'a str,
    /// Each field as the grammar found it, with where it is in the file
    fields: Vec<Pair<'a, Rule>>,
    /// Where the entry's `;` is
    end: Position<'a>,
}

// ======= Start of Entry impl ========

impl Entry<'_> {
    /// The entry as a `T`, its fields read in file order. A non-empty field after the last one
    /// `T` reads is an error.
    pub fn read<T: FromFields>(self) -> Result<T> {
        let mut fields = Fields::new(self);
        let value = T::from_fields(&mut fields)?;
        fields.finish()?;
        Ok(value)
    }

    /// How many fields the entry has, empty ones included
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// The text of a field: its strings, groups and bare text, without the comments between
    /// them. `"Input " $ note` and `"Assembly"` on the next line is `"Input " "Assembly"`. A field
    /// past the end of a short entry is empty.
    pub fn text(&self, index: usize) -> String {
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

    /// The line a field is on, with its comment: `line 2389: Param999,x,Assem101,  $ O->T RPI`.
    /// A field past the end of a short entry is on the line of the entry's `;`.
    pub fn line(&self, index: usize) -> String {
        let position = match self.fields.get(index) {
            Some(field) => field.as_span().start_pos(),
            None => self.end,
        };
        format!(
            "line {}: {}",
            position.line_col().0,
            position.line_of().trim()
        )
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
    let file = EdsGrammar::parse(Rule::file, text)
        .map_err(|error| error.renamed_rules(rule_description))?
        .next()
        .expect("the grammar matches one file");
    Ok(file
        .into_inner()
        .filter(|pair| pair.as_rule() == Rule::section)
        .map(section)
        .collect())
}

fn section(pair: Pair<Rule>) -> Section {
    let mut children = pair.into_inner();
    let header = children.next().expect("a section starts with its header");
    let name = header
        .into_inner()
        .next()
        .expect("a header holds the section's name");
    Section {
        name: name.as_str().trim(),
        entries: children.map(entry).collect(),
    }
}

fn entry(pair: Pair<Rule>) -> Entry {
    let mut children = pair.into_inner();
    let keyword = children.next().expect("an entry starts with its keyword");
    let semicolon = children
        .clone()
        .find(|child| child.as_rule() == Rule::semicolon)
        .expect("an entry ends with its ;");
    Entry {
        keyword: keyword.as_str(),
        fields: children
            .filter(|child| child.as_rule() == Rule::field)
            .collect(),
        end: semicolon.as_span().start_pos(),
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
        Rule::string => "a string",
        Rule::group => "a { } group",
        Rule::field | Rule::bare | Rule::name => "a value",
        Rule::WHITESPACE | Rule::COMMENT => "whitespace or a $ comment",
    }
    .to_string()
}
