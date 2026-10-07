mod common;

use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::CommandSpecificData;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::constants::NO_ENCAPSULATION_TIMEOUT;
use eipscanne_rs::eip::socket_addr::SocketAddrInfo;
use eipscanne_rs::object_assembly::ResponseObjectAssembly;

fn sample_address() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), ETHERNET_IP_IO_UDP_PORT)
}

#[test]
fn test_serialize_socket_addr_info_big_endian_fields() {
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

    let socket_addr_info = SocketAddrInfo::from(sample_address());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    socket_addr_info.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = SocketAddrInfo::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, socket_addr_info);
    assert_eq!(deserialized.socket_address(), sample_address());
}

/// The data of a successful Forward_Open reply: connection ids, serial numbers and actual packet
/// intervals echoed by the target (same exchange as tests/test_forward_open.rs)
fn forward_open_success_body() -> Vec<CipByte> {
    vec![
        0xd4, 0xc3, 0xb2, 0xa1, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01,
        0x00, 0x40, 0x42, 0x0f, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x00, 0x00,
    ]
}

#[test]
fn test_response_assembly_with_trailing_sockaddr_item() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 66
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
                    Length: 30
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
        Forward Open (Response)
            O->T Network Connection ID: 0xa1b2c3d4
            T->O Network Connection ID: 0x12345678
            Connection Serial Number: 0x0001
            Originator Vendor ID: 0x0156
            Originator Serial Number: 0x00012345
            O->T API: 1000.000ms
            T->O API: 1000.000ms
            Application Reply Size: 0 words
            Reserved: 0x00

    Hex Dump:
    0000   6f 00 42 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 03 00
    0020   00 00 00 00 b2 00 1e 00 d4 00 00 00 d4 c3 b2 a1
    0030   78 56 34 12 01 00 56 01 45 23 01 00 40 42 0f 00
    0040   40 42 0f 00 00 00 00 80 10 00 00 02 08 ae c0 a8
    0050   01 0a 00 00 00 00 00 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x42, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1e, 0x00, 0xd4, 0x00, 0x00, 0x00, 0xd4,
        0xc3, 0xb2, 0xa1, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00,
        0x40, 0x42, 0x0f, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x80, 0x10, 0x00, 0x00,
        0x02, 0x08, 0xae, 0xc0, 0xa8, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response_object = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    assert!(response_object.sockaddr_info_items().unwrap().o2t.is_some());

    // Writing the parsed object must reproduce the packet, including the lengths and the item count
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    response_object.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
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
                data: CipDataOpt::Raw(forward_open_success_body()),
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
            data: CipDataOpt::Raw(forward_open_success_body()),
        },
    };

    let mut response =
        ResponseObjectAssembly::new_send_rr_data(0x3, NO_ENCAPSULATION_TIMEOUT, forward_open_reply);
    if let CommandSpecificData::SendRrData(rr_data) = &mut response.command_specific_data {
        rr_data.sockaddr_info_items.o2t = Some(sample_address().into());
    }

    let mut written_bytes: Vec<u8> = Vec::new();
    response
        .write(&mut std::io::Cursor::new(&mut written_bytes))
        .unwrap();

    // ...and the scanner reads it back as a response, with the lengths filled in
    let read_back =
        ResponseObjectAssembly::read_response(&mut std::io::Cursor::new(&written_bytes)).unwrap();

    assert_eq!(read_back.header.length, Some(66));
    assert_eq!(read_back.response(), response.response());
    assert!(read_back.sockaddr_info_items().unwrap().o2t.is_some());
}
