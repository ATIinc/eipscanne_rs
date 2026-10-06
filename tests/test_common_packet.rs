use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::sockaddr::SockaddrInfo;
use eipscanne_rs::object_assembly::ResponseObjectAssembly;

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
