use binrw::BinRead;

use pretty_assertions::assert_eq;

use eipscanne_rs::cip::message::request::{MessageRouterRequest, RequestData};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData, RegisterData,
};
use eipscanne_rs::eip::packet::{EnIpPacketDescription, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

#[test]
fn test_deserialize_cip_identity_request() {
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
    let identity_request_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    let byte_cursor = std::io::Cursor::new(identity_request_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let cip_identity_request = RequestObjectAssembly::read_le(&mut buf_reader).unwrap();

    let expected_identity_packet =
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription {
                header: EncapsulationHeader {
                    command: EnIpCommand::SendRrData,
                    length: Some(26),
                    session_handle: 0x06,
                    status_code: EncapsStatusCode::Success,
                    sender_context: [0x00; 8],
                    options: 0x00,
                },
                command_specific_data: CommandSpecificData::SendRrData(
                    RRPacketData::test_with_size(0x0, 0x0, Some(10)),
                ),
            },
            cip_message: Some(MessageRouterRequest {
                service_container: ServiceContainer::new(ServiceCode::GetAttributeAll, false),
                request_data: RequestData::new(Some(0x4), CipPath::new(0x1, 0x1), None),
            }),
        };

    // Assert equality
    assert_eq!(expected_identity_packet, cip_identity_request);
}

#[test]
fn test_deserialize_registration_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000000, Register Session
    Encapsulation Header
        Command: Register Session (0x0065)
        Length: 4
        Session Handle: 0x00000000
        Status: Success (0x00000000)
        Sender Context: 0000000000000000
        Options: 0x00000000
    Command Specific Data
        Protocol Version: 1
        Option Flags: 0x0000

    -------------------------------------
    Hex Dump:
    0000   65 00 04 00 00 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 01 00 00 00

    */

    let registration_request_byte_array: Vec<CipByte> = vec![
        0x65, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];

    let byte_cursor = std::io::Cursor::new(registration_request_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let registration_request = RequestObjectAssembly::read_le(&mut buf_reader).unwrap();

    let expected_identity_packet = RequestObjectAssembly {
        packet_description: EnIpPacketDescription {
            header: EncapsulationHeader {
                command: EnIpCommand::RegisterSession,
                length: Some(4),
                session_handle: 0x00,
                status_code: EncapsStatusCode::Success,
                sender_context: [0x00; 8],
                options: 0x00,
            },
            command_specific_data: CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: 1,
                option_flags: 0x00,
            }),
        },
        cip_message: None,
    };

    assert_eq!(expected_identity_packet, registration_request);
}
