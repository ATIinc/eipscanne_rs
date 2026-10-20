//! `[Assembly]`: the `AssemN` entries, each the layout of the data an assembly instance holds.

use eds_parser_derive::FromEntry;

use crate::error::{Error, Result};
use crate::fields::{FromField, NumberOrParam, Path, Unread, numbered};

/// `AssemN` of `[Assembly]`, its fields in file order: six fields, then `member size, member
/// reference` pairs up to the last non-empty field:
///
/// ```text
/// Assem100 =
///     "Input Assembly",           $ name
///     "20 04 24 64 30 03",        $ path
///     5,                          $ size in bytes
///     0x0000,                     $ descriptor
///     ,,                          $ two reserved fields
///     16,Param1,                  $ members: size in bits, reference
///     16,Param2,
///     8,;
/// ```
#[derive(Debug, Clone, PartialEq, FromEntry)]
pub struct AssemblyEntry {
    pub name: String,
    pub path: Option<Path>,
    /// Bytes of data
    pub size: Option<NumberOrParam>,
    descriptor: Unread,
    reserved_5: Unread,
    reserved_6: Unread,
    /// `16,Param1, 16,Param2, 8,`
    pub members: Vec<AssemblyMember>,
}

/// One `member size, member reference` pair of an `AssemN` entry, its fields in file order. Either
/// field can be empty:
///
/// ```text
/// Assem100 = "Input Assembly", "20 04 24 64 30 03", 3, 0x0000,,,
///     16,Param1,                  $ 16 bits holding Param1
///     8,;                         $ 8 bits of opaque data
/// ```
#[derive(Debug, Clone, PartialEq, FromEntry)]
pub struct AssemblyMember {
    pub size_bits: Option<u16>,
    pub reference: Option<MemberReference>,
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
            parse_int::parse(text)
                .map(MemberReference::Number)
                .map_err(|cause| {
                    Error::Entry(format!(
                        "expected a ParamN, AssemN, number or path, found {text:?}: {cause}"
                    ))
                })
        }
    }
}
