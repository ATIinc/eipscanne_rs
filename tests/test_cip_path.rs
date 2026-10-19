mod common;

use binrw::{BinRead, BinWrite};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;

use common::{
    FORWARD_OPEN_CONFIG_INSTANCE, FORWARD_OPEN_O2T_CONNECTION_POINT,
    FORWARD_OPEN_T2O_CONNECTION_POINT,
};

fn write_path(path: &CipPath) -> Vec<u8> {
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    path.write(&mut writer).unwrap();
    byte_array_buffer
}

fn read_path(raw_bytes: &[u8]) -> binrw::BinResult<CipPath> {
    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
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
fn test_unsupported_segment_type_is_rejected() {
    // A port segment (segment type 0) is not a logical segment
    let raw_bytes: Vec<CipByte> = vec![0x01, 0x04];

    assert!(read_path(&raw_bytes).is_err());
}

#[test]
fn test_segment_overrunning_the_path_length_is_rejected() {
    let raw_bytes: Vec<CipByte> = vec![0x21, 0x00, 0x06, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    // The declared length (1 word) ends in the middle of the 16-bit class segment
    assert!(CipPath::read_le_args(&mut buf_reader, (1,)).is_err());
}
