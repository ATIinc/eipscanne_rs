mod common;

use binrw::{BinRead, BinWrite};

use bilge::prelude::{Integer, u4, u9};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ConnectionManagerExtendedStatus, ConnectionParameters, ForwardOpenError, ForwardOpenFailure,
    ForwardOpenRequest, ForwardOpenResponse, ForwardOpenUnsuccessfulResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionDirection, ConnectionPriority, ConnectionSizeError, ConnectionSizeType,
    ConnectionTimeoutMultiplier, ConnectionType, Direction, LargeNetworkConnectionParameters,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RealTimeFormat,
    RedundantOwner, StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipByte;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use common::{
    CLEARLINK_IO_SESSION_HANDLE, CONNECTION_SERIAL_NUMBER, FORWARD_OPEN_CONFIG_INSTANCE,
    FORWARD_OPEN_O2T_CONNECTION_POINT, FORWARD_OPEN_T2O_CONNECTION_POINT, IO_DATA_SIZE,
    O2T_CONNECTION_SIZE, O2T_NETWORK_CONNECTION_ID, ORIGINATOR_SERIAL_NUMBER, ORIGINATOR_VENDOR_ID,
    REQUESTED_O2T_NETWORK_CONNECTION_ID, RPI_MICROSECONDS, T2O_CONNECTION_SIZE,
    T2O_NETWORK_CONNECTION_ID, TICK_TIME, TIMEOUT_TICKS,
};

/// Tick time 10 (1024 ms per tick), normal priority
fn sample_priority_time_tick() -> PriorityTimeTick {
    let mut priority_time_tick = PriorityTimeTick::default();
    priority_time_tick.set_tick_time(u4::new(TICK_TIME));
    priority_time_tick.set_priority(false);
    priority_time_tick
}

/// Class 1, cyclic, client
fn sample_transport_type_trigger() -> TransportTypeTrigger {
    let mut transport_type_trigger = TransportTypeTrigger::default();
    transport_type_trigger.set_transport_class(TransportClass::Class1);
    transport_type_trigger.set_production_trigger(ProductionTrigger::Cyclic);
    transport_type_trigger.set_direction(Direction::Client);
    transport_type_trigger
}

/// The parameters of EIPScanner's implicit messaging example: 32-byte assemblies both ways,
/// a 1 s packet interval, class 1 cyclic, point-to-point with scheduled priority
fn sample_parameters(large: bool) -> ConnectionParameters {
    ConnectionParameters {
        priority_time_tick: sample_priority_time_tick(),
        timeout_ticks: TIMEOUT_TICKS,
        o2t_network_connection_id: REQUESTED_O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        o2t_rpi: RPI_MICROSECONDS,
        t2o_rpi: RPI_MICROSECONDS,
        o2t: ConnectionDirection {
            connection_type: ConnectionType::PointToPoint,
            priority: ConnectionPriority::Scheduled,
            connection_size_type: ConnectionSizeType::Fixed,
            redundant_owner: RedundantOwner::Exclusive,
            data_size: IO_DATA_SIZE,
            real_time_format: RealTimeFormat::Header32Bit,
        },
        t2o: ConnectionDirection {
            connection_type: ConnectionType::PointToPoint,
            priority: ConnectionPriority::Scheduled,
            connection_size_type: ConnectionSizeType::Fixed,
            redundant_owner: RedundantOwner::Exclusive,
            data_size: IO_DATA_SIZE,
            real_time_format: RealTimeFormat::Modeless,
        },
        transport_type_trigger: sample_transport_type_trigger(),
        connection_path: CipPath::new_assembly_connection(
            FORWARD_OPEN_CONFIG_INSTANCE,
            FORWARD_OPEN_O2T_CONNECTION_POINT,
            FORWARD_OPEN_T2O_CONNECTION_POINT,
        ),
        large,
    }
}

/// Point-to-point, scheduled priority, fixed size, exclusive owner
fn sample_standard_parameters(connection_size: u16) -> StandardNetworkConnectionParameters {
    let mut parameters = StandardNetworkConnectionParameters::default();
    parameters.set_connection_size(u9::new(connection_size));
    parameters.set_connection_size_type(ConnectionSizeType::Fixed);
    parameters.set_priority(ConnectionPriority::Scheduled);
    parameters.set_connection_type(ConnectionType::PointToPoint);
    parameters.set_redundant_owner(RedundantOwner::Exclusive);
    parameters
}

