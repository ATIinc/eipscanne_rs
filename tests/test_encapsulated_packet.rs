use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::message::{MessageRouterRequest, ServiceCode};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipByte, CipUint};
use eipscanne_rs::eip::packet::{
    CommandSpecificData, EnIpCommand, EnIpPacketDescription, EncapsStatusCode, EncapsulationHeader,
    RRPacketData,
};

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
            session_handle: 0x06,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x00; 8],
            options: 0x00,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new(0x0, 0)),
    };

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    // NOTE: The args are passed by the ObjectAssembly
    identity_request_packet
        .write_options(&mut writer, binrw::Endian::Little, (10,))
        .unwrap();

    assert_eq!(expected_eip_byte_array, identity_byte_array);
}

#[test]
fn test_serialize_message_router_generated_identity_ethernet_ip_component_request() {
    let expected_eip_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00,
    ];

    // create an empty packet
    let identity_cip_path = CipPath::new(0x1, 0x1);

    let message_router_request =
        MessageRouterRequest::new(ServiceCode::GetAttributeAll, identity_cip_path);

    let mut message_request_buffer: Vec<u8> = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut message_request_buffer);
    let _ = message_router_request.write(&mut temp_writer);

    let cip_request_packet = EnIpPacketDescription::new_cip_description(0x06, 0);

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    cip_request_packet
        .write_options(
            &mut writer,
            binrw::Endian::Little,
            (message_request_buffer.len() as u16,),
        )
        .unwrap();

    assert_eq!(expected_eip_byte_array, identity_byte_array);
}

#[test]
fn test_deserialize_identity_object_response_encapsulated_packet() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Send RR Data
    Encapsulation Header
        Command: Send RR Data (0x006f)
        Length: 44
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
                Length: 28
        [Request In: 7]
        [Time: 0.000514275 seconds]

    -------------------------------------
    Hex Dump:

    0000   6f 00 2c 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1c 00 81 00 00 00 a8 01 2b 00
    0030   01 00 02 5d 00 00 32 3d ff 01 09 43 6c 65 61 72
    0040   4c 69 6e 6b

    */

    let raw_bytes = vec![
        0x6f, 0x00, 0x2c, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1c, 0x00, 0x81, 0x00, 0x00, 0x00, 0xa8,
        0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32, 0x3d, 0xff, 0x01, 0x09, 0x43,
        0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let packet_description = EnIpPacketDescription::read(&mut buf_reader).unwrap();

    let expected_packet_description = EnIpPacketDescription {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            session_handle: 0x06,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x00; 8],
            options: 0x00,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new(0x0, 0)),
    };

    assert_eq!(expected_packet_description, packet_description);
}

#[test]
fn test_deserialize_identity_object_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Send RR Data
    Encapsulation Header
        Command: Send RR Data (0x006f)
        Length: 44
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
                Length: 28
        [Request In: 7]
        [Time: 0.000514275 seconds]

    -------------------------------------
    Hex Dump:

    0000   6f 00 2c 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1c 00

    */

    let raw_bytes = vec![
        0x6f, 0x00, 0x2c, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1c, 0x00,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let packet_description = EnIpPacketDescription::read(&mut buf_reader).unwrap();

    let expected_packaet_description = EnIpPacketDescription {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            // length: 44,
            session_handle: 0x06,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x00; 8],
            options: 0x00,
        },
        // packet_length = 28
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new(0x0, 0)),
    };

    assert_eq!(expected_packaet_description, packet_description);
}
