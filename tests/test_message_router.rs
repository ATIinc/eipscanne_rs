use eipscanne_rs::cip::message::{MessageRouter, ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;

#[test]
fn test_serialize_service_container() {
    let expected_byte_array: Vec<CipByte> = vec![0x01];

    let service_container = ServiceContainer::new(ServiceCode::GetAttributeAll, false);

    let service_container_bytes = bincode::serialize(&service_container).unwrap();

    assert_eq!(expected_byte_array, service_container_bytes);
}

#[test]
fn test_deserialize_request_service_container() {
    let raw_byte_array: Vec<CipByte> = vec![0x1];

    let expected_service_container = ServiceContainer::new(ServiceCode::GetAttributeAll, false);

    let service_container: ServiceContainer = bincode::deserialize(&raw_byte_array).unwrap();

    assert_eq!(expected_service_container, service_container);
}

#[test]
fn test_deserialize_response_service_container() {
    let raw_byte_array: Vec<CipByte> = vec![0b10000101];

    let expected_service_container = ServiceContainer::new(ServiceCode::Reset, true);

    let service_container: ServiceContainer = bincode::deserialize(&raw_byte_array).unwrap();

    assert_eq!(expected_service_container, service_container);
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
        MessageRouter::new_request(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

    // Serialize the struct into a byte array
    let message_router_bytes = bincode::serialize(&message_router_request).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, message_router_bytes);
}