/// Point-to-point, scheduled priority, fixed size, exclusive owner
fn sample_large_parameters(connection_size: u16) -> LargeNetworkConnectionParameters {
    let mut parameters = LargeNetworkConnectionParameters::default();
    parameters.set_connection_size(connection_size);
    parameters.set_connection_size_type(ConnectionSizeType::Fixed);
    parameters.set_priority(ConnectionPriority::Scheduled);
    parameters.set_connection_type(ConnectionType::PointToPoint);
    parameters.set_redundant_owner(RedundantOwner::Exclusive);
    parameters
}

fn sample_request(
    o2t_network_connection_parameters: NetworkConnectionParameters,
    t2o_network_connection_parameters: NetworkConnectionParameters,
) -> ForwardOpenRequest {
    ForwardOpenRequest {
        priority_time_tick: sample_priority_time_tick(),
        timeout_ticks: TIMEOUT_TICKS,
        o2t_network_connection_id: REQUESTED_O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        o2t_rpi: RPI_MICROSECONDS,
        o2t_network_connection_parameters,
        t2o_rpi: RPI_MICROSECONDS,
        t2o_network_connection_parameters,
        transport_type_trigger: sample_transport_type_trigger(),
        connection_path: CipPath::new_assembly_connection(
            FORWARD_OPEN_CONFIG_INSTANCE,
            FORWARD_OPEN_O2T_CONNECTION_POINT,
            FORWARD_OPEN_T2O_CONNECTION_POINT,
        ),
    }
}

/*
Forward Open (Request)
    .... 1010 = Tick time: 10
    ...0 .... = Priority: 0
    Time-out ticks: 5
    Actual Time Out: 5120ms
    O->T Network Connection ID: 0x00000000
    T->O Network Connection ID: 0x12345678
    Connection Serial Number: 0x0001
    Originator Vendor ID: 0x0156
    Originator Serial Number: 0x00012345
    Connection Timeout Multiplier: *4 (0)
    Reserved: 0x000000
    O->T RPI: 1.000s
    O->T Network Connection Parameters: 0x4826
        0... .... .... .... = Redundant Owner: Exclusive (0)
        .10. .... .... .... = Connection Type: Point to Point (2)
        .... 10.. .... .... = Priority: Scheduled (2)
        .... ..0. .... .... = Connection Size Type: Fixed (0)
        .... ...0 0010 0110 = Connection Size: 38 bytes
    T->O RPI: 1.000s
    T->O Network Connection Parameters: 0x4822
        0... .... .... .... = Redundant Owner: Exclusive (0)
        .10. .... .... .... = Connection Type: Point to Point (2)
        .... 10.. .... .... = Priority: Scheduled (2)
        .... ..0. .... .... = Connection Size Type: Fixed (0)
        .... ...0 0010 0010 = Connection Size: 34 bytes
    Transport Type/Trigger: 0x01
        0... .... = Direction: Client (0)
        .000 .... = Trigger: Cyclic (0)
        .... 0001 = Class: Class 1 (1)
    Connection Path Size: 4 words
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
0000   0a 05 00 00 00 00 78 56 34 12 01 00 56 01 45 23
0010   01 00 00 00 00 00 40 42 0f 00 26 48 40 42 0f 00
0020   22 48 01 04 20 04 24 97 2c 96 2c 64
*/
const FORWARD_OPEN_REQUEST_BYTES: [CipByte; 44] = [
    0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x26, 0x48, 0x40, 0x42, 0x0f, 0x00,
    0x22, 0x48, 0x01, 0x04, 0x20, 0x04, 0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
];

