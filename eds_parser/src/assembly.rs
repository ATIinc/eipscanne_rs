//! Step 3b: the `[Assembly]` section. A connection's direction names an `AssemN` as its data
//! format, and takes its size from it when the connection gives none. The members of an assembly
//! are its layout: what a caller's assembly struct is written from and checked against (see
//! `check`).

use std::fmt;

use crate::connection::resolve_path;
use crate::document::{Document, Entry, Field};
use crate::error::EdsError;
use crate::params::{self, Param, integer_field, is_param_reference, optional_integer};

/// An `AssemN` entry: its size, path and members; the descriptor is skipped
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    /// `AssemN`, as written in the file
    pub keyword: String,
    pub name: String,
    /// The path bytes (`20 04 24 64 30 03`), with every `[ParamN]` replaced by the param's
    /// default; empty when the entry gives none
    pub path: Vec<u8>,
    /// Bytes of application data: the size field, or the sum of the members when it is empty
    pub size: Option<u16>,
    /// The members in order, each placed right after the previous one
    pub members: Vec<Member>,
}

/// One `size, ParamN` pair of an assembly's member list
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Bits from the start of the assembly
    pub offset_bits: u32,
    pub size_bits: u16,
    /// What the member holds; `None` for a member without a param (opaque data)
    pub param: Option<Param>,
}

/// Field positions of an `AssemN` entry; the member pairs start at `FIRST_MEMBER`
const NAME: usize = 0;
const PATH: usize = 1;
const SIZE: usize = 2;
const FIRST_MEMBER: usize = 6;

// ======= Start of Assembly impl ========

impl Assembly {
    /// Every `AssemN` entry of the `[Assembly]` section, in file order; none when the section is
    /// absent
    pub fn all(document: &Document, params: &[Param]) -> Result<Vec<Assembly>, EdsError> {
        let Some(section) = document.section("Assembly") else {
            return Ok(Vec::new());
        };
        section
            .numbered_entries("Assem")
            .map(|entry| Assembly::from_entry(entry, params))
            .collect()
    }

    fn from_entry(entry: &Entry, params: &[Param]) -> Result<Assembly, EdsError> {
        let path = match entry.field(PATH) {
            Field::Text(text) => resolve_path(text, params, &entry.keyword)?,
            _ => Vec::new(),
        };
        let members = members(entry, params)?;
        let member_bits: u32 = members
            .iter()
            .map(|member| u32::from(member.size_bits))
            .sum();

        let size = match optional_integer(entry, SIZE)? {
            Some(value) => Some(u16::try_from(value).map_err(|_| EdsError::BadField {
                entry: entry.keyword.clone(),
                index: SIZE,
                expected: "a size in bytes",
                found: entry.field(SIZE).describe(),
            })?),
            None if members.is_empty() => None,
            None => Some(
                u16::try_from(member_bits / 8)
                    .ok()
                    .filter(|_| member_bits.is_multiple_of(8))
                    .ok_or_else(|| EdsError::BadField {
                        entry: entry.keyword.clone(),
                        index: SIZE,
                        expected: "a size, or members that add up to whole bytes",
                        found: format!("nothing, and members of {member_bits} bits"),
                    })?,
            ),
        };
        if let Some(size) = size
            && !members.is_empty()
            && member_bits != u32::from(size) * 8
        {
            return Err(EdsError::BadField {
                entry: entry.keyword.clone(),
                index: SIZE,
                expected: "the size the members add up to",
                found: format!("{size} bytes, while the members add up to {member_bits} bits"),
            });
        }

        Ok(Assembly {
            keyword: entry.keyword.clone(),
            name: entry.field(NAME).as_text().unwrap_or("").to_string(),
            path,
            size,
            members,
        })
    }

    /// The instance the path addresses (`24 64` -> 100), when the path has an instance segment
    pub fn instance(&self) -> Option<u16> {
        let mut rest = self.path.as_slice();
        loop {
            match rest {
                // 8-bit and 16-bit instance segments
                [0x24, instance, ..] => return Some(u16::from(*instance)),
                [0x25, _pad, low, high, ..] => return Some(u16::from_le_bytes([*low, *high])),
                // Other 8-bit and 16-bit logical segments: class, attribute, connection point
                [0x20 | 0x2C | 0x30, _, tail @ ..] => rest = tail,
                [0x21 | 0x2D | 0x31, _pad, _, _, tail @ ..] => rest = tail,
                _ => return None,
            }
        }
    }
}

