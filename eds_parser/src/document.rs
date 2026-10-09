//! Step 2: the shape of an EDS file, with no EDS meaning attached. This is the only module that
//! touches pest's parse tree, and the one that words errors about a field.

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::error::{Error, Result};

#[derive(Parser)]
#[grammar = "eds.pest"]
pub(crate) struct EdsParser;

/// A whole file: its sections in file order
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Document {
    pub sections: Vec<Section>,
}

/// `[name]` followed by its entries
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub name: String,
    pub entries: Vec<Entry>,
}

/// `keyword = field, field, ...;`
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub keyword: String,
    pub fields: Vec<Field>,
    /// The 1-based line the keyword is on
    pub line: usize,
}

/// One comma-separated value of an entry
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Field {
    /// Nothing between two commas (or before the `;`)
    Empty,
    /// A decimal or `0x` hexadecimal number
    Integer(i64),
    /// One or more adjacent quoted strings, joined, without the quotes
    Text(String),
    /// Any other unquoted token (`Param999`, `Assem100`, `TCP`, `1.6`, `08-11-2025`)
    Word(String),
}

// ======= Start of Document impl ========

impl Document {
    /// Parses the text of an EDS file. A syntax error carries pest's line and column.
    pub fn parse(text: &str) -> Result<Document> {
        let file = EdsParser::parse(Rule::file, text)
            .map_err(syntax_error)?
            .next()
            .expect("the file rule matches once");

        let sections = file
            .into_inner()
            .filter(|pair| pair.as_rule() == Rule::section)
            .map(section)
            .collect();
        Ok(Document { sections })
    }

    /// The section called `name`, whatever its case
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.name.eq_ignore_ascii_case(name))
    }
}

// ^^^^^^^^ End of Document impl ^^^^^^^^

// ======= Start of Section impl ========

impl Section {
    /// The entries whose keyword is `prefix` followed by a number (`Param1`, `Param999`), in
    /// file order
    pub fn numbered_entries<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a Entry> {
        self.entries
            .iter()
            .filter(move |entry| is_numbered(&entry.keyword, prefix))
    }
}

// ^^^^^^^^ End of Section impl ^^^^^^^^

// ======= Start of Entry impl ========

impl Entry {
    /// The field at `index`, `Empty` when the entry is shorter than that
    pub fn field(&self, index: usize) -> &Field {
        self.fields.get(index).unwrap_or(&Field::Empty)
    }

    /// A field that must be an integer that fits `T`
    pub fn integer<T: TryFrom<i64>>(&self, index: usize, expected: &str) -> Result<T> {
        self.optional_integer(index, expected)?
            .ok_or_else(|| self.bad_field(index, expected))
    }

    /// A field that is an integer that fits `T`, or empty
    pub fn optional_integer<T: TryFrom<i64>>(
        &self,
        index: usize,
        expected: &str,
    ) -> Result<Option<T>> {
        match self.field(index) {
            Field::Empty => Ok(None),
            Field::Integer(value) => T::try_from(*value)
                .map(Some)
                .map_err(|_| self.bad_field(index, expected)),
            _ => Err(self.bad_field(index, expected)),
        }
    }

    /// A text field, or an empty string when the field is empty or not text
    pub fn text(&self, index: usize) -> String {
        self.field(index).as_text().unwrap_or("").to_string()
    }

    /// An error about the entry: `Assem100 (line 212): message`
    pub fn error(&self, message: impl Into<String>) -> Error {
        Error {
            entry: format!("{} (line {})", self.keyword, self.line),
            message: message.into(),
        }
    }

    /// An error about the field at `index`, saying what it should be and what it is
    pub fn bad_field(&self, index: usize, expected: &str) -> Error {
        self.error(format!(
            "field {} should be {expected}, found {}",
            index + 1,
            self.field(index).describe()
        ))
    }
}

// ^^^^^^^^ End of Entry impl ^^^^^^^^

// ======= Start of Field impl ========

impl Field {
    pub fn is_empty(&self) -> bool {
        matches!(self, Field::Empty)
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Field::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Field::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_word(&self) -> Option<&str> {
        match self {
            Field::Word(word) => Some(word),
            _ => None,
        }
    }

    /// How the field reads in an error message
    pub fn describe(&self) -> String {
        match self {
            Field::Empty => "nothing".to_string(),
            Field::Integer(value) => format!("the number {value}"),
            Field::Text(text) => format!("the text {text:?}"),
            Field::Word(word) => format!("the word {word}"),
        }
    }
}

// ^^^^^^^^ End of Field impl ^^^^^^^^

/// Whether `keyword` is `prefix` followed by a number (`Param12` for `Param`), whatever its case
pub(crate) fn is_numbered(keyword: &str, prefix: &str) -> bool {
    keyword.len() > prefix.len()
        && keyword[..prefix.len()].eq_ignore_ascii_case(prefix)
        && keyword[prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
}

/// The pest error, placed at its line and column
pub(crate) fn syntax_error(error: pest::error::Error<Rule>) -> Error {
    let (line, column) = match error.line_col {
        pest::error::LineColLocation::Pos((line, column)) => (line, column),
        pest::error::LineColLocation::Span((line, column), _) => (line, column),
    };
    Error {
        entry: format!("line {line}, column {column}"),
        message: error.variant.message().into_owned(),
    }
}

fn section(pair: Pair<Rule>) -> Section {
    let mut name = String::new();
    let mut entries = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::section_name => name = inner.as_str().trim().to_string(),
            Rule::entry => entries.push(entry(inner)),
            _ => {}
        }
    }
    Section { name, entries }
}

