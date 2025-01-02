use std::vec;

use eipscanne_rs::cip::path::{
    CipPath, LogicalPathSegment, LogicalSegmentFormat, LogicalSegmentType, SegmentType,
};
use eipscanne_rs::cip::types::CipByte;

#[test]
fn test_serialize_path_segment() {
    let sample_path_segment = LogicalPathSegment::new(
        LogicalSegmentFormat::FormatAsU16,
        LogicalSegmentType::ClassId,
        SegmentType::LogicalSegment,
        0x0,
        0x01,
    );

    let expected_bytes = vec![0x21, 0x0, 0x01, 0x0];

    // need variable_int_encoding to allow enums to be encoded/decoded as u8
    // let little_endian_option = bincode::DefaultOptions::new().with_variable_int_encoding();
    let sample_path_bytes = bincode::serialize(&sample_path_segment).unwrap();

    assert_eq!(expected_bytes, sample_path_bytes);
}

#[test]
fn test_deserialize_path_segment() {
    let raw_bytes = vec![0x21, 0x00, 0x04, 0x00];

    let deserialized_path: LogicalPathSegment = bincode::deserialize(&raw_bytes).unwrap();

    assert_eq!(
        deserialized_path.segment_type(),
        SegmentType::LogicalSegment.into()
    );
    assert_eq!(
        deserialized_path.logical_segment_type(),
        LogicalSegmentType::ClassId.into()
    );
    assert_eq!(
        deserialized_path.logical_segment_format(),
        LogicalSegmentFormat::FormatAsU16.into()
    );
    assert_eq!(deserialized_path.data(), 0x04);
}

#[test]
fn test_serialize_cip_path() {
    /*
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

    -------------------------------------
    Hex Dump:

    0000   21 00 01 00 25 00 01 00

    */
    let expected_byte_array: Vec<CipByte> = vec![0x21, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00];

    let cip_path = CipPath::new(0x1, 0x1);

    let path_bytes: Vec<u8> = bincode::serialize(&cip_path).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, path_bytes);
}

#[test]
fn test_deserialize_cip_path() {
    /*
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

    -------------------------------------
    Hex Dump:

    0000   21 00 01 00 25 00 01 00

    */
    let raw_bytes: Vec<CipByte> = vec![0x21, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00];

    let cip_path: CipPath = bincode::deserialize(&raw_bytes).unwrap();

    // Assert equality
    assert_eq!(cip_path.class_id_segment.data(), 0x1);
    assert_eq!(
        cip_path.class_id_segment.logical_segment_type(),
        LogicalSegmentType::ClassId
    );
    assert_eq!(cip_path.instance_id_segment.data(), 0x1);
    assert_eq!(
        cip_path.instance_id_segment.logical_segment_type(),
        LogicalSegmentType::InstanceId
    );
}
