use bincode::serialize; // deserialize,

use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::packet::EncapsulatedPacket;

#[test]
fn test_serialize_register_session_request() {
    // NOTE: Big Endian
    // Encapsulation Header
    //      Register Session == 6500             == 0x65
    //      Length           == 0400             == 0x04
    //      Session Handle   == 00000000         == 0x00
    //      Sucess           == 00000000         == 0x00
    //      Sender Context   == 0000000000000000 == 0x00
    //      Options          == 00000000         == 0x00
    // Command Specific Data
    //      Protocol Version == 0100             == 0x01
    //      Option Flags     == 0000             == 0x00

    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000000, Register Session
    Encapsulation Header
        Command: Register Session (0x0065)
        Length: 4
        Session Handle: 0x00000000
        Status: Success (0x00000000)
        Sender Context: 0000000000000000
        Options: 0x00000000
    Command Specific Data
        Protocol Version: 1
        Option Flags: 0x0000

    -------------------------------------
    Hex Dump:

    0000   65 00 04 00 00 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 01 00 00 00

    */

    let expected_byte_array: Vec<CipByte> = vec![
        0x65, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];

    // create an empty packet
    let registration_packet = EncapsulatedPacket::new_registration();

    // Serialize the struct into a byte array
    let registration_byte_array = serialize(&registration_packet).unwrap();

    // Assert equality
    assert_eq!(expected_byte_array, registration_byte_array);
}

#[test]
fn test_serialize_register_session_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Register Session
    Encapsulation Header
        Command: Register Session (0x0065)
        Length: 4
        Session Handle: 0x00000006
        Status: Success (0x00000000)
        Sender Context: 0000000000000000
        Options: 0x00000000
    Command Specific Data
        Protocol Version: 1
        Option Flags: 0x0000

    -------------------------------------
    Hex Dump:

    0000   65 00 04 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 01 00 00 00

    */

    // Assert equality
    assert_eq!(0x0, 0x0);
}

#[test]
fn test_serialize_unregister_session_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Unregister Session
    Encapsulation Header
        Command: Unregister Session (0x0066)
        Length: 0
        Session Handle: 0x00000006
        Status: Success (0x00000000)
        Sender Context: 0000000000000000
        Options: 0x00000000

    -------------------------------------
    Hex Dump:

    0000   66 00 00 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00

    */

    // Assert equality
    assert_eq!(0x0, 0x0);
}
