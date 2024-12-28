use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipRevision {
    pub major_revision: CipUsint,
    pub minor_revision: CipUsint,
}

// NOTE: Need to make sure that the length of the string is correct
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipString {
    pub length: CipUint,
    pub value: Vec<CipByte>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipShortString {
    pub length: CipUsint,
    pub value: Vec<CipByte>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CipPath {
    // Previous EPath (in the EIPScanner project)
    pub class_id: CipUint,
    pub instance_id: CipUint,
    pub attribute_id: CipUint,
    pub size: CipUsint,
}
