//! `[Assembly]`: the `AssemN` entries, each the layout of the data an assembly instance holds.

use anyhow::Result;

use crate::syntax::Fields;
use crate::values::{MemberReference, NumberOrParam, Path, Unread};

/// `AssemN` of `[Assembly]`
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    pub name: Option<String>,
    pub path: Option<Path>,
    /// Bytes of data
    pub size: Option<NumberOrParam>,
    descriptor: Unread,
    reserved_5: Unread,
    reserved_6: Unread,
    /// The pairs after field 6: `16,Param1, 16,Param2, 8,`
    pub members: Vec<AssemblyMember>,
}

// ======= Start of Assembly impl ========

impl Assembly {
    /// Six fields, then `member size, member reference` pairs up to the last non-empty field:
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
    pub(crate) fn read(mut fields: Fields) -> Result<Assembly> {
        let mut assembly = Assembly {
            name: fields.next()?,
            path: fields.next()?,
            size: fields.next()?,
            descriptor: fields.next()?,
            reserved_5: fields.next()?,
            reserved_6: fields.next()?,
            members: Vec::new(),
        };
        while fields.any_left() {
            assembly.members.push(AssemblyMember {
                size_bits: fields.next()?,
                reference: fields.next()?,
            });
        }
        Ok(assembly)
    }
}

// ^^^^^^^^ End of Assembly impl ^^^^^^^^

/// One `member size, member reference` pair of an `AssemN` entry: `16,Param1,` (16 bits holding
/// Param1), `8,` (8 bits of opaque data), or `,` (neither)
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyMember {
    pub size_bits: Option<u16>,
    pub reference: Option<MemberReference>,
}
