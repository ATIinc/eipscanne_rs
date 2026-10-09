mod common;

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::{MessageRouterRequest, RequestData};
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipByte, CipShortString, CipUdint};
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, UnitPacketData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::description::{CommonPacketDescriptor, CommonPacketItemId};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

/// Session handle of the class 3 captures (the `read-identity-connected` example against OpENer)
const CONNECTED_SESSION_HANDLE: CipUdint = 0x01;
/// Originator to target network connection ID OpENer picked in the Forward_Open reply
const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xaa93_0013;
/// Target to originator network connection ID the scanner picked in the Forward_Open request
const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5679;

#[test]
fn test_serialize_connected_identity_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000001, Send Unit Data, Connection ID: 0xAA930013
        Encapsulation Header
            Command: Send Unit Data (0x0070)
            Length: 32
            Session Handle: 0x00000001
            Status: Success (0x00000000)
            Sender Context: 0000000000000000
            Options: 0x00000000
        Command Specific Data
            Interface Handle: CIP (0x00000000)
            Timeout: 0
            Item Count: 2
                Type ID: Connected Address Item (0x00a1)
                    Length: 4
                    Connection ID: 0xaa930013
                Type ID: Connected Data Item (0x00b1)
                    Length: 12
                    CIP Sequence Count: 1
    Common Industrial Protocol
        Service: Get Attributes All (Request)
            0... .... = Request/Response: Request (0x0)
            .000 0001 = Service: Get Attributes All (0x01)
        Request Path Size: 4 words
        Request Path: Identity, Instance: 0x0001

    Hex Dump:
    0000   70 00 20 00 01 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   a1 00 04 00 13 00 93 aa b1 00 0c 00 01 00 01 04
    0030   21 00 01 00 25 00 01 00
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x70, 0x00, 0x20, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0xa1, 0x00, 0x04, 0x00, 0x13, 0x00, 0x93, 0xaa, 0xb1, 0x00, 0x0c, 0x00, 0x01,
        0x00, 0x01, 0x04, 0x21, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    let packet = EnIpPacket::new_send_unit_data(
        CONNECTED_SESSION_HANDLE,
        O2T_NETWORK_CONNECTION_ID,
        1,
        MessageRouterRequest::new(
            ServiceCode::GetAttributeAll,
            CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
        ),
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    packet.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    // Read back, the lengths are the ones on the wire
    let deserialized = EnIpPacket::read(&mut std::io::Cursor::new(expected_byte_array)).unwrap();
    let expected_packet = EnIpPacket {
        header: EncapsulationHeader {
            command: EnIpCommand::SendUnitData,
            length: Some(32),
            session_handle: CONNECTED_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendUnitData(UnitPacketData {
            interface_handle: CIP_INTERFACE_HANDLE,
            timeout: NO_ENCAPSULATION_TIMEOUT,
            connected_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectionAddressItem,
                packet_length: Some(4),
            },
            connection_id: O2T_NETWORK_CONNECTION_ID,
            connected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectedTransportPacket,
                packet_length: Some(12),
            },
            cip_sequence_count: 1,
            cip_message: MessageRouterRequest {
                service_container: ServiceContainer::new_request(ServiceCode::GetAttributeAll),
                request_data: RequestData::new(
                    Some(0x4),
                    CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
                    None,
                ),
            }
            .into(),
        }),
    };
    assert_eq!(expected_packet, deserialized);
}

#[test]
fn test_deserialize_connected_identity_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000001, Send Unit Data, Connection ID: 0x12345679
        Encapsulation Header
            Command: Send Unit Data (0x0070)
            Length: 50
            Session Handle: 0x00000001
            Status: Success (0x00000000)
            Sender Context: 0000000000000000
            Options: 0x00000000
        Command Specific Data
            Interface Handle: CIP (0x00000000)
            Timeout: 0
            Item Count: 2
                Type ID: Connected Address Item (0x00a1)
                    Length: 4
                    Connection ID: 0x12345679
                Type ID: Connected Data Item (0x00b1)
                    Length: 30
                    CIP Sequence Count: 1
    Common Industrial Protocol
        Service: Get Attributes All (Response)
            1... .... = Request/Response: Response (0x1)
            .000 0001 = Service: Get Attributes All (0x01)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words
        Get Attributes All (Response)
            Attribute: 1 (Vendor ID)
                Vendor ID: Rockwell Automation/Allen-Bradley (0x0001)
            Attribute: 2 (Device Type)
                Device Type: Communications Adapter (0x000c)
            Attribute: 3 (Product Code)
                Product Code: 65001
            Attribute: 4 (Revision)
                Major Revision: 2
                Minor Revision: 3
            Attribute: 5 (Status)
                Status: 0x0000
            Attribute: 6 (Serial Number)
                Serial Number: 0x075bcd15
            Attribute: 7 (Product Name)
                Product Name: OpENer PC

    Hex Dump:
    0000   70 00 32 00 01 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   a1 00 04 00 79 56 34 12 b1 00 1e 00 01 00 81 00
    0030   00 00 01 00 0c 00 e9 fd 02 03 00 00 15 cd 5b 07
    0040   09 4f 70 45 4e 65 72 20 50 43
    */
    let response_bytes: Vec<CipByte> = vec![
        0x70, 0x00, 0x32, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0xa1, 0x00, 0x04, 0x00, 0x79, 0x56, 0x34, 0x12, 0xb1, 0x00, 0x1e, 0x00, 0x01,
        0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00, 0x0c, 0x00, 0xe9, 0xfd, 0x02, 0x03, 0x00, 0x00,
        0x15, 0xcd, 0x5b, 0x07, 0x09, 0x4f, 0x70, 0x45, 0x4e, 0x65, 0x72, 0x20, 0x50, 0x43,
    ];

    let response = EnIpPacket::read(&mut std::io::Cursor::new(response_bytes)).unwrap();

    // The data is read raw; it compares equal to the typed Identity response
    let expected_response = EnIpPacket {
        header: EncapsulationHeader {
            command: EnIpCommand::SendUnitData,
            length: Some(50),
            session_handle: CONNECTED_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendUnitData(UnitPacketData {
            interface_handle: CIP_INTERFACE_HANDLE,
            timeout: NO_ENCAPSULATION_TIMEOUT,
            connected_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectionAddressItem,
                packet_length: Some(4),
            },
            connection_id: T2O_NETWORK_CONNECTION_ID,
            connected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::ConnectedTransportPacket,
                packet_length: Some(30),
            },
            cip_sequence_count: 1,
            cip_message: MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0x0,
                    additional_status: vec![],
                    data: CipDataOpt::Typed(Box::new(IdentityResponse {
                        vendor_id: VendorId::Unknown(0x0001),
                        device_type: DeviceType::Unknown(0x000c),
                        product_code: 65001,
                        revision: Revision { major: 2, minor: 3 },
                        status: IdentityStatusBits::default().into(),
                        serial_number: 0x075b_cd15,
                        product_name: CipShortString::from("OpENer PC".to_string()),
                    })),
                },
            }
            .into(),
        }),
    };
    assert_eq!(expected_response, response);

    // The response is found the same way as in a Send RR Data reply
    assert!(response.response().unwrap().is_success());
}
