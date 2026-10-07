mod common;

use binrw::{BinRead, BinWrite};

use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::types::{CipByte, CipShortString};
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::description::{CommonPacketDescriptor, CommonPacketItemId};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::eip::sockaddr::SockaddrInfoItems;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

use common::IDENTITY_SESSION_HANDLE;

#[test]
fn test_deserialize_device_type() {
    let generic_device_type_bytes: Vec<CipByte> = vec![0x2b, 0x00];
    let generic_device_type =
        DeviceType::read(&mut std::io::Cursor::new(generic_device_type_bytes)).unwrap();
    let expected_generic_device_type = DeviceType::GenericDevice;
    assert_eq!(expected_generic_device_type, generic_device_type);

    let unknown_device_type_bytes: Vec<CipByte> = vec![0x4b, 0x00];
    let unknown_device_type =
        DeviceType::read(&mut std::io::Cursor::new(unknown_device_type_bytes)).unwrap();
    let expected_unknown_device_type = DeviceType::Unknown(0x4b);
    assert_eq!(expected_unknown_device_type, unknown_device_type);
}

#[test]
fn test_serialize_new_identity_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Send RR Data
    Encapsulation Header
        Command: Send RR Data (0x006f)
        Length: 26
        Session Handle: 0x00000006
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
                Length: 10
        [Response In: 8]

    -------------------------------------
    Hex Dump:

    0000   6f 00 1a 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 0a 00 01 04 21 00 01 00 25 00
    0030   01 00

    */

    /*
    Common Industrial Protocol
    Service: Get Attributes All (Request)
        0... .... = Request/Response: Request (0x0)
        .000 0001 = Service: Get Attributes All (0x01)
    Request Path Size: 4 words
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
    Get Attributes All (Request)

    -------------------------------------
    Hex Dump:

    0000   01 04 21 00 01 00 25 00 01 00

    */
    let expected_identity_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1a, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0a, 0x00, 0x01, 0x04, 0x21, 0x00, 0x01,
        0x00, 0x25, 0x00, 0x01, 0x00,
    ];

    // create an empty packet
    let identity_request_packet = RequestObjectAssembly::new_identity(IDENTITY_SESSION_HANDLE);

    let mut identity_byte_array: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut identity_byte_array);

    identity_request_packet.write(&mut writer).unwrap();

    assert_eq!(expected_identity_byte_array, identity_byte_array);
}