fn entry(pair: Pair<Rule>) -> Entry {
    let (line, _) = pair.line_col();
    let mut keyword = String::new();
    let mut fields = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::keyword => keyword = inner.as_str().to_string(),
            Rule::fields => fields = inner.into_inner().map(field).collect(),
            _ => {}
        }
    }
    Entry {
        keyword,
        fields,
        line,
    }
}

fn field(pair: Pair<Rule>) -> Field {
    let inner = pair
        .into_inner()
        .next()
        .expect("a field is exactly one of its forms");
    match inner.as_rule() {
        Rule::empty => Field::Empty,
        Rule::integer => Field::Integer(integer(inner.as_str())),
        Rule::text => Field::Text(
            inner
                .into_inner()
                .map(|string| string.as_str().trim_matches('"'))
                .collect(),
        ),
        Rule::word => Field::Word(inner.as_str().to_string()),
        other => unreachable!("a field is not a {other:?}"),
    }
}

/// The value of an `integer` token, which the grammar guarantees is well formed
fn integer(token: &str) -> i64 {
    if let Some(hex) = token.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).unwrap_or(i64::MAX)
    } else {
        token.parse().unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Document {
        Document::parse(text).unwrap()
    }

    fn only_entry(text: &str) -> Entry {
        let document = parse(text);
        document.sections[0].entries[0].clone()
    }

    #[test]
    fn sections_and_entries_keep_file_order() {
        let document =
            parse("[File]\nDescText = \"x\";\n[Device]\nVendCode = 424;\nProdCode = 10;\n");

        let names: Vec<&str> = document
            .sections
            .iter()
            .map(|section| section.name.as_str())
            .collect();
        assert_eq!(names, vec!["File", "Device"]);
        assert_eq!(document.sections[1].entries.len(), 2);
        assert_eq!(document.sections[1].entries[1].line, 5);
    }

    #[test]
    fn section_lookups_ignore_case() {
        let document = parse("[Connection Manager]\nConnection1 = 1;\n");

        assert!(document.section("connection manager").is_some());
        assert!(document.section("Params").is_none());
    }

    #[test]
    fn section_names_may_carry_slashes_and_dashes() {
        let document = parse(
            "[TCP/IP Interface Class]\nRevision = 4;\n[Device-Classification]\nClass1 = EtherNetIP;\n",
        );
        assert!(document.section("TCP/IP Interface Class").is_some());
        assert_eq!(
            document.sections[1].entries[0].fields,
            vec![Field::Word("EtherNetIP".to_string())]
        );
    }

    #[test]
    fn integers_are_decimal_negative_or_hex() {
        let entry = only_entry("[S]\nK = 228, -1, 0x0401, 0xD2;\n");
        assert_eq!(
            entry.fields,
            vec![
                Field::Integer(228),
                Field::Integer(-1),
                Field::Integer(0x0401),
                Field::Integer(0xD2)
            ]
        );
    }

    #[test]
    fn empty_fields_keep_their_position() {
        let entry = only_entry("[S]\nK = 0,,,0x0000,1,;\n");
        assert_eq!(
            entry.fields,
            vec![
                Field::Integer(0),
                Field::Empty,
                Field::Empty,
                Field::Integer(0),
                Field::Integer(1),
                Field::Empty
            ]
        );
        assert_eq!(entry.field(99), &Field::Empty);
    }

    #[test]
    fn adjacent_strings_join_into_one_text() {
        let entry = only_entry("[S]\nIconContents =\n  \"AAA\"\n  \"BBB\";\n");
        assert_eq!(entry.fields, vec![Field::Text("AAABBB".to_string())]);
    }

    #[test]
    fn words_cover_names_dates_times_and_revisions() {
        let entry =
            only_entry("[S]\nK = Param999, Assem100, Rx, TCP, 08-11-2025, 12:10:26, 1.6;\n");
        let words: Vec<&str> = entry.fields.iter().map(|f| f.as_word().unwrap()).collect();
        assert_eq!(
            words,
            vec![
                "Param999",
                "Assem100",
                "Rx",
                "TCP",
                "08-11-2025",
                "12:10:26",
                "1.6"
            ]
        );
    }

    #[test]
    fn comments_and_line_breaks_may_sit_between_fields() {
        let entry = only_entry(
            "$ generated\r\n[S]\r\nK =\r\n  1,   $ first\r\n  $ a whole comment line\r\n  \"two\", $ second\r\n  ;\r\n",
        );
        assert_eq!(
            entry.fields,
            vec![
                Field::Integer(1),
                Field::Text("two".to_string()),
                Field::Empty
            ]
        );
    }

    #[test]
    fn numbered_entries_are_found_by_prefix() {
        let document =
            parse("[Params]\nParam1 = 1;\nEnum1 = 0,\"a\";\nParam12 = 2;\nParamX = 3;\n");
        let section = document.section("Params").unwrap();
        let keywords: Vec<&str> = section
            .numbered_entries("Param")
            .map(|entry| entry.keyword.as_str())
            .collect();
        assert_eq!(keywords, vec!["Param1", "Param12"]);
    }

    #[test]
    fn a_missing_semicolon_reports_line_and_column() {
        // pest points at the entry it could not finish
        let error = Document::parse("[S]\nK = 1\n[T]\n").unwrap_err();
        assert_eq!(error.entry, "line 2, column 5");
    }

    #[test]
    fn an_entry_outside_a_section_is_a_syntax_error() {
        let error = Document::parse("K = 1;\n").unwrap_err();
        assert_eq!(error.entry, "line 1, column 1");
    }

    #[test]
    fn nested_fields_are_not_supported() {
        let error = Document::parse("[S]\nK = { 1, 2 };\n").unwrap_err();
        assert!(error.entry.starts_with("line 2,"), "{error}");
    }
}