/// The layout, one member per line: the input for writing an assembly struct
impl fmt::Display for Assembly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} \"{}\"", self.keyword, self.name)?;
        if let Some(instance) = self.instance() {
            write!(f, ", instance {instance}")?;
        }
        match self.size {
            Some(size) => writeln!(f, ", {size} bytes")?,
            None => writeln!(f, ", no size")?,
        }
        if self.members.is_empty() {
            return writeln!(f, "  (no members listed)");
        }

        writeln!(
            f,
            "  {:>6}  {:>4}  {:<6} {:<9} name",
            "byte", "bits", "type", "param"
        )?;
        for member in &self.members {
            let byte = match member.offset_bits % 8 {
                0 => format!("{}", member.offset_bits / 8),
                bit => format!("{}.{bit}", member.offset_bits / 8),
            };
            let Some(param) = &member.param else {
                writeln!(f, "  {byte:>6}  {:>4}  (no param)", member.size_bits)?;
                continue;
            };
            write!(
                f,
                "  {byte:>6}  {:>4}  {:<6} {:<9} {}",
                member.size_bits,
                param.data_type.name(),
                param.keyword,
                param.name
            )?;
            if !param.units.is_empty() {
                write!(f, " [{}]", param.units)?;
            }
            if !param.help.is_empty() {
                write!(f, " -- {}", param.help)?;
            }
            writeln!(f)?;
            for (value, name) in &param.enum_names {
                if param.data_type.is_bit_string() {
                    writeln!(f, "  {:33}bit {value}: {name}", "")?;
                } else {
                    writeln!(f, "  {:33}{value} = {name}", "")?;
                }
            }
        }
        Ok(())
    }
}

// ^^^^^^^^ End of Assembly impl ^^^^^^^^

// ======= Start of Member impl ========

impl Member {
    /// The bytes the member occupies, partly or wholly
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        let start = self.offset_bits / 8;
        let end = (self.offset_bits + u32::from(self.size_bits)).div_ceil(8);
        start as usize..end as usize
    }

    /// How the member reads in a message: its param and name, or its position
    pub fn describe(&self) -> String {
        let byte = self.offset_bits / 8;
        match &self.param {
            Some(param) => format!("{} \"{}\" at byte {byte}", param.keyword, param.name),
            None => format!("the {}-bit member at byte {byte}", self.size_bits),
        }
    }
}

// ^^^^^^^^ End of Member impl ^^^^^^^^

/// The `size, ParamN` pairs after the descriptor, each member placed after the previous one. A
/// pair with both fields empty is skipped; a member that is itself an assembly is not supported.
fn members(entry: &Entry, params: &[Param]) -> Result<Vec<Member>, EdsError> {
    let pairs = entry.fields.get(FIRST_MEMBER..).unwrap_or_default();
    let mut members = Vec::new();
    let mut offset_bits = 0;
    for (pair_index, pair) in pairs.chunks(2).enumerate() {
        let index = FIRST_MEMBER + pair_index * 2;
        let reference = pair.get(1).unwrap_or(&Field::Empty);
        if pair[0].is_empty() && reference.is_empty() {
            continue;
        }

        let size_bits: u16 = integer_field(entry, index, "a member size in bits")?;
        let param = match reference {
            Field::Empty => None,
            Field::Word(word) if is_param_reference(word) => {
                Some(params::lookup(params, word, &entry.keyword)?.clone())
            }
            other => {
                return Err(EdsError::BadField {
                    entry: entry.keyword.clone(),
                    index: index + 1,
                    expected: "a ParamN or nothing (members that are assemblies are not supported)",
                    found: other.describe(),
                });
            }
        };
        members.push(Member {
            offset_bits,
            size_bits,
            param,
        });
        offset_bits += u32::from(size_bits);
    }
    Ok(members)
}

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

    const PARAMS: &str = r#"
[Params]
    Param1 = 0,,,0x0000,0xD2,2,"Digital Inputs","","Bits 0-12",,,0,,,,,,,,,;
    Enum1 = 0,"I/O-0", 1,"I/O-1";
    Param2 = 0,,,0x0000,0xC3,2,"Analog Input","mV","",,,0,,,,,,,,,;
    Param3 = 0,,,0x0000,0xC1,1,"Flag","","",,,0,,,,,,,,,;
    Param4 = 0,,,0x0000,0xC6,1,"Config instance","","",,,0x97,,,,,,,,,;
