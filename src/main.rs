use bincode::{deserialize, serialize};
use std::error::Error;

use eipscanne_rs::eip::packet::{
    EncapsCommand, EncapsStatusCode, EncapsulatedHeader, RegisterData,
};

fn identity_object() -> Result<(), Box<dyn Error>> {
    // auto si = std::make_shared<SessionInfo>("172.28.1.3", 0xAF12);
    // IdentityObject identityObject(1, si);

    // extract from SessionInfo
    // let session_handle = 3;

    // create an empty packet
    let empty_eip_packet = eipscanne_rs::eip::packet::EncapsulatedPacket {
        header: EncapsulatedHeader {
            command: EncapsCommand::RegisterSession,
            length: 0x2,
            session_handle: 0x1,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0],
            options: 0x00,
        },
        command_data: eipscanne_rs::eip::packet::CommandSpecificData::RegisterSession(
            RegisterData {
                protocol_version: 0x1,
                option_flags: 0x0,
            },
        ),
    };

    // Serialize the struct into a byte array
    let byte_array = serialize(&empty_eip_packet).unwrap();

    println!("Serialized byte array: {:?}", byte_array);

    let deserialized_struct: eipscanne_rs::eip::packet::EncapsulatedPacket =
        deserialize(&byte_array).unwrap();

    println!("Deserialized struct: {:?}", deserialized_struct);

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
    identity_object()?;
    Ok(())
}
