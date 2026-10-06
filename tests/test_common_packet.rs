use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::description::{CommonPacketItem, CommonPacketItemId};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::eip::sockaddr::SockaddrInfo;
use eipscanne_rs::object_assembly::ResponseObjectAssembly;

/// Items of a response packet
type ResponseItem = CommonPacketItem<MessageRouterResponse>;

fn sample_address() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 0x08AE)
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

    let sockaddr_info = SockaddrInfo::new(sample_address());

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

    let item = ResponseItem::new_t2o_sockaddr_info(sample_address());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    item.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = ResponseItem::read(&mut buf_reader).unwrap();

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
    let item = ResponseItem::read(&mut buf_reader).unwrap();

    assert_eq!(
        item,
        ResponseItem::Unknown {
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
            session_handle: 0x3,
            status_code: EncapsStatusCode::Success,
            sender_context: [0x0; 8],
            options: 0x0,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new(
            0x0,
            0,
            vec![
                CommonPacketItem::NullAddress,
                CommonPacketItem::UnconnectedData(MessageRouterResponse {
                    service_container: ServiceContainer::new_response(ServiceCode::ForwardOpen),
                    response_data: ResponseData {
                        status: ResponseStatusCode::Success,
                        additional_status_size: 0,
                        additional_status: vec![],
                        data: CipDataOpt::Raw(vec![]),
                    },
                }),
                CommonPacketItem::new_o2t_sockaddr_info(sample_address()),
            ],
        )),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response_object = ResponseObjectAssembly::read(&mut buf_reader).unwrap();

    assert_eq!(expected_response, response_object);
    assert_eq!(response_object.sockaddr_info_items().count(), 1);

    // Writing the parsed object must reproduce the packet, including the lengths and the item count
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    response_object.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}
