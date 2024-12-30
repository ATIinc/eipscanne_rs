use std::vec;

use eipscanne_rs::cip::message::CipPath;
use eipscanne_rs::cip::types::CipByte;

use deku::prelude::DekuContainerRead;

#[test]
fn test_from_bytes_cip_path() {
    let byte_offset = 0;
    let raw_bytes = vec![0x21, 0x00];

    let ((_remaining_bytes, _remaining_byte_size), deserialized_path) =
        CipPath::from_bytes((raw_bytes.as_ref(), byte_offset)).unwrap();

    assert_eq!(deserialized_path.logical_segment, 0x01);
    assert_eq!(deserialized_path.class_id, 0x00);
    assert_eq!(deserialized_path.instance_id, 0x01);
}

#[test]
fn test_deserialize_cip_path() {
    let raw_bytes = vec![0x21, 0x04];

    let deserialized_path: CipPath = bincode::deserialize(&raw_bytes).unwrap();

    assert_eq!(deserialized_path.logical_segment, 0x01);
    assert_eq!(deserialized_path.class_id, 0x00);
    assert_eq!(deserialized_path.instance_id, 0x01);
    assert_eq!(deserialized_path.attribute_id, 0x04);
}

#[test]
fn test_serialize_cip_path() {
    /*
    Path Segment: 0x21 (16-Bit Class Segment)
        001. .... = Path Segment Type: Logical Segment (1)
        ...0 00.. = Logical Segment Type: Class ID (0)
        .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)

    Get Attributes All (Request)

    -------------------------------------
    Hex Dump:

    0000   21 00 01 00

    */

    let expected_byte_array: Vec<CipByte> = vec![0x21, 0x00];

    let cip_path = CipPath {
        logical_segment: 0x01,
        class_id: 0x00,
        instance_id: 0x01,
        attribute_id: 0x00,
    };

    let path_bytes: Vec<u8> = bincode::serialize(&cip_path).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, path_bytes);
}
