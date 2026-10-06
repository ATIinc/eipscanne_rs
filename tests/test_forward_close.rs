mod common;

use binrw::{BinRead, BinWrite};

use bilge::prelude::u4;

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseError, ForwardCloseFailure, ForwardCloseRequest, ForwardCloseResponse,
    ForwardCloseUnsuccessfulResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::parameters::PriorityTimeTick;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use common::{
    CLEARLINK_IO_SESSION_HANDLE, CONNECTION_SERIAL_NUMBER, FORWARD_OPEN_CONFIG_INSTANCE,
    FORWARD_OPEN_O2T_CONNECTION_POINT, FORWARD_OPEN_T2O_CONNECTION_POINT, ORIGINATOR_SERIAL_NUMBER,
    ORIGINATOR_VENDOR_ID, TICK_TIME, TIMEOUT_TICKS,
};

fn sample_request() -> ForwardCloseRequest {
    // Tick time 10 (1024 ms per tick), normal priority
    let mut priority_time_tick = PriorityTimeTick::default();
    priority_time_tick.set_tick_time(u4::new(TICK_TIME));

    ForwardCloseRequest {
        priority_time_tick,
        timeout_ticks: TIMEOUT_TICKS,
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
        connection_path: CipPath::new_assembly_connection(
            FORWARD_OPEN_CONFIG_INSTANCE,
            FORWARD_OPEN_O2T_CONNECTION_POINT,
            FORWARD_OPEN_T2O_CONNECTION_POINT,
        ),
    }
}

/*
Forward Close (Request)
    .... 1010 = Tick time: 10
    ...0 .... = Priority: 0
    Time-out ticks: 5
    Actual Time Out: 5120ms
    Connection Serial Number: 0x0001
    Originator Vendor ID: 0x0156
    Originator Serial Number: 0x00012345
    Connection Path Size: 4 words
    Reserved: 0x00
    Connection Path: Assembly, Instance: 0x97, Connection Point: 0x96, Connection Point: 0x64
        Path Segment: 0x20 (8-Bit Class Segment)
            Class: Assembly (0x04)
        Path Segment: 0x24 (8-Bit Instance Segment)
            Instance: 0x97
        Path Segment: 0x2C (8-Bit Connection Point Segment)
            Connection Point: 0x96
        Path Segment: 0x2C (8-Bit Connection Point Segment)
            Connection Point: 0x64

Hex Dump:
0000   0a 05 01 00 56 01 45 23 01 00 04 00 20 04 24 97
0010   2c 96 2c 64
*/
const FORWARD_CLOSE_REQUEST_BYTES: [CipByte; 20] = [
    0x0a, 0x05, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x04, 0x00, 0x20, 0x04, 0x24, 0x97,
    0x2c, 0x96, 0x2c, 0x64,
];

#[test]
fn test_serialize_forward_close_request() {
    let expected_byte_array: Vec<CipByte> = FORWARD_CLOSE_REQUEST_BYTES.to_vec();

    let request = sample_request();

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = ForwardCloseRequest::read(&mut buf_reader).unwrap();

    assert_eq!(deserialized, request);
}

#[test]
fn test_serialize_encapsulated_forward_close_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 46
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
                    Length: 30
    Common Industrial Protocol
        Service: Forward Close (Request)
            0... .... = Request/Response: Request (0x0)
            .100 1110 = Service: Forward Close (0x4e)
        Request Path Size: 4 words
        Request Path: Connection Manager, Instance: 0x0001
            Path Segment: 0x21 (16-Bit Class Segment)
                Class: Connection Manager (0x0006)
            Path Segment: 0x25 (16-Bit Instance Segment)
                Instance: 0x0001
        Forward Close (Request)
            ... as in test_serialize_forward_close_request

    Hex Dump:
    0000   6f 00 2e 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1e 00 4e 04 21 00 06 00 25 00
    0030   01 00 0a 05 01 00 56 01 45 23 01 00 04 00 20 04
    0040   24 97 2c 96 2c 64
    */
    let encapsulation_header: [CipByte; 24] = [
        0x6f, 0x00, 0x2e, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let command_specific_data: [CipByte; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1e,
        0x00,
    ];
    let message_router_request: [CipByte; 10] =
        [0x4e, 0x04, 0x21, 0x00, 0x06, 0x00, 0x25, 0x00, 0x01, 0x00];

    let expected_byte_array: Vec<CipByte> = [
        &encapsulation_header[..],
        &command_specific_data[..],
        &message_router_request[..],
        &FORWARD_CLOSE_REQUEST_BYTES[..],
    ]
    .concat();

    let request_object =
        RequestObjectAssembly::new_forward_close(CLEARLINK_IO_SESSION_HANDLE, sample_request());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request_object.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);
}

#[test]
fn test_deserialize_forward_close_success_response() {
    /*
    Common Industrial Protocol
        Service: Forward Close (Response)
            1... .... = Request/Response: Response (0x1)
            .100 1110 = Service: Forward Close (0x4e)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words
        Forward Close (Response)
            Connection Serial Number: 0x0001
            Originator Vendor ID: 0x0156
            Originator Serial Number: 0x00012345
            Application Reply Size: 0 words
            Reserved: 0x00

    Hex Dump:
    0000   ce 00 00 00 01 00 56 01 45 23 01 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0xce, 0x00, 0x00, 0x00, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
    ];

    let expected_response = ForwardCloseResponse {
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
        application_reply_size: 0,
        application_reply: vec![],
    };

    // The reply data on its own
    let byte_cursor = std::io::Cursor::new(raw_bytes[4..].to_vec());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response = ForwardCloseResponse::read(&mut buf_reader).unwrap();

    assert_eq!(expected_response, response);

    // Writing it back must reproduce the reply data, including the reserved byte
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    response.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes[4..].to_vec(), byte_array_buffer);

    // The whole Message Router response
    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let message_router_response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    assert_eq!(
        ForwardCloseResponse::from_message_router_response(&message_router_response).unwrap(),
        expected_response
    );
}

#[test]
fn test_deserialize_forward_close_rejected_response() {
    /*
    Common Industrial Protocol
        Service: Forward Close (Response)
            1... .... = Request/Response: Response (0x1)
            .100 1110 = Service: Forward Close (0x4e)
        Status: Connection failure:
            General Status: Connection failure (0x01)
            Additional Status Size: 1 word
            Additional Status: 0x0107
            Extended Status: Target connection not found (0x0107)
        Forward Close (Response)
            Connection Serial Number: 0x0001
            Originator Vendor ID: 0x0156
            Originator Serial Number: 0x00012345

    Hex Dump:
    0000   ce 00 01 01 07 01 01 00 56 01 45 23 01 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0xce, 0x00, 0x01, 0x01, 0x07, 0x01, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let message_router_response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    let error =
        ForwardCloseResponse::from_message_router_response(&message_router_response).unwrap_err();
    let ForwardCloseError::Rejected(failure) = error else {
        panic!("expected a rejected Forward_Close, got {error:?}");
    };

    assert_eq!(
        failure,
        ForwardCloseFailure {
            general_status: ResponseStatusCode::ConnectionFailure,
            extended_status: Some(ConnectionManagerExtendedStatus::TargetConnectionNotFound),
            additional_status: vec![0x0107],
            response: Some(ForwardCloseUnsuccessfulResponse {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
                remaining_path_size: None,
            }),
        }
    );
}
