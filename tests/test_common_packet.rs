mod common;

use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    ETHERNET_IP_IO_UDP_PORT, NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::description::{CommonPacketItem, CommonPacketItemId};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::eip::sockaddr::SockaddrInfo;
use eipscanne_rs::object_assembly::ResponseObjectAssembly;

use common::CLEARLINK_IO_SESSION_HANDLE;

fn sample_address() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), ETHERNET_IP_IO_UDP_PORT)
}

#[test]
fn test_serialize_sockaddr_info_big_endian_fields() {
    /*
    Socket Address
        sin_family: 2
        sin_port: 2222
        sin_addr: 192.168.1.10
        sin_zero: 0000000000000000

    Hex Dump:
    0000   00 02 08 ae c0 a8 01 0a 00 00 00 00 00 00 00 00
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x00, 0x02, 0x08, 0xae, 0xc0, 0xa8, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];

    let sockaddr_info = SockaddrInfo::from(sample_address());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    sockaddr_info.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = SockaddrInfo::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, sockaddr_info);
    assert_eq!(deserialized.socket_address(), sample_address());
}

#[test]
fn test_serialize_t2o_sockaddr_info_item() {
    /*
    Type ID: Socket Address Info T->O (0x8001)
        Length: 16
        Socket Address ...
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x01, 0x80, 0x10, 0x00, 0x00, 0x02, 0x08, 0xae, 0xc0, 0xa8, 0x01, 0x0a, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let item = CommonPacketItem::T2OSockAddrInfo(sample_address().into());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    item.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = CommonPacketItem::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, item);
    assert_eq!(
        deserialized
            .sockaddr_info()
            .map(SockaddrInfo::socket_address),
        Some(sample_address())
    );
}

#[test]
fn test_unknown_item_is_kept_as_raw_bytes() {
    let raw_bytes: Vec<CipByte> = vec![0x34, 0x12, 0x03, 0x00, 0xaa, 0xbb, 0xcc];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let item = CommonPacketItem::read(&mut buf_reader).unwrap();

    assert_eq!(
        item,
        CommonPacketItem::Unknown {
            type_id: CommonPacketItemId::Unknown(0x1234),
            data: vec![0xaa, 0xbb, 0xcc],
        }
    );
    assert!(item.sockaddr_info().is_none());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    item.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}

#[test]
fn test_response_assembly_with_trailing_sockaddr_item() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 40
            Session Handle: 0x00000003
            Status: Success (0x00000000)
            Sender Context: 0000000000000000
            Options: 0x00000000
        Command Specific Data
            Interface Handle: CIP (0x00000000)
            Timeout: 0
            Item Count: 3
                Type ID: Null Address Item (0x0000)
                    Length: 0
                Type ID: Unconnected Data Item (0x00b2)
                    Length: 4
                Type ID: Socket Address Info O->T (0x8000)
                    Length: 16
                    sin_family: 2
                    sin_port: 2222
                    sin_addr: 192.168.1.10
                    sin_zero: 0000000000000000
    Common Industrial Protocol
        Service: Forward Open (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Forward Open (0x54)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words

    Hex Dump:
    0000   6f 00 28 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 03 00
    0020   00 00 00 00 b2 00 04 00 d4 00 00 00 00 80 10 00
    0030   00 02 08 ae c0 a8 01 0a 00 00 00 00 00 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x28, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x04, 0x00, 0xd4, 0x00, 0x00, 0x00, 0x00,
        0x80, 0x10, 0x00, 0x00, 0x02, 0x08, 0xae, 0xc0, 0xa8, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    let expected_response = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(40),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData({
            let mut rr_data = RRPacketData::new_unconnected(
                CIP_INTERFACE_HANDLE,
                NO_ENCAPSULATION_TIMEOUT,
                MessageRouterResponse {
                    service_container: ServiceContainer::new_response(ServiceCode::ForwardOpen),
                    response_data: ResponseData {
                        status: ResponseStatusCode::Success,
                        additional_status_size: 0,
                        additional_status: vec![],
                        data: CipDataOpt::Raw(vec![]),
                    },
                },
            );
            rr_data
                .items
                .push(CommonPacketItem::O2TSockAddrInfo(sample_address().into()));
            rr_data
        }),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response_object = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    assert_eq!(expected_response, response_object);
    assert_eq!(response_object.sockaddr_info_items().count(), 1);

    // Writing the parsed object must reproduce the packet, including the lengths and the item count
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    response_object.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}

#[test]
fn test_read_response_rejects_a_request() {
    // The identity request of a Get Attributes All exchange: a SendRRData packet carrying a request
    let request_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    let read_response =
        ResponseObjectAssembly::read_response(&mut std::io::Cursor::new(&request_bytes));
    assert!(read_response.is_err());

    // The lenient read keeps the request
    let packet = ResponseObjectAssembly::read(&mut std::io::Cursor::new(&request_bytes)).unwrap();
    assert!(matches!(packet.cip_message(), Some(CipMessage::Request(_))));
    assert!(packet.response().is_none());
}

#[cfg(feature = "adapter")]
#[test]
fn test_read_request_accepts_a_request_and_rejects_a_response() {
    use eipscanne_rs::object_assembly::RequestObjectAssembly;

    let request_bytes = RequestObjectAssembly::new_identity(0x6);
    let mut written_request: Vec<u8> = Vec::new();
    request_bytes
        .write(&mut std::io::Cursor::new(&mut written_request))
        .unwrap();
    assert!(
        RequestObjectAssembly::read_request(&mut std::io::Cursor::new(&written_request)).is_ok()
    );

    let response = ResponseObjectAssembly::new_send_rr_data(
        0x6,
        0,
        MessageRouterResponse {
            service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
            response_data: ResponseData {
                status: ResponseStatusCode::Success,
                additional_status_size: 0,
                additional_status: vec![],
                data: CipDataOpt::Raw(vec![]),
            },
        },
    );
    let mut written_response: Vec<u8> = Vec::new();
    response
        .write(&mut std::io::Cursor::new(&mut written_response))
        .unwrap();
    assert!(
        RequestObjectAssembly::read_request(&mut std::io::Cursor::new(&written_response)).is_err()
    );
}

#[test]
fn test_write_then_read_response() {
    // An adapter builds a Forward_Open reply with a Sockaddr Info item and writes it...
    let forward_open_reply = MessageRouterResponse {
        service_container: ServiceContainer::new_response(ServiceCode::ForwardOpen),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0,
            additional_status: vec![],
            data: CipDataOpt::Raw(vec![]),
        },
    };

    let mut response =
        ResponseObjectAssembly::new_send_rr_data(0x3, NO_ENCAPSULATION_TIMEOUT, forward_open_reply);
    if let CommandSpecificData::SendRrData(rr_data) = &mut response.command_specific_data {
        rr_data
            .items
            .push(CommonPacketItem::O2TSockAddrInfo(sample_address().into()));
    }

    let mut written_bytes: Vec<u8> = Vec::new();
    response
        .write(&mut std::io::Cursor::new(&mut written_bytes))
        .unwrap();

    // ...and the scanner reads it back as a response, with the lengths filled in
    let read_back =
        ResponseObjectAssembly::read_response(&mut std::io::Cursor::new(&written_bytes)).unwrap();

    assert_eq!(read_back.header.length, Some(40));
    assert_eq!(read_back.response(), response.response());
    assert_eq!(
        read_back.command_specific_data,
        response.command_specific_data
    );
    assert_eq!(read_back.sockaddr_info_items().count(), 1);
}
