mod common;

use binrw::{BinRead, BinWrite};

use bilge::prelude::u4;

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::PriorityTimeTick;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::{MessageRouterRequest, RequestData};
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::object_ids::{CONNECTION_MANAGER_CLASS_ID, CONNECTION_MANAGER_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

use common::{
    CLEARLINK_IO_SESSION_HANDLE, CONNECTION_SERIAL_NUMBER, FORWARD_OPEN_CONFIG_INSTANCE,
    FORWARD_OPEN_O2T_CONNECTION_POINT, FORWARD_OPEN_T2O_CONNECTION_POINT, ORIGINATOR_SERIAL_NUMBER,
    ORIGINATOR_VENDOR_ID, TICK_TIME, TIMEOUT_TICKS,
};

/// Connection serial number and originator identity of the captures
fn sample_connection_triad() -> ConnectionTriad {
    ConnectionTriad {
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
    }
}

/// Closes the connection of the Forward_Open tests: the same triad and connection path,
/// tick time 10 (1024 ms per tick), normal priority
fn sample_request() -> ForwardCloseRequest {
    ForwardCloseRequest {
        priority_time_tick: PriorityTimeTick::builder()
            .tick_time(u4::new(TICK_TIME))
            .priority(false)
            .build(),
        timeout_ticks: TIMEOUT_TICKS,
        connection_triad: sample_connection_triad(),
        connection_path: CipPath::new_assembly_connection(
            FORWARD_OPEN_CONFIG_INSTANCE,
            FORWARD_OPEN_O2T_CONNECTION_POINT,
            FORWARD_OPEN_T2O_CONNECTION_POINT,
        ),
    }
}

/// The request data of a packet read from the wire, parsed as a Forward_Close request (what an
/// adapter does with the packet)
fn forward_close_request_of(packet: &RequestObjectAssembly) -> ForwardCloseRequest {
    let Some(CipMessage::Request(message)) = packet
        .command_specific_data
        .as_send_rr_data()
        .map(|rr_data| &rr_data.cip_message)
    else {
        panic!(
            "expected a Message Router request, got {:?}",
            packet.command_specific_data
        );
    };
    // A packet read from the wire keeps its request data raw
    let CipDataOpt::Raw(data) = &message.request_data.additional_data else {
        panic!(
            "expected raw request data, got {:?}",
            message.request_data.additional_data
        );
    };
    ForwardCloseRequest::read(&mut std::io::Cursor::new(data)).unwrap()
}

#[test]
fn test_serialize_forward_close_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 42
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
                    Length: 26
    Common Industrial Protocol
        Service: Unknown Service (0x4e) (Request)
            0... .... = Request/Response: Request (0x0)
            .100 1110 = Service: Unknown (0x4e)
        Request Path Size: 2 words
        Request Path: Connection Manager, Instance: 0x01
            Path Segment: 0x20 (8-Bit Class Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 00.. = Logical Segment Type: Class ID (0)
                .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                Class: Connection Manager (0x06)
            Path Segment: 0x24 (8-Bit Instance Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 01.. = Logical Segment Type: Instance ID (1)
                .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                Instance: 0x01
    CIP Connection Manager
        Service: Forward Close (Request)
            0... .... = Request/Response: Request (0x0)
            .100 1110 = Service: Forward Close (0x4e)
        Command Specific Data
            ...0 .... = Priority: 0
            .... 1010 = Tick time: 10
            Time-out ticks: 5
            Actual Time Out: 5120ms
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            Connection Path Size: 4 words
            Reserved: 0x00
            Connection Path: Assembly, Instance: 0x97, Connection Point: 0x96, Connection Point: 0x64
                Path Segment: 0x20 (8-Bit Class Segment)
                    001. .... = Path Segment Type: Logical Segment (1)
                    ...0 00.. = Logical Segment Type: Class ID (0)
                    .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                    Class: Assembly (0x04)
                Path Segment: 0x24 (8-Bit Instance Segment)
                    001. .... = Path Segment Type: Logical Segment (1)
                    ...0 01.. = Logical Segment Type: Instance ID (1)
                    .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                    Instance: 0x97
                Path Segment: 0x2c (8-Bit Connection Point Segment)
                    001. .... = Path Segment Type: Logical Segment (1)
                    ...0 11.. = Logical Segment Type: Connection Point (3)
                    .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                    Connection Point: 0x96
                Path Segment: 0x2c (8-Bit Connection Point Segment)
                    001. .... = Path Segment Type: Logical Segment (1)
                    ...0 11.. = Logical Segment Type: Connection Point (3)
                    .... ..00 = Logical Segment Format: 8-bit Logical Segment (0)
                    Connection Point: 0x64

    Hex Dump:
    0000   6f 00 2a 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1a 00 4e 02 20 06 24 01 0a 05
    0030   01 00 56 01 45 23 01 00 04 00 20 04 24 97 2c 96
    0040   2c 64
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x2a, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1a, 0x00, 0x4e, 0x02, 0x20, 0x06, 0x24,
        0x01, 0x0a, 0x05, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x04, 0x00, 0x20, 0x04,
        0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
    ];

    let request = sample_request();

    let expected_request_object = RequestObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(42),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterRequest {
                service_container: ServiceContainer::new_request(ServiceCode::ForwardClose),
                request_data: RequestData {
                    total_word_size: 2,
                    cip_path: CipPath::new_u8(
                        CONNECTION_MANAGER_CLASS_ID,
                        CONNECTION_MANAGER_INSTANCE_ID,
                    ),
                    additional_data: CipDataOpt::Typed(Box::new(request.clone())),
                },
            },
        )),
    };

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    expected_request_object.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    // The constructor builds the same packet, computing the lengths while writing
    let request_object =
        RequestObjectAssembly::new_forward_close(CLEARLINK_IO_SESSION_HANDLE, request.clone());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request_object.write(&mut writer).unwrap();

    assert_eq!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = RequestObjectAssembly::read(&mut buf_reader).unwrap();

    assert_eq!(forward_close_request_of(&deserialized), request);
}

