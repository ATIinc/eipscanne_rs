//! `[Params]`: the `ParamN` entries, each a value of the device a field elsewhere can refer to.

use bilge::prelude::{DebugBits, FromBits, bitsize, u2};

use eds_parser_derive::FromFields;

use crate::fields::Unread;
use crate::values::ParamValue;

/// `ParamN` of `[Params]`, its fields in file order:
///
/// ```text
/// Param1 =
///     0,                          $ reserved
///     ,,                          $ link path size, link path
///     0x0004,                     $ descriptor
///     0xC8,                       $ data type: UDINT
///     4,                          $ data size in bytes
///     "RPI Range",                $ name
///     "us",                       $ units
///     "Requested packet interval",    $ help
///     1000,1000000,1000000,       $ minimum, maximum, default
///     1,1000,1,0,                 $ scaling: multiplier, divider, base, offset
///     ,,,,                        $ scaling links
///     1;                          $ decimal precision
/// ```
#[derive(Debug, Clone, PartialEq, FromFields)]
pub struct Param {
    reserved: Unread,
    link_path_size: Unread,
    link_path: Unread,
    /// `0x0004`: supports scaling
    pub descriptor: ParamDescriptor,
    /// `0xC8`: UDINT
    pub data_type: CipDataType,
    /// Bytes the value occupies on the wire
    pub data_size: u8,
    pub name: String,
    pub units: String,
    pub help: String,
    pub minimum: Option<ParamValue>,
    pub maximum: Option<ParamValue>,
    pub default: Option<ParamValue>,
    scaling_multiplier: Unread,
    scaling_divider: Unread,
    scaling_base: Unread,
    scaling_offset: Unread,
    multiplier_link: Unread,
    divisor_link: Unread,
    base_link: Unread,
    offset_link: Unread,
    decimal_precision: Unread,
    international_name: Unread,
    international_units: Unread,
    international_help: Unread,
}

/// Field 4 of `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`: what the param
/// supports and how a tool treats it. `0x0004` is supports_scaling (bit 2); `0x0010` is read-only
/// (bit 4).
#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone)]
pub struct ParamDescriptor {
    pub supports_settable_path: bool,
    pub supports_enumerated_strings: bool,
    pub supports_scaling: bool,
    pub supports_scaling_links: bool,
    pub read_only: bool,
    pub monitor: bool,
    pub supports_extended_precision_scaling: bool,
    obsolete_7_8: u2,
    pub non_displayed: bool,
    pub indirect_reference: bool,
    pub not_addressable: bool,
    pub save_supported: bool,
    pub apply_supported: bool,
    pub write_only: bool,
    reserved_15: bool,
}

/// Field 5 of `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`: the CIP data type
/// of the param's value. `0xC8` is UDINT. Each variant is the EDS spelling in Rust case
/// (`TimeOfDay` is TIME_OF_DAY); an unlisted code is `Unknown(code)`.
#[bitsize(8)]
#[derive(FromBits, PartialEq, Debug, Copy, Clone)]
#[repr(u8)]
pub enum CipDataType {
    Bool = 0xC1,
    Sint = 0xC2,
    Int = 0xC3,
    Dint = 0xC4,
    Lint = 0xC5,
    Usint = 0xC6,
    Uint = 0xC7,
    Udint = 0xC8,
    Ulint = 0xC9,
    Real = 0xCA,
    Lreal = 0xCB,
    Stime = 0xCC,
    Date = 0xCD,
    TimeOfDay = 0xCE,
    DateAndTime = 0xCF,
    String = 0xD0,
    Byte = 0xD1,
    Word = 0xD2,
    Dword = 0xD3,
    Lword = 0xD4,
    String2 = 0xD5,
    Ftime = 0xD6,
    Ltime = 0xD7,
    Itime = 0xD8,
    StringN = 0xD9,
    ShortString = 0xDA,
    Time = 0xDB,
    Epath = 0xDC,
    Engunit = 0xDD,
    StringI = 0xDE,

    #[fallback]
    Unknown(u8),
}
