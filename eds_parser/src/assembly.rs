//! `[Assembly]`: the `AssemN` entries, each the layout of the data an assembly instance holds.

use eds_parser_derive::FromEntry;

use crate::fields::Unread;
use crate::values::{MemberReference, NumberOrParam, Path};

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
pub struct Assembly {
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
