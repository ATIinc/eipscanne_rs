use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::request::{MessageRouterRequest, RequestData};
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};

use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::cip::message::data::CipDataOpt;

#[test]
fn test_serialize_service_container() {
    let expected_byte_array: Vec<CipByte> = vec![0x01];

    let service_container_bits = ServiceContainer::new(ServiceCode::GetAttributeAll, false);
    let service_container = ServiceContainer::from(service_container_bits);

    let mut service_container_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut service_container_bytes);

    service_container.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, service_container_bytes);
}

#[test]
fn test_deserialize_request_service_container() {
    let expected_service_container = ServiceContainer::new(ServiceCode::GetAttributeAll, false);

    let raw_byte_array: Vec<CipByte> = vec![0x1];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let deserialized_service_container = ServiceContainer::read(&mut buf_reader).unwrap();

    assert_eq!(expected_service_container, deserialized_service_container);
}

#[test]
fn test_deserialize_response_service_container() {
    let expected_service_container = ServiceContainer::new(ServiceCode::Reset, true);

    let raw_byte_array: Vec<CipByte> = vec![0b10000101];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let deserialized_service_container = ServiceContainer::read(&mut buf_reader).unwrap();

    assert_eq!(expected_service_container, deserialized_service_container);
}

#[test]
fn test_serialize_get_attributes_all_request() {
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
        MessageRouterRequest::new(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

    let mut message_router_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut message_router_bytes);

    message_router_request.write(&mut writer).unwrap();

    // Assert equality
    assert_eq_hex!(expected_byte_array, message_router_bytes);
}

#[test]
fn test_deserialize_empty_response() {
    let raw_byte_array: Vec<CipByte> = vec![0x81, 0x00, 0x00, 0x00, 0x04];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let message_router_response_length: u16 = 4;

    let message_router_response = MessageRouterResponse::read_args(&mut buf_reader, (message_router_response_length,)).unwrap();

    let expected_message_router_response = MessageRouterResponse {
        service_container: ServiceContainer::from(ServiceContainer::new(
            ServiceCode::GetAttributeAll,
            true,
        )),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0x0,
            data: CipDataOpt::Raw(vec![]),
        },
    };

    // Assert equality
    assert_eq!(expected_message_router_response, message_router_response);
}

#[test]
fn test_message_cip_path_byte_size() {
    let message_router_request = MessageRouterRequest {
        service_container: ServiceContainer::from(ServiceContainer::new(
            ServiceCode::GetAttributeAll,
            false,
        )),
        request_data: RequestData::new(None, CipPath::new(0x1, 0x1), None),
    };

    let mut tmp_output_buffer: Vec<u8> = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut tmp_output_buffer);
    let _ = message_router_request.write(&mut temp_writer);

    // Assert equality
    assert_eq!(10, tmp_output_buffer.len());
}

#[test]
fn test_message_cip_path_request_byte_size() {
    let message_router_request =
        MessageRouterRequest::new(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

    // Assert equality
    let mut tmp_output_buffer: Vec<u8> = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut tmp_output_buffer);
    let _ = message_router_request.write(&mut temp_writer);

    assert_eq!(10, tmp_output_buffer.len());
}

#[test]
fn test_message_cip_full_path_request_bytes() {
    /*
    Common Industrial Protocol
    Service: Get Attribute Single (Request)
        0... .... = Request/Response: Request (0x0)
        .000 1110 = Service: Get Attribute Single (0x0e)
    Request Path Size: 3 words
    Request Path: Assembly, Instance: 0x70, Attribute: 0x03
        Path Segment: 0x20 (8-Bit Class Segment)
            001. .... = Path Segment Type: Logical Segment (1)
            ...0 00.. = Logical Segment Type: Class ID (0)
            .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
            Class: Assembly (0x04)
        Path Segment: 0x24 (8-Bit Instance Segment)
            001. .... = Path Segment Type: Logical Segment (1)
            ...0 01.. = Logical Segment Type: Instance ID (1)
            .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
            Instance: 0x70
        Path Segment: 0x30 (8-Bit Attribute Segment)
            001. .... = Path Segment Type: Logical Segment (1)
            ...1 00.. = Logical Segment Type: Attribute ID (4)
            .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
            Attribute: 3
    Get Attribute Single (Request)

    -------------------------------------
    Hex Dump:

    0000   0e 03 20 04 24 70 30 03

    */

    let expected_byte_array = vec![0x0e, 0x03, 0x20, 0x04, 0x24, 0x70, 0x30, 0x03];

    let message_router_request = MessageRouterRequest::new(
        ServiceCode::GetAttributeSingle,
        CipPath::new_full(0x4, 0x70, 0x3),
    );

    // Assert equality
    let mut tmp_output_buffer: Vec<u8> = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut tmp_output_buffer);
    let _ = message_router_request.write(&mut temp_writer);

    assert_eq!(expected_byte_array, tmp_output_buffer);
}

#[test]
fn test_message_cip_full_path_request_byte_size() {
    let message_router_request = MessageRouterRequest::new(
        ServiceCode::GetAttributeSingle,
        CipPath::new_full(0x1, 0x2, 0x3),
    );

    // Assert equality
    let mut tmp_output_buffer: Vec<u8> = Vec::new();
    let mut temp_writer = std::io::Cursor::new(&mut tmp_output_buffer);
    let _ = message_router_request.write(&mut temp_writer);

    assert_eq!(8, tmp_output_buffer.len());
}
