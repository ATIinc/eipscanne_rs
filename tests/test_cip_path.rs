mod common;

use std::io::{BufReader, Cursor};

use bilge::prelude::u7;
use binrw::{BinRead, BinResult, BinWrite};

use hex_assert::prelude::*;

use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::path::{
    CipPath, ElectronicKey, LogicalPathSegment, LogicalSegmentType, MajorRevisionCompatibility,
    PathData,
};
use eipscanne_rs::cip::types::CipByte;

use common::{
    FORWARD_OPEN_CONFIG_INSTANCE, FORWARD_OPEN_O2T_CONNECTION_POINT,
    FORWARD_OPEN_T2O_CONNECTION_POINT,
};

fn write_path(path: &CipPath) -> Vec<u8> {
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut byte_array_buffer);
    path.write(&mut writer).unwrap();
    byte_array_buffer
}

fn read_path(raw_bytes: &[u8]) -> BinResult<CipPath> {
    let byte_cursor = Cursor::new(raw_bytes);
    let mut buf_reader = BufReader::new(byte_cursor);
    CipPath::read_le_args(&mut buf_reader, ((raw_bytes.len() / 2) as u8,))
}

#[test]
fn test_assembly_connection_path() {
    /*
    Connection Path: Assembly, Instance: 0x97, Connection Point: 0x96, Connection Point: 0x64
        Path Segment: 0x20 (8-Bit Class Segment)
            Class: Assembly (0x04)
        Path Segment: 0x24 (8-Bit Instance Segment)
            Instance: 0x97
        Path Segment: 0x2c (8-Bit Connection Point Segment)
            Connection Point: 0x96
        Path Segment: 0x2c (8-Bit Connection Point Segment)
            Connection Point: 0x64

    Hex Dump:
    0000   20 04 24 97 2c 96 2c 64
    */
    let expected_byte_array: Vec<CipByte> = vec![0x20, 0x04, 0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64];

    let path = CipPath::new_assembly_connection(
        FORWARD_OPEN_CONFIG_INSTANCE,
        FORWARD_OPEN_O2T_CONNECTION_POINT,
        FORWARD_OPEN_T2O_CONNECTION_POINT,
    );
    assert_eq!(path.word_len(), 4);

    assert_eq_hex!(expected_byte_array, write_path(&path));
    assert_eq!(read_path(&expected_byte_array).unwrap(), path);
}

#[test]
fn test_assembly_connection_path_with_electronic_key() {
    /*
    Connection Path: [Key], Assembly, Instance: 0x04fe, Connection Point: 0x97, Connection Point: 0x98
        Path Segment: 0x34 (Electronic Key Segment)
            Key Format: 4
            Vendor ID: 0x0328
            Device Type: 0x0064
            Product Code: 11584
            Compatibility: Bit Cleared
            Major Revision: 1
            Minor Revision: 1
        Path Segment: 0x20 (8-Bit Class Segment)
            Class: Assembly (0x04)
        Path Segment: 0x25 (16-Bit Instance Segment)
            Instance: 0x04fe
        Path Segment: 0x2c (8-Bit Connection Point Segment)
            Connection Point: 0x97
        Path Segment: 0x2c (8-Bit Connection Point Segment)
            Connection Point: 0x98

    Hex Dump:
    0000   34 04 28 03 64 00 40 2d 01 01 20 04 25 00 fe 04
    0010   2c 97 2c 98
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x34, 0x04, 0x28, 0x03, 0x64, 0x00, 0x40, 0x2d, 0x01, 0x01, 0x20, 0x04, 0x25, 0x00, 0xfe,
        0x04, 0x2c, 0x97, 0x2c, 0x98,
    ];

    let path = CipPath::from_segments(vec![
        LogicalPathSegment::new_electronic_key(ElectronicKey {
            vendor_id: 808,
            device_type: 100,
            product_code: 11584,
            major_revision_compatibility: MajorRevisionCompatibility::builder()
                .major_revision(u7::new(1))
                .compatibility(false)
                .build(),
            minor_revision: 1,
        }),
        LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, ASSEMBLY_CLASS_ID),
        LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, 1278),
        LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, 151),
        LogicalPathSegment::new_u8(LogicalSegmentType::ConnectionPoint, 152),
    ]);
    assert_eq!(path.word_len(), 10);

    assert_eq_hex!(expected_byte_array, write_path(&path));
    assert_eq!(read_path(&expected_byte_array).unwrap(), path);
}

#[test]
fn test_electronic_key_compatibility_is_bit_7() {
    // Major revision 1 with the compatibility bit set
    let raw_bytes: Vec<CipByte> = vec![0x34, 0x04, 0x28, 0x03, 0x64, 0x00, 0x40, 0x2d, 0x81, 0x01];

    let path = read_path(&raw_bytes).unwrap();
    let [segment] = path.segments.as_slice() else {
        panic!("expected one segment, got {:?}", path.segments);
    };
    let PathData::ElectronicKey(electronic_key) = segment.data else {
        panic!("expected an electronic key, got {:?}", segment.data);
    };

    assert!(electronic_key.major_revision_compatibility.compatibility());
    assert_eq!(
        electronic_key.major_revision_compatibility.major_revision(),
        u7::new(1)
    );
}

#[test]
fn test_unsupported_segment_type_is_rejected() {
    // A port segment (segment type 0) is not a logical segment
    let raw_bytes: Vec<CipByte> = vec![0x01, 0x04];

    assert!(read_path(&raw_bytes).is_err());
}

#[test]
fn test_segment_overrunning_the_path_length_is_rejected() {
    let raw_bytes: Vec<CipByte> = vec![0x21, 0x00, 0x06, 0x00];

    let byte_cursor = Cursor::new(raw_bytes);
    let mut buf_reader = BufReader::new(byte_cursor);

    // The declared length (1 word) ends in the middle of the 16-bit class segment
    assert!(CipPath::read_le_args(&mut buf_reader, (1,)).is_err());
}
