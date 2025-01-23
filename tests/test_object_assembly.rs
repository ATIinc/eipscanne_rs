use binrw::BinWrite;

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::packet::EnIpPacketDescription;

#[test]
fn test_write_output_assembly_object_request() {
    /*

    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 24
            Session Handle: 0x00000003
            Status: Success (0x00000000)
            Sender Context: 0000000000000000
            Options: 0x00000000
        Command Specific Data
            Interface Handle: CIP (0x00000000)
            Timeout: 0
            Item Count: 2
                Type ID: Null Address Item (0x0000)
                    Length: 0
                Type ID: Unconnected Data Item (0x00b2)
                    Length: 8
            [Response In: 12]

    Common Industrial Protocol
        Service: Get Attribute Single (Request)
            0... .... = Request/Response: Request (0x0)
            .000 1110 = Service: Get Attribute Single (0x0e)
        Request Path Size: 3 words
        Request Path: Assembly, Instance: 0x70, Attribute: 0x03
            Path Segment: 0x20 (8-Bit Class Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 00.. = Logical Segment Type: Class ID (0)
                .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                Class: Assembly (0x04)
            Path Segment: 0x24 (8-Bit Instance Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 01.. = Logical Segment Type: Instance ID (1)
                .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                Instance: 0x70
            Path Segment: 0x30 (8-Bit Attribute Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...1 00.. = Logical Segment Type: Attribute ID (4)
                .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                Attribute: 3
        Get Attribute Single (Request)


        -------------------------------------
        Hex Dump:

        0000   6f 00 18 00 03 00 00 00 00 00 00 00 00 00 00 00
        0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
        0020   00 00 00 00 b2 00 08 00 0e 03 20 04 24 70 30 03


    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x18, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x08, 0x00, 0x0e, 0x03, 0x20, 0x04, 0x24,
        0x70, 0x30, 0x03,
    ];

    let provided_session_handle = 0x3;

    let full_path_request = MessageRouterRequest::new(
        ServiceCode::GetAttributeSingle,
        CipPath::new_full(0x4, 0x70, 0x3),
    );

    let set_digital_output_object = eipscanne_rs::object_assembly::RequestObjectAssembly {
        packet_description: EnIpPacketDescription::new_cip_description(provided_session_handle, 0),
        cip_message: Some(full_path_request),
    };

    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    set_digital_output_object.write(&mut writer).unwrap();

    // Assert equality
    assert_eq_hex!(expected_byte_array, byte_array_buffer);
}
