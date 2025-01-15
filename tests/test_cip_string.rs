use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::types::{CipByte, CipShortString};

#[test]
fn test_serialize_cip_string() {
    /*
    Attribute: 7 (Product Name)
    Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   09 43 6c 65 61 72 4c 69 6e 6b

    */
    let expected_byte_array: Vec<CipByte> =
        vec![0x09, 0x43, 0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b];

    let cip_string = CipShortString::new(String::from("ClearLink"));

    // Write the cip_string binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    cip_string.write(&mut writer).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, byte_array_buffer);
}

#[test]
fn test_deserialize_cip_string() {
    /*
    Attribute: 7 (Product Name)
    Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   09 43 6c 65 61 72 4c 69 6e 6b

    */
    let raw_bytes: Vec<CipByte> = vec![0x09, 0x43, 0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b];

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let cip_string = CipShortString::read(&mut buf_reader).unwrap();

    let expected_cip_string = CipShortString::new(String::from("ClearLink"));

    // Assert equality
    assert_eq!(expected_cip_string, cip_string);
}
