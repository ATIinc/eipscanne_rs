use eipscanne_rs::cip::message::{CipPath, MessageRouterRequest, ServiceCode};
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::cip_data::CipDataPacket;

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

    let message_router = MessageRouterRequest {
            service_code: ServiceCode::GetAttributeAll,
            path: CipPath {
                logical_segment: 0x01,
                class_id: 0x01,
                instance_id: 0x01,
                attribute_id: None,
            },
        };

    let cip_data_packet = CipDataPacket::new(
        data: vec![],
        use_8_bit_path_segments: true,
    });

    // Serialize the struct into a byte array
    let data_packet_bytes = serialize(&cip_data_packet).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, data_packet_bytes);
}
