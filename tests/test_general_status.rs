use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::CipByte;

#[test]
fn test_deserialize_response_with_additional_status_and_data() {
    // General status 0x1E (embedded service error), two additional status words, two data bytes
    let raw_bytes: Vec<CipByte> = vec![0x8e, 0x00, 0x1e, 0x02, 0x09, 0x01, 0x34, 0x12, 0xaa, 0xbb];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    assert_eq!(
        response.service_container,
        ServiceContainer::new_response(ServiceCode::GetAttributeSingle)
    );
    assert_eq!(
        response.response_data.status,
        ResponseStatusCode::EmbeddedServiceError
    );
    assert_eq!(
        response.response_data.additional_status,
        vec![0x0109, 0x1234]
    );
    assert_eq!(
        response.response_data.data,
        CipDataOpt::Raw(vec![0xaa, 0xbb])
    );
}

#[test]
fn test_unknown_general_status_is_preserved() {
    let raw_bytes: Vec<CipByte> = vec![0x8e, 0x00, 0xd0, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    assert_eq!(
        response.response_data.status,
        ResponseStatusCode::Unknown(0xd0)
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    response.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}

#[test]
fn test_connection_manager_service_codes() {
    assert_eq!(
        u8::from(ServiceContainer::new_request(ServiceCode::ForwardOpen)),
        0x54
    );
    assert_eq!(
        u8::from(ServiceContainer::new_request(ServiceCode::LargeForwardOpen)),
        0x5b
    );
    assert_eq!(
        u8::from(ServiceContainer::new_response(ServiceCode::ForwardClose)),
        0xce
    );
}
