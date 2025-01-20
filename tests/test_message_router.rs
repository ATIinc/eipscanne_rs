use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::message::{
    MessageRouterRequest, MessageRouterResponse, RequestData, ResponseData, ServiceCode,
    ServiceContainer, ServiceContainerBits,
};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;

#[test]
fn test_serialize_service_container() {
    let expected_byte_array: Vec<CipByte> = vec![0x01];

    let service_container_bits = ServiceContainerBits::new(ServiceCode::GetAttributeAll, false);
    let service_container = ServiceContainer::from(service_container_bits);

    let mut service_container_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut service_container_bytes);

    service_container.write(&mut writer).unwrap();

    assert_eq!(expected_byte_array, service_container_bytes);
}

#[test]
fn test_deserialize_request_service_container() {
    let expected_service_container = ServiceContainerBits::new(ServiceCode::GetAttributeAll, false);

    let raw_byte_array: Vec<CipByte> = vec![0x1];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let deserialized_service_container = ServiceContainer::read(&mut buf_reader).unwrap();

    let deserialized_service_container_bits =
        ServiceContainerBits::from(deserialized_service_container);

    assert_eq!(
        expected_service_container,
        deserialized_service_container_bits
    );
}

#[test]
fn test_deserialize_response_service_container() {
    let expected_service_container = ServiceContainerBits::new(ServiceCode::Reset, true);

    let raw_byte_array: Vec<CipByte> = vec![0b10000101];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let deserialized_service_container = ServiceContainer::read(&mut buf_reader).unwrap();

    let deserialized_service_container_bits =
        ServiceContainerBits::from(deserialized_service_container);

    assert_eq!(
        expected_service_container,
        deserialized_service_container_bits
    );
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
    assert_eq!(expected_byte_array, message_router_bytes);
}

#[test]
fn test_deserialize_get_attributes_all_response() {
    let raw_byte_array: Vec<CipByte> =
        vec![0x01, 0x04, 0x21, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let message_router_response = MessageRouterResponse::read(&mut buf_reader).unwrap();

    let expected_message_router_response = MessageRouterResponse {
        service_container: ServiceContainer::from(ServiceContainerBits::new(
            ServiceCode::GetAttributeAll,
            true,
        )),
        router_data: ResponseData {
            status: 0x0,
            additional_status_size: 0x0,
            data: CipPath::new(0x1, 0x1),
        },
    };

    // Assert equality
    assert_eq!(expected_message_router_response, message_router_response);
}

#[test]
fn test_deserialize_empty_response() {
    let raw_byte_array: Vec<CipByte> = vec![0x81, 0x00, 0x00, 0x00, 0x04];

    let byte_cursor = std::io::Cursor::new(raw_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let message_router_response = MessageRouterResponse::<u8>::read(&mut buf_reader).unwrap();

    let expected_message_router_response = MessageRouterResponse {
        service_container: ServiceContainer::from(ServiceContainerBits::new(
            ServiceCode::GetAttributeAll,
            true,
        )),
        router_data: ResponseData {
            status: 0x0,
            additional_status_size: 0x0,
            data: 0x4,
        },
    };

    // Assert equality
    assert_eq!(expected_message_router_response, message_router_response);
}

#[test]
fn test_message_cip_path_byte_size() {
    let message_router_request = MessageRouterRequest {
        service_container: ServiceContainer::from(ServiceContainerBits::new(
            ServiceCode::GetAttributeAll,
            false,
        )),
        router_data: RequestData {
            data_word_size: 4,
            data: CipPath::new(0x1, 0x1),
        },
    };

    // Assert equality
    assert_eq!(10, message_router_request.byte_size());
}

#[test]
fn test_message_cip_path_request_byte_size() {
    let message_router_request =
        MessageRouterRequest::new(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

    // Assert equality
    assert_eq!(10, message_router_request.byte_size());
}
