use binrw::BinWrite;

use eipscanne_rs::cip::types::{CipByte, CipUint};
use eipscanne_rs::eip::packet::{
    CommandSpecificData, CommonPacketDescriptor, CommonPacketItemId, EnIpCommand,
    EnIpPacketDescription, EncapsStatusCode, EncapsulationHeader, PacketData,
};

/*
NOTE: To use another endianness, create an options object and call the serialize function with the options object.


    let big_endian_serializer = bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding();
    big_endian_serializer.serialize(&command).unwrap();

*/

#[test]
fn test_cast_encaps_command() {
    let command = EnIpCommand::RegisterSession;

    let expected_value = 0x0065;

    // Assert equality
    assert_eq!(expected_value, command as CipUint);
}

#[test]
fn test_serialize_encaps_command() {
    let command = EnIpCommand::RegisterSession;

    let mut command_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut command_byte_array);

    command.write(&mut writer).unwrap();

    let expected_byte_array = vec![0x65, 0x00];

    // Assert equality
    assert_eq!(expected_byte_array, command_byte_array);
}

#[test]
fn test_serialize_identity_ethernet_ip_component_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Send RR Data
    Encapsulation Header
        Command: Send RR Data (0x006f)
        Length: 26
        Session Handle: 0x00000006
        Status: Success (0x00000000)
        Sender Context: 0000000000000000
        Options: 0x00000000
    Command Specific Data
        Interface Handle: CIP (0x00000000)
        Timeout: 0
        Item Count: 2
            Type ID: Null Address Item (0x0000)
                Length: 0
            Type ID: Unconnected Data Item (0x00b2)
                Length: 10
        [Response In: 8]

    -------------------------------------
    Hex Dump:

    0000   6f 00 1a 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 0a 00 01 04 21 00 01 00 25 00
    0030   01 00

    */

    let expected_eip_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00,
    ];

    // create an empty packet
    let identity_request_packet = EnIpPacketDescription {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: 26,
            session_handle: 0x06,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x00; 8],
            options: 0x00,
        },
        command_data: CommandSpecificData::SendRrData(PacketData {
            interface_handle: 0x0,
            timeout: 0,
            item_count: 2,
            cip_data_packets: [
                CommonPacketDescriptor {
                    type_id: CommonPacketItemId::NullAddr,
                    packet_length: 0,
                },
                CommonPacketDescriptor {
                    type_id: CommonPacketItemId::UnconnectedMessage,
                    packet_length: 10,
                },
            ],
        }),
    };

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    identity_request_packet.write(&mut writer).unwrap();

    assert_eq!(expected_eip_byte_array, identity_byte_array);
}
