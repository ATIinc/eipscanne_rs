use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::socket_addr::SocketAddrInfo;

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
