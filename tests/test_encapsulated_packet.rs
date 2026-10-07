mod common;

use binrw::BinWrite;

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::ServiceContainer;
use eipscanne_rs::cip::message::{request::MessageRouterRequest, shared::ServiceCode};
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipByte, CipUint};
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::description::{CommonPacketDescriptor, CommonPacketItemId};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::eip::sockaddr::SockaddrInfoItems;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

use common::IDENTITY_SESSION_HANDLE;

/// The Get Attributes All response of the identity object (the data of the Unconnected Data Item)
fn identity_response_message() -> MessageRouterResponse {
    MessageRouterResponse {
        service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0x0,
            additional_status: vec![],
            data: CipDataOpt::Raw(vec![
                0xa8, 0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32, 0x3d, 0xff, 0x01,
                0x09, 0x43, 0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
            ]),
        },
    }
}

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
    assert_eq_hex!(expected_byte_array, command_byte_array);
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
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    let identity_request_packet = RequestObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: None,
            session_handle: IDENTITY_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterRequest::new(
                ServiceCode::GetAttributeAll,
                CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
            ),
        )),
    };

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    // The lengths of the header and of the Unconnected Data Item are calculated while writing
    identity_request_packet.write(&mut writer).unwrap();

    assert_eq!(expected_eip_byte_array, identity_byte_array);
}

#[test]
fn test_serialize_message_router_generated_identity_ethernet_ip_component_request() {
    let expected_eip_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    let identity_cip_path = CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID);

    let message_router_request =
        MessageRouterRequest::new(ServiceCode::GetAttributeAll, identity_cip_path);

    let cip_request_packet = RequestObjectAssembly::new_send_rr_data(
        IDENTITY_SESSION_HANDLE,
        NO_ENCAPSULATION_TIMEOUT,
        message_router_request,
    );

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    cip_request_packet.write(&mut writer).unwrap();

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

    let packet_description = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    let expected_packet_description = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(44),
            session_handle: IDENTITY_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData {
            interface_handle: CIP_INTERFACE_HANDLE,
            timeout: NO_ENCAPSULATION_TIMEOUT,
            null_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: Some(28),
            },
            cip_message: identity_response_message().into(),
            sockaddr_info_items: SockaddrInfoItems::empty(),
        }),
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
    0020   00 00 00 00 b2 00 1c 00 81 00 00 00 a8 01 2b 00
    0030   01 00 02 5d 00 00 32 3d ff 01 09 43 6c 65 61 72
    0040   4c 69 6e 6b

    */

    // The data of the Unconnected Data Item is part of the packet, so it must be present to read it
    let raw_bytes = vec![
        0x6f, 0x00, 0x2c, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1c, 0x00, 0x81, 0x00, 0x00, 0x00, 0xa8,
        0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32, 0x3d, 0xff, 0x01, 0x09, 0x43,
        0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let packet_description = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    let expected_packaet_description = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(44),
            session_handle: IDENTITY_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData {
            interface_handle: CIP_INTERFACE_HANDLE,
            timeout: NO_ENCAPSULATION_TIMEOUT,
            null_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: Some(28),
            },
            cip_message: identity_response_message().into(),
            sockaddr_info_items: SockaddrInfoItems::empty(),
        }),
    };

    assert_eq!(expected_packaet_description, packet_description);
}
