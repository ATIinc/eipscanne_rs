use std::vec;

use binrw::{BinRead, BinWrite};

use bilge::prelude::u3;

use eipscanne_rs::cip::path::{
    CipFullPath, CipPath, CipPathBits, LogicalPathSegment, LogicalPathSegmentBits,
    LogicalSegmentFormat, LogicalSegmentType, SegmentType,
};
use eipscanne_rs::cip::types::CipByte;

#[test]
fn test_serialize_path_segment() {
    let sample_path_segment_bits = LogicalPathSegmentBits::new(
        LogicalSegmentFormat::FormatAsU16,
        LogicalSegmentType::ClassId,
        SegmentType::LogicalSegment,
        0x0,
        0x01,
    );

    let expected_bytes = vec![0x21, 0x0, 0x01, 0x0];

    let logical_path_segment = LogicalPathSegment::from(sample_path_segment_bits);

    let mut sample_path_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut sample_path_bytes);

    logical_path_segment.write(&mut writer).unwrap();

    assert_eq!(expected_bytes, sample_path_bytes);
}

#[test]
fn test_deserialize_path_segment() {
    let raw_bytes = vec![0x21, 0x00, 0x04, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let deserialized_path = LogicalPathSegment::read(&mut buf_reader).unwrap();

    let deserialized_path_bits = LogicalPathSegmentBits::from(deserialized_path);

    assert_eq!(
        deserialized_path_bits.segment_type(),
        SegmentType::LogicalSegment.into()
    );
    assert_eq!(
        deserialized_path_bits.logical_segment_type(),
        LogicalSegmentType::ClassId.into()
    );
    assert_eq!(
        deserialized_path_bits.logical_segment_format(),
        LogicalSegmentFormat::FormatAsU16.into()
    );
    assert_eq!(deserialized_path_bits.data(), 0x04);
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

    let mut cip_path_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut cip_path_bytes);

    cip_path.write(&mut writer).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, cip_path_bytes);
}

#[test]
fn test_serialize_cip_full_path() {
    /*
    Request Path: Assembly, Instance: 0x96, Attribute: 0x03
    Path Segment: 0x20 (8-Bit Class Segment)
        001. .... = Path Segment Type: Logical Segment (1)
        ...0 00.. = Logical Segment Type: Class ID (0)
        .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
        Class: Assembly (0x04)
    Path Segment: 0x24 (8-Bit Instance Segment)
        001. .... = Path Segment Type: Logical Segment (1)
        ...0 01.. = Logical Segment Type: Instance ID (1)
        .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
        Instance: 0x96
    Path Segment: 0x30 (8-Bit Attribute Segment)
        001. .... = Path Segment Type: Logical Segment (1)
        ...1 00.. = Logical Segment Type: Attribute ID (4)
        .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
        Attribute: 3

    -------------------------------------
    Hex Dump:

    0000   20 04 24 96 30 03

    */
    let expected_byte_array: Vec<CipByte> = vec![0x20, 0x04, 0x24, 0x96, 0x30, 0x03];

    let cip_full_path = CipFullPath::new(0x4, 0x96, 0x3);

    let mut cip_full_path_bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut cip_full_path_bytes);

    cip_full_path.write(&mut writer).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, cip_full_path_bytes);
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

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let cip_path = CipPath::read(&mut buf_reader).unwrap();

    let cip_path_bits = CipPathBits::from(cip_path);

    // Assert equality
    assert_eq!(cip_path_bits.class_id_segment.data(), 0x1);
    assert_eq!(
        cip_path_bits.class_id_segment.logical_segment_type(),
        LogicalSegmentType::ClassId
    );
    assert_eq!(cip_path_bits.instance_id_segment.data(), 0x1);
    assert_eq!(
        cip_path_bits.instance_id_segment.logical_segment_type(),
        LogicalSegmentType::InstanceId
    );
}

#[test]
fn test_deserialize_unknown_cip_path() {
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
    let raw_bytes: Vec<CipByte> = vec![0b10011001, 0x00, 0x01, 0x00, 0x25, 0x00, 0x01, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // Read from buffered reader
    let cip_path = CipPath::read(&mut buf_reader).unwrap();

    let cip_path_bits = CipPathBits::from(cip_path);

    // Assert equality
    assert_eq!(
        cip_path_bits.class_id_segment.segment_type(),
        SegmentType::Unknown(u3::new(0x4))
    );

    assert_eq!(cip_path_bits.class_id_segment.data(), 0x1);
    assert_eq!(
        cip_path_bits.class_id_segment.logical_segment_type(),
        LogicalSegmentType::Unknown(u3::new(0x6))
    );
    assert_eq!(cip_path_bits.instance_id_segment.data(), 0x1);
    assert_eq!(
        cip_path_bits.instance_id_segment.logical_segment_type(),
        LogicalSegmentType::InstanceId
    );
}
