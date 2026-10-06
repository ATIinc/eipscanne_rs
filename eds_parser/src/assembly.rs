//! Step 3b: the `[Assembly]` section. A connection's direction names an `AssemN` as its data
//! format, and takes its size from it when the connection gives none.

use crate::document::{Document, Entry};
use crate::error::EdsError;
use crate::params::{optional_integer, text_or_empty};

/// An `AssemN` entry: the member list, path and descriptor are skipped
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    /// `AssemN`, as written in the file
    pub keyword: String,
    pub name: String,
    /// Bytes of application data, when the entry gives it
    pub size: Option<u16>,
}

/// Field positions of an `AssemN` entry
const NAME: usize = 0;
const SIZE: usize = 2;

// ======= Start of Assembly impl ========

impl Assembly {
    /// Every `AssemN` entry of the `[Assembly]` section, in file order; none when the section is
    /// absent
    pub fn all(document: &Document) -> Result<Vec<Assembly>, EdsError> {
        let Some(section) = document.section("Assembly") else {
            return Ok(Vec::new());
        };
        section
            .numbered_entries("Assem")
            .map(Assembly::from_entry)
            .collect()
    }

    fn from_entry(entry: &Entry) -> Result<Assembly, EdsError> {
        let size = match optional_integer(entry, SIZE)? {
            None => None,
            Some(value) => Some(u16::try_from(value).map_err(|_| EdsError::BadField {
                entry: entry.keyword.clone(),
                index: SIZE,
                expected: "a size in bytes",
                found: entry.field(SIZE).describe(),
            })?),
        };
        Ok(Assembly {
            keyword: entry.keyword.clone(),
            name: text_or_empty(entry, NAME),
            size,
        })
    }
}

// ^^^^^^^^ End of Assembly impl ^^^^^^^^

/// The assembly with `keyword` (case ignored), or an `UnknownReference` naming `entry`
pub(crate) fn lookup<'a>(
    assemblies: &'a [Assembly],
    keyword: &str,
    entry: &str,
) -> Result<&'a Assembly, EdsError> {
    assemblies
        .iter()
        .find(|assembly| assembly.keyword.eq_ignore_ascii_case(keyword))
        .ok_or_else(|| EdsError::UnknownReference {
            entry: entry.to_string(),
            reference: keyword.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemblies_are_read_with_their_sizes() {
        let document = Document::parse(
            r#"
[Assembly]
    Object_Name = "Assembly Object";
    Object_Class_Code = 0x04;
    Assem100 =
        "Input Assembly",
        "20 04 24 64 30 03",
        228,
        0x0000,
        ,,
        16,Param1,
        32,Param4;
    Assem151 = "Configuration", "20 04 24 97 30 03", , 0x0000, , ;
"#,
        )
        .unwrap();

        let assemblies = Assembly::all(&document).unwrap();

        assert_eq!(
            assemblies,
            vec![
                Assembly {
                    keyword: "Assem100".to_string(),
                    name: "Input Assembly".to_string(),
                    size: Some(228),
                },
                Assembly {
                    keyword: "Assem151".to_string(),
                    name: "Configuration".to_string(),
                    size: None,
                },
            ]
        );
        assert_eq!(
            lookup(&assemblies, "assem100", "x").unwrap().size,
            Some(228)
        );
        assert!(lookup(&assemblies, "Assem1", "Connection1").is_err());
    }
}
