use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize};
use std::error::Error;

// Define the child struct
#[derive(Serialize, Deserialize, Debug)]
struct ChildStruct {
    id: u32,
    description: String,
}

// Define the parent struct
#[derive(Serialize, Deserialize, Debug)]
struct MyStruct {
    byte_field: u8,
    int_field: i32,
    double_field: f64,
    ubyte_field: u8,
    byte_array: [u8; 6],
    child: ChildStruct, // Include the child struct as a field
}

// Serialize function for MyStruct
fn serialize_struct(my_struct: &MyStruct) -> Result<Vec<u8>, Box<dyn Error>> {
    let serialized = serialize(my_struct)?;
    Ok(serialized)
}

// Deserialize function for MyStruct
fn deserialize_struct(bytes: &[u8]) -> Result<MyStruct, Box<dyn Error>> {
    let deserialized: MyStruct = deserialize(bytes)?;
    Ok(deserialized)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Create an instance of MyStruct with a ChildStruct field
    let my_struct = MyStruct {
        byte_field: 10,
        int_field: -100,
        double_field: 3.14159,
        ubyte_field: 255,
        byte_array: [1, 2, 3, 4, 5, 6],
        child: ChildStruct {
            id: 1234,
            description: String::from("A child struct example"),
        },
    };

    // Serialize the struct into a byte array
    let byte_array = serialize_struct(&my_struct)?;

    println!("Serialized byte array: {:?}", byte_array);

    // Deserialize the byte array back into a struct
    let deserialized_struct = deserialize_struct(&byte_array)?;

    println!("Deserialized struct: {:?}", deserialized_struct);

    Ok(())
}
