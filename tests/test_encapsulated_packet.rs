use bincode::serialize;

use eipscanne_rs::{cip::types::CipUint, eip::packet::EncapsCommand};

/*
NOTE: To use another endianness, create an options object and call the serialize function with the options object.


    let big_endian_serializer = bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding();
    big_endian_serializer.serialize(&command).unwrap();

*/

#[test]
fn test_cast_encaps_command() {
    let command = EncapsCommand::RegisterSession;

    let expected_value = 0x0065;

    // Assert equality
    assert_eq!(expected_value, command as CipUint);
}

#[test]
fn test_serialize_encaps_command() {
    let command = EncapsCommand::RegisterSession;

    let command_byte_array = serialize(&command).unwrap();

    let expected_byte_array = vec![0x65, 0x00];

    // Assert equality
    assert_eq!(command_byte_array, expected_byte_array);
}
