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

    // let cip_string = CipShortString::new(String::from("ClearLink"));

    // let string_bytes: Vec<u8> = bincode::serialize(&cip_string).unwrap();

    // let string_bytes: Vec<u8> = String::from("ClearLink").as_bytes().to_vec();
    let string_bytes: Vec<u8> = String::from("the Answer").as_bytes().to_vec();

    // Assert equality
    assert_eq!(expected_byte_array, string_bytes);
}
