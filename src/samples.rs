use crate::cip::types::{CipRevision, CipShortString, CipUdint, CipUint, CipWord};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]

pub struct IdentityObject {
    // Identity object attributes
    pub vendor_id: CipUint,
    pub device_type: CipUint,
    pub product_code: CipUint,
    pub revision: CipRevision,
    pub status: CipWord,
    pub serial_number: CipUdint,
    pub product_name: CipShortString,
}

// Define the child struct
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ChildStruct {
    pub id: u32,
    pub description: String,
}

// Define the parent struct
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MyStruct {
    pub byte_field: u8,
    pub int_field: i32,
    pub double_field: f64,
    pub ubyte_field: u8,
    pub byte_array: [u8; 6],
    pub child: ChildStruct, // Include the child struct as a field
}