"#;

    fn assemblies(section: &str) -> Result<Vec<Assembly>, EdsError> {
        let document = Document::parse(&format!("{PARAMS}{section}")).unwrap();
        let params = Param::all(&document).unwrap();
        Assembly::all(&document, &params)
    }

    #[test]
    fn assemblies_are_read_with_their_path_size_and_members() {
        let assemblies = assemblies(
            r#"
[Assembly]
    Object_Name = "Assembly Object";
    Object_Class_Code = 0x04;
    Assem100 =
        "Input Assembly",
        "20 04 24 64 30 03",
        5,
        0x0000,
        ,,
        16,Param1,
        16,Param2,
        8,;
    Assem151 = "Configuration", "20 04 24 97 30 03", , 0x0000, , ;
"#,
        )
        .unwrap();

        let [input, configuration] = assemblies.as_slice() else {
            panic!("two assemblies expected, got {assemblies:?}");
        };
        assert_eq!(input.keyword, "Assem100");
        assert_eq!(input.name, "Input Assembly");
        assert_eq!(input.path, vec![0x20, 0x04, 0x24, 0x64, 0x30, 0x03]);
        assert_eq!(input.size, Some(5));
        let layout: Vec<(u32, u16, Option<&str>)> = input
            .members
            .iter()
            .map(|member| {
                (
                    member.offset_bits,
                    member.size_bits,
                    member.param.as_ref().map(|param| param.keyword.as_str()),
                )
            })
            .collect();
        assert_eq!(
            layout,
            vec![
                (0, 16, Some("Param1")),
                (16, 16, Some("Param2")),
                (32, 8, None)
            ]
        );

        assert_eq!(configuration.size, None);
        assert_eq!(configuration.members, vec![]);
        assert_eq!(lookup(&assemblies, "assem100", "x").unwrap().size, Some(5));
        assert!(lookup(&assemblies, "Assem1", "Connection1").is_err());
    }

    #[test]
    fn an_empty_size_is_the_sum_of_the_members() {
        let assemblies =
            assemblies("[Assembly]\nAssem1 = \"a\", \"\", , 0, , , 16,Param1, 1,Param3, 7,;\n")
                .unwrap();

        assert_eq!(assemblies[0].size, Some(3));
        assert_eq!(assemblies[0].members[1].offset_bits, 16);
        assert_eq!(assemblies[0].members[2].offset_bits, 17);
        assert_eq!(assemblies[0].members[2].byte_range(), 2..3);
    }

    #[test]
    fn members_must_add_up_to_the_size() {
        assert_eq!(
            assemblies("[Assembly]\nAssem1 = \"a\", \"\", 4, 0, , , 16,Param1;\n").unwrap_err(),
            EdsError::BadField {
                entry: "Assem1".to_string(),
                index: SIZE,
                expected: "the size the members add up to",
                found: "4 bytes, while the members add up to 16 bits".to_string(),
            }
        );
        assert!(assemblies("[Assembly]\nAssem1 = \"a\", \"\", , 0, , , 3,Param3;\n").is_err());
    }

    #[test]
    fn a_member_that_is_an_assembly_is_not_supported() {
        assert_eq!(
            assemblies("[Assembly]\nAssem1 = \"a\", \"\", 2, 0, , , 16,Assem2;\n").unwrap_err(),
            EdsError::BadField {
                entry: "Assem1".to_string(),
                index: 7,
                expected: "a ParamN or nothing (members that are assemblies are not supported)",
                found: "the word Assem2".to_string(),
            }
        );
    }

    #[test]
    fn an_assembly_path_may_embed_a_param() {
        let assemblies =
            assemblies("[Assembly]\nAssem1 = \"a\", \"20 04 24 [Param4] 30 03\", 0, 0;\n").unwrap();

        assert_eq!(assemblies[0].path, vec![0x20, 0x04, 0x24, 0x97, 0x30, 0x03]);
        assert_eq!(assemblies[0].instance(), Some(0x97));
    }

    #[test]
    fn the_instance_comes_from_the_instance_segment() {
        let with_path = |path: Vec<u8>| Assembly {
            keyword: "Assem1".to_string(),
            name: String::new(),
            path,
            size: None,
            members: vec![],
        };

        assert_eq!(
            with_path(vec![0x20, 0x04, 0x24, 0x64, 0x30, 0x03]).instance(),
            Some(100)
        );
        assert_eq!(
            with_path(vec![0x20, 0x04, 0x25, 0x00, 0x2C, 0x01, 0x30, 0x03]).instance(),
            Some(0x012C)
        );
        assert_eq!(with_path(vec![0x20, 0x04, 0x30, 0x03]).instance(), None);
        assert_eq!(with_path(vec![]).instance(), None);
    }

    #[test]
    fn the_layout_lists_every_member_with_its_param() {
        let assemblies = assemblies(
            "[Assembly]\nAssem100 = \"Inputs\", \"20 04 24 64 30 03\", 5, 0, , , 16,Param1, 16,Param2, 8,;\n",
        )
        .unwrap();

        assert_eq!(
            assemblies[0].to_string(),
            r#"Assem100 "Inputs", instance 100, 5 bytes
    byte  bits  type   param     name
       0    16  WORD   Param1    Digital Inputs -- Bits 0-12
                                   bit 0: I/O-0
                                   bit 1: I/O-1
       2    16  INT    Param2    Analog Input [mV]
       4     8  (no param)
"#
        );
    }
}