#[test]
fn test_deserialize_just_cip_identity_response() {
    /*
        Attribute: 1 (Vendor ID)
            Vendor ID: Teknic, Inc. (0x01a8)
        Attribute: 2 (Device Type)
            Device Type: Generic Device (keyable) (0x002b)
        Attribute: 3 (Product Code)
            Product Code: 1
        Attribute: 4 (Revision)
            Major Revision: 2
            Minor Revision: 93
        Attribute: 5 (Status)
            Status: 0x0000
                .... .... .... ...0 = Owned: 0
                .... .... .... .0.. = Configured: 0
                .... .... 0000 .... = Extended Device Status: 0x0
                .... ...0 .... .... = Minor Recoverable Fault: 0
                .... ..0. .... .... = Minor Unrecoverable Fault: 0
                .... .0.. .... .... = Major Recoverable Fault: 0
                .... 0... .... .... = Major Unrecoverable Fault: 0
                0000 .... .... .... = Extended Device Status 2: 0x0
        Attribute: 6 (Serial Number)
            Serial Number: 0x01ff3d32
        Attribute: 7 (Product Name)
            Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   a8 01 2b 00 01 00 02 5d 00 00 32 3d
    0010   ff 01 09 43 6c 65 61 72 4c 69 6e 6b

    */

    let identity_response_bytes: Vec<u8> = vec![
        0xa8, 0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32, 0x3d, 0xff, 0x01, 0x09,
        0x43, 0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
    ];

    let byte_cursor = std::io::Cursor::new(identity_response_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let cip_identity_response = IdentityResponse::read(&mut buf_reader).unwrap();

    let expected_cip_identity_response = IdentityResponse {
        vendor_id: VendorId::TeknicInc,
        device_type: DeviceType::GenericDevice,
        product_code: 0x1,
        revision: Revision {
            major: 2,
            minor: 93,
        },
        status: IdentityStatusBits::default().into(),
        serial_number: 0x01ff3d32,
        product_name: CipShortString::from("ClearLink".to_string()),
    };

    // Assert equality
    assert_eq!(expected_cip_identity_response, cip_identity_response);
}

#[test]
fn test_deserialize_cip_identity_response() {
    /*
    Common Industrial Protocol
    Service: Get Attributes All (Response)
        1... .... = Request/Response: Response (0x1)
        .000 0001 = Service: Get Attributes All (0x01)
    Status: Success:
        General Status: Success (0x00)
        Additional Status Size: 0 words
    [Request Path Size: 4 words]
    [Request Path: Identity, Instance: 0x0001]
        [Path Segment: 0x21 (16-Bit Class Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 00.. = Logical Segment Type: Class ID (0)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Class: Identity (0x0001)]
        [Path Segment: 0x25 (16-Bit Instance Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 01.. = Logical Segment Type: Instance ID (1)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Instance: 0x0001]
    Get Attributes All (Response)
        Attribute: 1 (Vendor ID)
            Vendor ID: Teknic, Inc. (0x01a8)
        Attribute: 2 (Device Type)
            Device Type: Generic Device (keyable) (0x002b)
        Attribute: 3 (Product Code)
            Product Code: 1
        Attribute: 4 (Revision)
            Major Revision: 2
            Minor Revision: 93
        Attribute: 5 (Status)
            Status: 0x0000
                .... .... .... ...0 = Owned: 0
                .... .... .... .0.. = Configured: 0
                .... .... 0000 .... = Extended Device Status: 0x0
                .... ...0 .... .... = Minor Recoverable Fault: 0
                .... ..0. .... .... = Minor Unrecoverable Fault: 0
                .... .0.. .... .... = Major Recoverable Fault: 0
                .... 0... .... .... = Major Unrecoverable Fault: 0
                0000 .... .... .... = Extended Device Status 2: 0x0
        Attribute: 6 (Serial Number)
            Serial Number: 0x01ff3d32
        Attribute: 7 (Product Name)
            Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   81 00 00 00 a8 01 2b 00 01 00 02 5d 00 00 32 3d
    0010   ff 01 09 43 6c 65 61 72 4c 69 6e 6b

    */

    let message_router_response_length: u16 = 28;

    let identity_response_bytes: Vec<u8> = vec![
        0x81, 0x00, 0x00, 0x00, 0xa8, 0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32,
        0x3d, 0xff, 0x01, 0x09, 0x43, 0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
    ];

    let byte_cursor = std::io::Cursor::new(identity_response_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let cip_identity_response =
        MessageRouterResponse::read_args(&mut buf_reader, (message_router_response_length,))
            .unwrap();

    let expected_cip_identity_response = MessageRouterResponse {
        service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0x0,
            additional_status: vec![],
            data: CipDataOpt::Typed(Box::new(IdentityResponse {
                vendor_id: VendorId::TeknicInc,
                device_type: DeviceType::GenericDevice,
                product_code: 0x1,
                revision: Revision {
                    major: 2,
                    minor: 93,
                },
                status: IdentityStatusBits::default().into(),
                serial_number: 0x01ff3d32,
                product_name: CipShortString::from("ClearLink".to_string()),
            })),
        },
    };

    // Assert equality
    assert_eq!(expected_cip_identity_response, cip_identity_response);
}

#[test]
fn test_deserialize_full_identity_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000006, Send RR Data
    Encapsulation Header
        Command: Send RR Data (0x006f)
        Length: 44
        Session Handle: 0x00000006
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
                Length: 28
        [Request In: 7]
        [Time: 0.000514275 seconds]

    -------------------------------------
    Hex Dump:

    0000   6f 00 2c 00 06 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1c 00

    */

    /*
    Common Industrial Protocol
    Service: Get Attributes All (Response)
        1... .... = Request/Response: Response (0x1)
        .000 0001 = Service: Get Attributes All (0x01)
    Status: Success:
        General Status: Success (0x00)
        Additional Status Size: 0 words
    [Request Path Size: 4 words]
    [Request Path: Identity, Instance: 0x0001]
        [Path Segment: 0x21 (16-Bit Class Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 00.. = Logical Segment Type: Class ID (0)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Class: Identity (0x0001)]
        [Path Segment: 0x25 (16-Bit Instance Segment)]
            [001. .... = Path Segment Type: Logical Segment (1)]
            [...0 01.. = Logical Segment Type: Instance ID (1)]
            [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
            [Instance: 0x0001]
    Get Attributes All (Response)
        Attribute: 1 (Vendor ID)
            Vendor ID: Teknic, Inc. (0x01a8)
        Attribute: 2 (Device Type)
            Device Type: Generic Device (keyable) (0x002b)
        Attribute: 3 (Product Code)
            Product Code: 1
        Attribute: 4 (Revision)
            Major Revision: 2
            Minor Revision: 93
        Attribute: 5 (Status)
            Status: 0x0000
                .... .... .... ...0 = Owned: 0
                .... .... .... .0.. = Configured: 0
                .... .... 0000 .... = Extended Device Status: 0x0
                .... ...0 .... .... = Minor Recoverable Fault: 0
                .... ..0. .... .... = Minor Unrecoverable Fault: 0
                .... .0.. .... .... = Major Recoverable Fault: 0
                .... 0... .... .... = Major Unrecoverable Fault: 0
                0000 .... .... .... = Extended Device Status 2: 0x0
        Attribute: 6 (Serial Number)
            Serial Number: 0x01ff3d32
        Attribute: 7 (Product Name)
            Product Name: ClearLink

    -------------------------------------
    Hex Dump:

    0000   81 00 00 00 a8 01 2b 00 01 00 02 5d 00 00 32 3d
    0010   ff 01 09 43 6c 65 61 72 4c 69 6e 6b

    */

    let identity_response_bytes: Vec<u8> = vec![
        0x6f, 0x00, 0x2c, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1c, 0x00, 0x81, 0x00, 0x00, 0x00, 0xa8,
        0x01, 0x2b, 0x00, 0x01, 0x00, 0x02, 0x5d, 0x00, 0x00, 0x32, 0x3d, 0xff, 0x01, 0x09, 0x43,
        0x6c, 0x65, 0x61, 0x72, 0x4c, 0x69, 0x6e, 0x6b,
    ];

    let byte_cursor = std::io::Cursor::new(identity_response_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);

    let identity_response = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    let expected_identity_response = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(44),
            session_handle: IDENTITY_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData {
            interface_handle: CIP_INTERFACE_HANDLE,
            timeout: NO_ENCAPSULATION_TIMEOUT,
            null_address_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::NullAddr,
                packet_length: Some(0),
            },
            unconnected_data_item: CommonPacketDescriptor {
                type_id: CommonPacketItemId::UnconnectedMessage,
                packet_length: Some(28),
            },
            cip_message: MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::GetAttributeAll),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0x0,
                    additional_status: vec![],
                    data: CipDataOpt::Typed(Box::new(IdentityResponse {
                        vendor_id: VendorId::TeknicInc,
                        device_type: DeviceType::GenericDevice,
                        product_code: 0x1,
                        revision: Revision {
                            major: 2,
                            minor: 93,
                        },
                        status: IdentityStatusBits::default().into(),
                        serial_number: 0x01ff3d32,
                        product_name: CipShortString::from("ClearLink".to_string()),
                    })),
                },
            }
            .into(),
            sockaddr_info_items: SockaddrInfoItems::default(),
        }),
    };

    // Assert equality
    assert_eq!(expected_identity_response, identity_response);
}
