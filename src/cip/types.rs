use binrw::{
    binrw, // #[binrw] attribute
};

// This file contains the basic types used in the CIP protocol
pub type CipOctet = u8; // 8-bit value that indicates particular data type
pub type CipBool = u8; // Boolean data type
pub type CipByte = u8; // 8-bit unsigned integer
pub type CipWord = u16; // 16-bit unsigned integer
pub type CipDword = u32; // 32-bit unsigned integer
pub type CipUsint = u8; // 8-bit unsigned integer
pub type CipUint = u16; // 16-bit unsigned integer
pub type CipUdint = u32; // 32-bit unsigned integer
pub type CipSint = i8; // 8-bit signed integer
pub type CipInt = i16; // 16-bit signed integer
pub type CipDint = i32; // 32-bit signed integer
pub type CipReal = f32; // 32-bit IEEE 754 floating point
pub type CipLreal = f64; // 64-bit IEEE 754 floating point
pub type CipLword = u64; // 64-bit unsigned integer
pub type CipLint = i64; // 64-bit signed integer
pub type CipUlint = u64; // 64-bit unsigned integer

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct CipRevision {
    pub major_revision: CipUsint,
    pub minor_revision: CipUsint,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct CipShortString {
    pub length: CipUsint,

    #[br(count = length)]
    pub value: Vec<CipUsint>,
}

// ======= Start of CipShortString impl ========

impl From<String> for CipShortString {
    fn from(string_val: String) -> Self {
        CipShortString {
            length: string_val.len() as CipUsint,
            value: string_val.as_bytes().to_vec(),
        }
    }
}

impl From<CipShortString> for String {
    fn from(short_string_val: CipShortString) -> Self {
        // TODO: Determine whether the converted string matches the length of the short_string
        String::from_utf8_lossy(&short_string_val.value).to_string()
    }
}

// ======= Start of CipShortString impl ========
