use serde::{Deserialize, Serialize};

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
