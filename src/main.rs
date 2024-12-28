use bincode::{deserialize, serialize};
use std::error::Error;

use eipscanne_rs::{
    cip::types::CipByte,
    samples::{ChildStruct, MyStruct},
};

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

fn sample_struct() {
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
    let byte_array = serialize_struct(&my_struct).unwrap();

    println!("Serialized byte array: {:?}", byte_array);

    // Deserialize the byte array back into a struct
    let deserialized_struct = deserialize_struct(&byte_array).unwrap();

    println!("Deserialized struct: {:?}", deserialized_struct);
}

fn identity_object() -> Result<(), Box<dyn Error>> {
    // auto si = std::make_shared<SessionInfo>("172.28.1.3", 0xAF12);
    // IdentityObject identityObject(1, si);

    // extract from SessionInfo
    let session_handle = 0;

    // create an empty packet
    let empty_eip_packet =
        eipscanne_rs::eip::packet::EncapsulatedPacket::new_empty(session_handle, 0);

    // // read the header
    // auto header = _socket.Receive(EncapsPacket::HEADER_SIZE);
    // auto length = EncapsPacket::getLengthFromHeader(header);

    // // read the rest of the packet
    // auto data = _socket.Receive(length);
    // header.insert(header.end(), data.begin(), data.end());

    // // deserialize the packet
    // EncapsPacket recvPacket;
    // recvPacket.expand(header);

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    sample_struct();
    // identity_object()?;
    Ok(())
}