#[test]
fn test_deserialize_forward_close_success_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 30
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
                    Length: 14
            [Request In: 1]
            [Time: 0.000001000 seconds]
    Common Industrial Protocol
        Service: Unknown Service (0x4e) (Response)
            1... .... = Request/Response: Response (0x1)
            .100 1110 = Service: Unknown (0x4e)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words
        [Request Path Size: 4 words]
        [Request Path: Connection Manager, Instance: 0x0001]
            [Path Segment: 0x21 (16-Bit Class Segment)]
                [001. .... = Path Segment Type: Logical Segment (1)]
                [...0 00.. = Logical Segment Type: Class ID (0)]
                [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
                [Class: Connection Manager (0x0006)]
            [Path Segment: 0x25 (16-Bit Instance Segment)]
                [001. .... = Path Segment Type: Logical Segment (1)]
                [...0 01.. = Logical Segment Type: Instance ID (1)]
                [.... ..01 = Logical Segment Format: 16-bit Logical Segment (1)]
                [Instance: 0x0001]
    CIP Connection Manager
        Service: Forward Close (Response)
            1... .... = Request/Response: Response (0x1)
            .100 1110 = Service: Forward Close (0x4e)
        Command Specific Data
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            Application Reply Size: 0 words
            Reserved: 0x00

    Hex Dump:
    0000   6f 00 1e 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 0e 00 ce 00 00 00 01 00 56 01
    0030   45 23 01 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x1e, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x0e, 0x00, 0xce, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
    ];

    let expected_response = ForwardCloseResponse {
        connection_triad: sample_connection_triad(),
        application_reply_size: 0,
        application_reply: vec![],
    };

    let expected_response_object = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(30),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::ForwardClose),
                response_data: ResponseData {
                    status: ResponseStatusCode::Success,
                    additional_status_size: 0,
                    additional_status: vec![],
                    data: CipDataOpt::Typed(Box::new(expected_response.clone())),
                },
            },
        )),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response_object = ResponseObjectAssembly::read(&mut buf_reader).unwrap();

    // The reply data as the typed reply
    let CipDataOpt::Raw(reply_data) = &response_object.response().unwrap().response_data.data
    else {
        panic!("a reply read from the wire holds its data raw");
    };
    assert_eq!(
        ForwardCloseResponse::read_le(&mut std::io::Cursor::new(reply_data)).unwrap(),
        expected_response
    );

    // Writing the typed reply (what an adapter does) reproduces the packet, reserved byte included
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    expected_response_object.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}