#[test]
fn test_serialize_forward_open_request() {
    let expected_byte_array: Vec<CipByte> = FORWARD_OPEN_REQUEST_BYTES.to_vec();

    let request = ForwardOpenRequest::new(&sample_parameters(false)).unwrap();

    assert_eq!(request.service_code(), ServiceCode::ForwardOpen);
    assert_eq!(
        request,
        sample_request(
            NetworkConnectionParameters::Standard(sample_standard_parameters(O2T_CONNECTION_SIZE)),
            NetworkConnectionParameters::Standard(sample_standard_parameters(T2O_CONNECTION_SIZE)),
        )
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = ForwardOpenRequest::read_le_args(&mut buf_reader, (false,)).unwrap();

    assert_eq!(deserialized, request);
}

#[test]
fn test_serialize_large_forward_open_request() {
    /*
    Large Forward Open (Request)
        ... as the Forward Open request, with 32-bit Network Connection Parameters:
        O->T Network Connection Parameters: 0x48000026
            0... .... .... .... .... .... .... .... = Redundant Owner: Exclusive (0)
            .10. .... .... .... .... .... .... .... = Connection Type: Point to Point (2)
            .... 10.. .... .... .... .... .... .... = Priority: Scheduled (2)
            .... ..0. .... .... .... .... .... .... = Connection Size Type: Fixed (0)
            .... .... .... .... 0000 0000 0010 0110 = Connection Size: 38 bytes
        T->O Network Connection Parameters: 0x48000022
            .... .... .... .... 0000 0000 0010 0010 = Connection Size: 34 bytes

    Hex Dump:
    0000   0a 05 00 00 00 00 78 56 34 12 01 00 56 01 45 23
    0010   01 00 00 00 00 00 40 42 0f 00 26 00 00 48 40 42
    0020   0f 00 22 00 00 48 01 04 20 04 24 97 2c 96 2c 64
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45,
        0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x26, 0x00, 0x00, 0x48,
        0x40, 0x42, 0x0f, 0x00, 0x22, 0x00, 0x00, 0x48, 0x01, 0x04, 0x20, 0x04, 0x24, 0x97, 0x2c,
        0x96, 0x2c, 0x64,
    ];

    let request = ForwardOpenRequest::new(&sample_parameters(true)).unwrap();

    assert_eq!(request.service_code(), ServiceCode::LargeForwardOpen);
    assert_eq!(
        request,
        sample_request(
            NetworkConnectionParameters::Large(sample_large_parameters(O2T_CONNECTION_SIZE)),
            NetworkConnectionParameters::Large(sample_large_parameters(T2O_CONNECTION_SIZE)),
        )
    );

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = ForwardOpenRequest::read_le_args(&mut buf_reader, (true,)).unwrap();

    assert_eq!(deserialized, request);
}

#[test]
fn test_serialize_encapsulated_forward_open_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 70
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
                    Length: 54
    Common Industrial Protocol
        Service: Forward Open (Request)
            0... .... = Request/Response: Request (0x0)
            .101 0100 = Service: Forward Open (0x54)
        Request Path Size: 4 words
        Request Path: Connection Manager, Instance: 0x0001
            Path Segment: 0x21 (16-Bit Class Segment)
                Class: Connection Manager (0x0006)
            Path Segment: 0x25 (16-Bit Instance Segment)
                Instance: 0x0001
        Forward Open (Request)
            ... as in test_serialize_forward_open_request

    Hex Dump:
    0000   6f 00 46 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 36 00 54 04 21 00 06 00 25 00
    0030   01 00 0a 05 00 00 00 00 78 56 34 12 01 00 56 01
    0040   45 23 01 00 00 00 00 00 40 42 0f 00 26 48 40 42
    0050   0f 00 22 48 01 04 20 04 24 97 2c 96 2c 64
    */
    let encapsulation_header: [CipByte; 24] = [
        0x6f, 0x00, 0x46, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let command_specific_data: [CipByte; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x36,
        0x00,
    ];
    let message_router_request: [CipByte; 10] =
        [0x54, 0x04, 0x21, 0x00, 0x06, 0x00, 0x25, 0x00, 0x01, 0x00];

    let expected_byte_array: Vec<CipByte> = [
        &encapsulation_header[..],
        &command_specific_data[..],
        &message_router_request[..],
        &FORWARD_OPEN_REQUEST_BYTES[..],
    ]
    .concat();

    let request = ForwardOpenRequest::new(&sample_parameters(false)).unwrap();
    let request_object =
        RequestObjectAssembly::new_forward_open(CLEARLINK_IO_SESSION_HANDLE, request);

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request_object.write(&mut writer).unwrap();

    assert_eq_hex!(expected_byte_array, byte_array_buffer);
}

#[test]
fn test_deserialize_forward_open_success_response() {
    /*
    Common Industrial Protocol
        Service: Forward Open (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Forward Open (0x54)
        Status: Success:
            General Status: Success (0x00)
            Additional Status Size: 0 words
        Forward Open (Response)
            O->T Network Connection ID: 0xa1b2c3d4
            T->O Network Connection ID: 0x12345678
            Connection Serial Number: 0x0001
            Originator Vendor ID: 0x0156
            Originator Serial Number: 0x00012345
            O->T API: 1.000s
            T->O API: 1.000s
            Application Reply Size: 0 words
            Reserved: 0x00

    Hex Dump:
    0000   d4 00 00 00 d4 c3 b2 a1 78 56 34 12 01 00 56 01
    0010   45 23 01 00 40 42 0f 00 40 42 0f 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0xd4, 0x00, 0x00, 0x00, 0xd4, 0xc3, 0xb2, 0xa1, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56,
        0x01, 0x45, 0x23, 0x01, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x00, 0x00,
    ];

    let expected_response = ForwardOpenResponse {
        o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
        o2t_api: RPI_MICROSECONDS,
        t2o_api: RPI_MICROSECONDS,
        application_reply_size: 0,
        application_reply: vec![],
    };

    // The reply data on its own
    let byte_cursor = std::io::Cursor::new(raw_bytes[4..].to_vec());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response = ForwardOpenResponse::read(&mut buf_reader).unwrap();

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
        ForwardOpenResponse::from_message_router_response(&message_router_response).unwrap(),
        expected_response
    );
}

#[test]
fn test_deserialize_forward_open_rejected_response() {
    /*
    Common Industrial Protocol
        Service: Forward Open (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Forward Open (0x54)
        Status: Connection failure:
            General Status: Connection failure (0x01)
            Additional Status Size: 1 word
            Additional Status: 0x0100
            Extended Status: Connection in use or duplicate Forward Open (0x0100)
        Forward Open (Response)
            Connection Serial Number: 0x0001
            Originator Vendor ID: 0x0156
            Originator Serial Number: 0x00012345
            Remaining Path Size: 0 words
            Reserved: 0x00

    Hex Dump:
    0000   d4 00 01 01 00 01 01 00 56 01 45 23 01 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0xd4, 0x00, 0x01, 0x01, 0x00, 0x01, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00,
        0x00,
    ];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let message_router_response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    let error =
        ForwardOpenResponse::from_message_router_response(&message_router_response).unwrap_err();
    let ForwardOpenError::Rejected(failure) = error else {
        panic!("expected a rejected Forward_Open, got {error:?}");
    };

    assert_eq!(
        failure,
        ForwardOpenFailure {
            general_status: ResponseStatusCode::ConnectionFailure,
            extended_status: Some(
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
            ),
            additional_status: vec![0x0100],
            response: Some(ForwardOpenUnsuccessfulResponse {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
                remaining_path_size: Some((0, 0)),
            }),
        }
    );

    // The same reply without the Remaining Path Size and its reserved byte
    let short_bytes: Vec<CipByte> = raw_bytes[..14].to_vec();

    let byte_cursor = std::io::Cursor::new(short_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let message_router_response =
        MessageRouterResponse::read_args(&mut buf_reader, (short_bytes.len() as u16,)).unwrap();

    let error =
        ForwardOpenResponse::from_message_router_response(&message_router_response).unwrap_err();
    let ForwardOpenError::Rejected(failure) = error else {
        panic!("expected a rejected Forward_Open, got {error:?}");
    };

    assert_eq!(
        failure.response,
        Some(ForwardOpenUnsuccessfulResponse {
            connection_serial_number: CONNECTION_SERIAL_NUMBER,
            originator_vendor_id: ORIGINATOR_VENDOR_ID,
            originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
            remaining_path_size: None,
        })
    );
}

#[test]
fn test_forward_open_reply_to_another_service_is_malformed() {
    // A Get Attribute Single reply (service 0x8e) with a success status
    let raw_bytes: Vec<CipByte> = vec![0x8e, 0x00, 0x00, 0x00];

    let byte_cursor = std::io::Cursor::new(raw_bytes.clone());
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let message_router_response =
        MessageRouterResponse::read_args(&mut buf_reader, (raw_bytes.len() as u16,)).unwrap();

    let error =
        ForwardOpenResponse::from_message_router_response(&message_router_response).unwrap_err();

    assert!(matches!(error, ForwardOpenError::Malformed(_)));
}

#[test]
fn test_forward_open_request_rejects_connection_size_over_nine_bits() {
    const OVERSIZED_DATA_SIZE: u16 = 600;
    const OVERSIZED_CONNECTION_SIZE: u16 = OVERSIZED_DATA_SIZE + 6;

    let mut parameters = sample_parameters(false);
    parameters.o2t.data_size = OVERSIZED_DATA_SIZE;

    // 600 bytes of data, 2 bytes of sequence count and the 4-byte header do not fit 9 bits
    assert_eq!(
        ForwardOpenRequest::new(&parameters),
        Err(ConnectionSizeError {
            connection_size: u32::from(OVERSIZED_CONNECTION_SIZE),
            maximum: u32::from(u9::MAX.value()),
        })
    );

    // ... but a Large_Forward_Open carries them
    parameters.large = true;
    let request = ForwardOpenRequest::new(&parameters).unwrap();

    assert_eq!(
        request.o2t_network_connection_parameters,
        NetworkConnectionParameters::Large(sample_large_parameters(OVERSIZED_CONNECTION_SIZE))
    );
}

#[test]
fn test_network_connection_parameters_layout() {
    assert_eq!(
        u16::from(sample_standard_parameters(O2T_CONNECTION_SIZE)),
        0x4826
    );
    assert_eq!(
        u16::from(sample_standard_parameters(T2O_CONNECTION_SIZE)),
        0x4822
    );
    assert_eq!(
        u32::from(sample_large_parameters(O2T_CONNECTION_SIZE)),
        0x48000026
    );

    let mut variable_multicast = StandardNetworkConnectionParameters::default();
    variable_multicast.set_connection_size(u9::new(0x1ff));
    variable_multicast.set_connection_size_type(ConnectionSizeType::Variable);
    variable_multicast.set_priority(ConnectionPriority::Urgent);
    variable_multicast.set_connection_type(ConnectionType::Multicast);
    variable_multicast.set_redundant_owner(RedundantOwner::Redundant);
    assert_eq!(u16::from(variable_multicast), 0xafff);

    let parsed = StandardNetworkConnectionParameters::from(0x4826u16);
    assert_eq!(parsed.connection_size().value(), O2T_CONNECTION_SIZE);
    assert_eq!(parsed.connection_size_type(), ConnectionSizeType::Fixed);
    assert_eq!(parsed.priority(), ConnectionPriority::Scheduled);
    assert_eq!(parsed.connection_type(), ConnectionType::PointToPoint);
    assert_eq!(parsed.redundant_owner(), RedundantOwner::Exclusive);

    let parsed = LargeNetworkConnectionParameters::from(0x48000026u32);
    assert_eq!(parsed.connection_size(), O2T_CONNECTION_SIZE);
    assert_eq!(parsed.priority(), ConnectionPriority::Scheduled);
    assert_eq!(parsed.connection_type(), ConnectionType::PointToPoint);
}

#[test]
fn test_transport_type_trigger_layout() {
    assert_eq!(u8::from(sample_transport_type_trigger()), 0x01);

    let mut class3_application_server = TransportTypeTrigger::default();
    class3_application_server.set_transport_class(TransportClass::Class3);
    class3_application_server.set_production_trigger(ProductionTrigger::ApplicationObject);
    class3_application_server.set_direction(Direction::Server);
    assert_eq!(u8::from(class3_application_server), 0xa3);

    let parsed = TransportTypeTrigger::from(0xa3u8);
    assert_eq!(parsed.transport_class(), TransportClass::Class3);
    assert_eq!(
        parsed.production_trigger(),
        ProductionTrigger::ApplicationObject
    );
    assert_eq!(parsed.direction(), Direction::Server);
}

#[test]
fn test_priority_time_tick_layout() {
    assert_eq!(u8::from(sample_priority_time_tick()), 0x0a);
    assert_eq!(PriorityTimeTick::from(0x0au8), sample_priority_time_tick());
}

#[test]
fn test_connection_timeout_multiplier() {
    assert_eq!(ConnectionTimeoutMultiplier::X4.multiplier(), 4);
    assert_eq!(ConnectionTimeoutMultiplier::X512.multiplier(), 512);
    assert_eq!(ConnectionTimeoutMultiplier::Unknown(8).multiplier(), 1024);

    let raw_bytes: Vec<CipByte> = vec![0x07];
    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    assert_eq!(
        ConnectionTimeoutMultiplier::read(&mut buf_reader).unwrap(),
        ConnectionTimeoutMultiplier::X512
    );
}

#[test]
fn test_connection_manager_extended_status_codes() {
    assert_eq!(
        ConnectionManagerExtendedStatus::from(0x0107),
        ConnectionManagerExtendedStatus::TargetConnectionNotFound
    );
    assert_eq!(
        ConnectionManagerExtendedStatus::from(0x0814),
        ConnectionManagerExtendedStatus::InvalidProduceConsumeDataFormat
    );
    assert_eq!(
        ConnectionManagerExtendedStatus::from(0x0101),
        ConnectionManagerExtendedStatus::Unknown(0x0101)
    );

    for code in [0x0100, 0x011b, 0x0207, 0x031f, 0x0800, 0x0813, 0x1234] {
        assert_eq!(ConnectionManagerExtendedStatus::from(code).code(), code);
    }
}
