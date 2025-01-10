use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::message::{MessageRouter, ServiceCode};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::packet::{
    CommandSpecificData, CommonPacketDescriptor, CommonPacketItemId, EnIpCommand,
    EnIpPacketDescription, EncapsStatusCode, EncapsulationHeader, PacketData,
};
use eipscanne_rs::object_assembly::ObjectAssembly;

#[test]
fn test_serialize_full_identity_request() {
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
        command_specific_data: CommandSpecificData::SendRrData(PacketData {
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
    let mut identity_writer = std::io::Cursor::new(&mut identity_byte_array);

    identity_request_packet.write(&mut identity_writer).unwrap();

    /*
    Common Industrial Protocol
    Service: Get Attributes All (Request)
        0... .... = Request/Response: Request (0x0)
        .000 0001 = Service: Get Attributes All (0x01)
    Request Path Size: 4 words
    Request Path: Identity, Instance: 0x0001
        Path Segment: 0x21 (16-Bit Class Segment)
            001. .... = Path Segment Type: Logical Segment (1)
            ...0 00.. = Logical Segment Type: Class ID (0)
            .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
            Class: Identity (0x0001)
        Path Segment: 0x25 (16-Bit Instance Segment)
            001. .... = Path Segment Type: Logical Segment (1)
            ...0 01.. = Logical Segment Type: Instance ID (1)
            .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
            Instance: 0x0001
    Get Attributes All (Request)

    -------------------------------------
    Hex Dump:

    0000   01 04 21 00 01 00 25 00 01 00

    */

    let expected_byte_array: Vec<CipByte> =
        vec![0x01, 0x04, 0x21, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00];

    let message_router_request =
        MessageRouter::new_request(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

    // Serialize the struct into a byte array
    let mut message_router_bytes: Vec<u8> = Vec::new();
    let mut message_writer = std::io::Cursor::new(&mut message_router_bytes);

    message_router_request.write(&mut message_writer).unwrap();

    // Assert equality
    let full_expected_identity_request = [expected_eip_byte_array, expected_byte_array].concat();
    let full_identity_request = [identity_byte_array, message_router_bytes].concat();

    assert_eq!(full_expected_identity_request, full_identity_request);
}

#[test]
fn test_serialize_new_identity_request() {
    let expected_identity_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    // create an empty packet
    let identity_request_packet = ObjectAssembly::new_identity(0x6);

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    identity_request_packet.write(&mut writer).unwrap();

    assert_eq!(expected_identity_byte_array, identity_byte_array);
}

#[test]
fn test_deserialize_identity_response() {
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

    /*
    Common Industrial Protocol
    Service: Get Attributes All (Response)
        1... .... = Request/Response: Response (0x1)
        .000 0001 = Service: Get Attributes All (0x01)
    Status: Success:
        General Status: Success (0x00)
        Additional Status Size: 0 words
    [Request Path Size: 4 words]
    [Request Path: Identity, Instance: 0x0001]
        [Path Segment: 0x21 (16-Bit Class Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 00.. = Logical Segment Type: Class ID (0)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Class: Identity (0x0001)]
        [Path Segment: 0x25 (16-Bit Instance Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 01.. = Logical Segment Type: Instance ID (1)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Instance: 0x0001]
    Get Attributes All (Response)
        Attribute: 1 (Vendor ID)
            Vendor ID: Teknic, Inc. (0x01a8)
        Attribute: 2 (Device Type)
            Device Type: Generic Device (keyable) (0x002b)
        Attribute: 3 (Product Code)
            Product Code: 1
        Attribute: 4 (Revision)
            Major Revision: 2
            Minor Revision: 93
        Attribute: 5 (Status)
            Status: 0x0000
                .... .... .... ...0 = Owned: 0
                .... .... .... .0.. = Configured: 0
                .... .... 0000 .... = Extended Device Status: 0x0
                .... ...0 .... .... = Minor Recoverable Fault: 0
                .... ..0. .... .... = Minor Unrecoverable Fault: 0
                .... .0.. .... .... = Major Recoverable Fault: 0
                .... 0... .... .... = Major Unrecoverable Fault: 0
                0000 .... .... .... = Extended Device Status 2: 0x0
        Attribute: 6 (Serial Number)
            Serial Number: 0x01ff3d32
        Attribute: 7 (Product Name)
            Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   81 00 00 00 a8 01 2b 00 01 00 02 5d 00 00 32 3d
    0010   ff 01 09 43 6c 65 61 72 4c 69 6e 6b

    */

    // Assert equality
    assert_eq!(0x0, 0x0);
}
