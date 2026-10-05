use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::path::{
    EPath, LogicalPathSegment, LogicalSegmentType, PathSegment, SimpleDataSegment,
};
use eipscanne_rs::cip::types::CipByte;

fn write_epath(path: &EPath) -> Vec<u8> {
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    path.write(&mut writer).unwrap();
    byte_array_buffer
}

fn read_epath(raw_bytes: &[u8]) -> binrw::BinResult<EPath> {
    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    EPath::read_le_args(&mut buf_reader, (raw_bytes.len() as u16,))
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

    let path = EPath::new_assembly_connection(0x97, 0x96, 0x64);
    assert_eq!(path.word_len(), 4);

    assert_eq_hex!(expected_byte_array, write_epath(&path));
    assert_eq!(read_epath(&expected_byte_array).unwrap(), path);
}

#[test]
fn test_class_instance_path_uses_16_bit_segments() {
    /*
    Request Path: Connection Manager, Instance: 0x0001
        Path Segment: 0x21 (16-Bit Class Segment)
            Class: Connection Manager (0x0006)
        Path Segment: 0x25 (16-Bit Instance Segment)
            Instance: 0x0001

    Hex Dump:
    0000   21 00 06 00 25 00 01 00
    */
    let expected_byte_array: Vec<CipByte> = vec![0x21, 0x00, 0x06, 0x00, 0x25, 0x00, 0x01, 0x00];

    let path = EPath::new_class_instance(0x06, 0x01);

    assert_eq_hex!(expected_byte_array, write_epath(&path));
    assert_eq!(read_epath(&expected_byte_array).unwrap(), path);
}

#[test]
fn test_path_with_simple_data_segment() {
    /*
    Path Segment: 0x20 (8-Bit Class Segment)
        Class: Assembly (0x04)
    Path Segment: 0x24 (8-Bit Instance Segment)
        Instance: 0x05
    Path Segment: 0x80 (Simple Data Segment)
        Data Size: 2 words
        Data: 0100 0200

    Hex Dump:
    0000   20 04 24 05 80 02 01 00 02 00
    */
    let expected_byte_array: Vec<CipByte> =
        vec![0x20, 0x04, 0x24, 0x05, 0x80, 0x02, 0x01, 0x00, 0x02, 0x00];

    let path = EPath::new(vec![
        PathSegment::Logical(LogicalPathSegment::new_u8(
            LogicalSegmentType::ClassId,
            0x04,
        )),
        PathSegment::Logical(LogicalPathSegment::new_u8(
            LogicalSegmentType::InstanceId,
            0x05,
        )),
        PathSegment::Data(SimpleDataSegment::new(vec![0x0001, 0x0002])),
    ]);
    assert_eq!(path.byte_len(), 10);
    assert_eq!(path.word_len(), 5);

    assert_eq_hex!(expected_byte_array, write_epath(&path));
    assert_eq!(read_epath(&expected_byte_array).unwrap(), path);
}

#[test]
fn test_unsupported_segment_type_is_rejected() {
    // A port segment (segment type 0) is neither a logical nor a data segment
    let raw_bytes: Vec<CipByte> = vec![0x01, 0x04];

    assert!(read_epath(&raw_bytes).is_err());
}

#[test]
fn test_segment_overrunning_the_path_length_is_rejected() {
    let raw_bytes: Vec<CipByte> = vec![0x21, 0x00, 0x06, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // The declared length ends in the middle of the 16-bit class segment
    assert!(EPath::read_le_args(&mut buf_reader, (3,)).is_err());
}
