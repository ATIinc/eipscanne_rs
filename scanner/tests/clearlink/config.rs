//! Byte-exact tests of the ClearLink config assembly against captured packets

use std::vec;

use binrw::BinWrite;

use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseData};
use hex_test_macros::prelude::*;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::object_assembly::ResponseObjectAssembly;

use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    NO_ENCAPSULATION_TIMEOUT,
};

use crate::assemblies::clearlink::config::{CONFIG_ASSEMBLY_INSTANCE, ConfigAssemblyObject};

/// Session handle of the captures below
const CAPTURE_SESSION_HANDLE: CipUdint = 0x03;

#[test]
fn test_write_clearlink_config_assembly_object() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 256
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
                    Length: 240
            [Response In: 10]
    Common Industrial Protocol
        Service: Set Attribute Single (Request)
        Request Path Size: 3 words
        Request Path: Assembly, Instance: 0x96, Attribute: 0x03
        Set Attribute Single (Request)
            Data: 64646464640000000a0a0a0a102710271027102710271027102710271027102710271027…



    -------------------------------------
    Hex Dump:

    0000   6f 00 00 01 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 f0 00 10 03 20 04 24 96 30 03
    0030   64 64 64 64 64 00 00 00 0a 0a 0a 0a 10 27 10 27
    0040   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
    0050   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
    0060   10 27 10 27 10 27 10 27 10 27 10 27 10 27 10 27
    0070   0a 0a 0a 0a 0a 0a 0a 0a 64 00 00 00 05 00 00 00
    0080   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
    0090   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
    00a0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
    00b0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
    00c0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
    00d0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
    00e0   08 00 00 00 01 00 00 00 01 00 00 00 80 96 98 00
    00f0   00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff 00
    0100   00 c2 01 00 00 00 00 00 00 00 00 00 00 00 00 00
    0110   00 00 00 00 0a 00 00 00


    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0xf0, 0x00, 0x10, 0x03, 0x20, 0x04, 0x24,
        0x96, 0x30, 0x03, 0x64, 0x64, 0x64, 0x64, 0x64, 0x00, 0x00, 0x00, 0x0a, 0x0a, 0x0a, 0x0a,
        0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10,
        0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27,
        0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x10,
        0x27, 0x10, 0x27, 0x10, 0x27, 0x10, 0x27, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a,
        0x64, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08, 0x00, 0x00,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0x96, 0x98, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0x00, 0x00, 0xc2, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
    ];

    let provided_session_handle = CAPTURE_SESSION_HANDLE;

    let set_clearlink_config_message = MessageRouterRequest::new_data(
        ServiceCode::SetAttributeSingle,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            CONFIG_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        Some(Box::new(ConfigAssemblyObject::default())),
    );

    let set_clearlink_config_object =
        eipscanne_rs::object_assembly::RequestObjectAssembly::new_send_rr_data(
            provided_session_handle,
            NO_ENCAPSULATION_TIMEOUT,
            set_clearlink_config_message,
        );

    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    set_clearlink_config_object.write(&mut writer).unwrap();

    // Assert equality
    assert_eq_hex!(expected_byte_array, byte_array_buffer);
}

#[test]
fn test_read_clearlink_config_assembly_object_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 20
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
                    Length: 4
            [Request In: 9]
            [Time: 0.000727724 seconds]
    Common Industrial Protocol
        Service: Set Attribute Single (Response)
            1... .... = Request/Response: Response (0x1)
            .001 0000 = Service: Set Attribute Single (0x10)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words
        [Request Path Size: 3 words]
        [Request Path: Assembly, Instance: 0x96, Attribute: 0x03]
        Set Attribute Single (Response)

    -------------------------------------
    Hex Dump:

    0000   6f 00 14 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 04 00 90 00 00 00

    */

    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x14, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x04, 0x00, 0x90, 0x00, 0x00, 0x00,
    ];

    let expected_set_config_assembly_response = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(20),
            session_handle: CAPTURE_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::SetAttributeSingle),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0,
                    additional_status: vec![],
                    data: CipDataOpt::Raw(vec![]),
                },
            },
        )),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let response_object = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    assert_eq!(expected_set_config_assembly_response, response_object);
}
