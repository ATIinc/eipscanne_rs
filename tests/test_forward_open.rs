mod common;

use binrw::{BinRead, BinWrite};

use bilge::prelude::{u4, u9};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::connection_manager::response::{
    ConnectionManagerExtendedStatus, ConnectionManagerResponse,
};
use eipscanne_rs::cip::connection_manager::shared::{ConnectionTriad, UnsuccessfulResponse};
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
    FORWARD_OPEN_O2T_CONNECTION_POINT, FORWARD_OPEN_T2O_CONNECTION_POINT, IO_DATA_SIZE,
    O2T_CONNECTION_SIZE, O2T_NETWORK_CONNECTION_ID, ORIGINATOR_SERIAL_NUMBER, ORIGINATOR_VENDOR_ID,
    REQUESTED_O2T_NETWORK_CONNECTION_ID, REQUESTED_PACKET_INTERVAL_MICROSECONDS,
    T2O_CONNECTION_SIZE, T2O_NETWORK_CONNECTION_ID, TICK_TIME, TIMEOUT_TICKS,
};

/// Tick time 10 (1024 ms per tick), normal priority
fn sample_priority_time_tick() -> PriorityTimeTick {
    PriorityTimeTick::builder()
        .tick_time(u4::new(TICK_TIME))
        .priority(false)
        .build()
}

/// Class 1, cyclic, client
fn sample_transport_type_trigger() -> TransportTypeTrigger {
    TransportTypeTrigger::builder()
        .transport_class(TransportClass::Class1)
        .production_trigger(ProductionTrigger::Cyclic)
        .direction(Direction::Client)
        .build()
}

/// Connection serial number and originator identity of the captures
fn sample_connection_triad() -> ConnectionTriad {
    ConnectionTriad {
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
    }
}

/// Point-to-point, scheduled priority, fixed size, exclusive owner
fn sample_standard_parameters(connection_size: u16) -> StandardNetworkConnectionParameters {
    StandardNetworkConnectionParameters::builder()
        .connection_size(u9::new(connection_size))
        .connection_size_type(ConnectionSizeType::Fixed)
        .priority(ConnectionPriority::Scheduled)
        .connection_type(ConnectionType::PointToPoint)
        .redundant_owner(RedundantOwner::Exclusive)
        .build()
}

/// Point-to-point, scheduled priority, fixed size, exclusive owner
fn sample_large_parameters(connection_size: u16) -> LargeNetworkConnectionParameters {
    LargeNetworkConnectionParameters::builder()
        .connection_size(connection_size)
        .connection_size_type(ConnectionSizeType::Fixed)
        .priority(ConnectionPriority::Scheduled)
        .connection_type(ConnectionType::PointToPoint)
        .redundant_owner(RedundantOwner::Exclusive)
        .build()
}

/// The parameter word of one direction in the width of the service: sized for the 32-byte
/// assemblies of the captures over class 1, with the real-time header of the direction
fn sample_network_connection_parameters(
    large: bool,
    real_time_format: RealTimeFormat,
) -> NetworkConnectionParameters {
    let size = connection_size(IO_DATA_SIZE, TransportClass::Class1, real_time_format);
    if large {
        NetworkConnectionParameters::Large(sample_large_parameters(size))
    } else {
        NetworkConnectionParameters::Standard(sample_standard_parameters(size))
    }
}

/// The request of EIPScanner's implicit messaging example: 32-byte assemblies both ways, a 1 s
/// packet interval, class 1 cyclic, point-to-point with scheduled priority, a 32-bit header on
/// the originator to target data; a Large_Forward_Open when `large`
fn sample_request(large: bool) -> ForwardOpenRequest {
    ForwardOpenRequest {
        priority_time_tick: sample_priority_time_tick(),
        timeout_ticks: TIMEOUT_TICKS,
        o2t_network_connection_id: REQUESTED_O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: sample_connection_triad(),
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        o2t_requested_packet_interval: REQUESTED_PACKET_INTERVAL_MICROSECONDS,
        o2t_network_connection_parameters: sample_network_connection_parameters(
            large,
            RealTimeFormat::Header32Bit,
        ),
        t2o_requested_packet_interval: REQUESTED_PACKET_INTERVAL_MICROSECONDS,
        t2o_network_connection_parameters: sample_network_connection_parameters(
            large,
            RealTimeFormat::Modeless,
        ),
        transport_type_trigger: sample_transport_type_trigger(),
        connection_path: CipPath::new_assembly_connection(
            FORWARD_OPEN_CONFIG_INSTANCE,
            FORWARD_OPEN_O2T_CONNECTION_POINT,
            FORWARD_OPEN_T2O_CONNECTION_POINT,
        ),
    }
}

/// The request data of a packet read from the wire, parsed as a Forward_Open request of the
/// given parameter width (what an adapter does with the packet)
fn forward_open_request_of(packet: &RequestObjectAssembly, large: bool) -> ForwardOpenRequest {
    let Some(CipMessage::Request(message)) = packet.cip_message() else {
        panic!(
            "expected a Message Router request, got {:?}",
            packet.cip_message()
        );
    };
    // A packet read from the wire keeps its request data raw
    let CipDataOpt::Raw(data) = &message.request_data.additional_data else {
        panic!(
            "expected raw request data, got {:?}",
            message.request_data.additional_data
        );
    };
    ForwardOpenRequest::read_le_args(&mut std::io::Cursor::new(data), (large,)).unwrap()
}

#[test]
fn test_serialize_forward_open_request() {
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
        Service: Unknown Service (0x54) (Request)
            0... .... = Request/Response: Request (0x0)
            .101 0100 = Service: Unknown (0x54)
        Request Path Size: 4 words
        Request Path: Connection Manager, Instance: 0x0001
            Path Segment: 0x21 (16-Bit Class Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 00.. = Logical Segment Type: Class ID (0)
                .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
                Class: Connection Manager (0x0006)
            Path Segment: 0x25 (16-Bit Instance Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 01.. = Logical Segment Type: Instance ID (1)
                .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
                Instance: 0x0001
    CIP Connection Manager
        Service: Forward Open (Request)
            0... .... = Request/Response: Request (0x0)
            .101 0100 = Service: Forward Open (0x54)
        Command Specific Data
            ...0 .... = Priority: 0
            .... 1010 = Tick time: 10
            Time-out ticks: 5
            Actual Time Out: 5120ms
            O->T Network Connection ID: 0x00000000
            T->O Network Connection ID: 0x12345678
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            Connection Timeout Multiplier: *4 (0)
            Reserved: 0x000000
            O->T RPI: 1000.000ms
            O->T Network Connection Parameters: 0x4826
                0... .... .... .... = Redundant Owner: Non-Redundant (0)
                .10. .... .... .... = Connection Type: Point to Point (2)
                .... 10.. .... .... = Priority: Scheduled (2)
                .... ..0. .... .... = Connection Size Type: Fixed (0)
                .... ...0 0010 0110 = Connection Size: 38 bytes
            T->O RPI: 1000.000ms
            T->O Network Connection Parameters: 0x4822
                0... .... .... .... = Redundant Owner: Non-Redundant (0)
                .10. .... .... .... = Connection Type: Point to Point (2)
                .... 10.. .... .... = Priority: Scheduled (2)
                .... ..0. .... .... = Connection Size Type: Fixed (0)
                .... ...0 0010 0010 = Connection Size: 34 bytes
            Transport Type/Trigger: 0x01, Trigger: Cyclic, Class: 1
                0... .... = Direction: Client
                .000 .... = Trigger: Cyclic (0)
                .... 0001 = Class: 1 (1)
            Connection Path Size: 4 words
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
    0000   6f 00 46 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 36 00 54 04 21 00 06 00 25 00
    0030   01 00 0a 05 00 00 00 00 78 56 34 12 01 00 56 01
    0040   45 23 01 00 00 00 00 00 40 42 0f 00 26 48 40 42
    0050   0f 00 22 48 01 04 20 04 24 97 2c 96 2c 64
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x46, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x36, 0x00, 0x54, 0x04, 0x21, 0x00, 0x06,
        0x00, 0x25, 0x00, 0x01, 0x00, 0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
        0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42, 0x0f,
        0x00, 0x26, 0x48, 0x40, 0x42, 0x0f, 0x00, 0x22, 0x48, 0x01, 0x04, 0x20, 0x04, 0x24, 0x97,
        0x2c, 0x96, 0x2c, 0x64,
    ];

    let request = sample_request(false);

    assert_eq!(request.service_code(), ServiceCode::ForwardOpen);
    // 32 bytes of data plus the sequence count, and the 32-bit header originator to target only
    assert_eq!(
        request.o2t_network_connection_parameters,
        NetworkConnectionParameters::Standard(sample_standard_parameters(O2T_CONNECTION_SIZE))
    );
    assert_eq!(
        request.t2o_network_connection_parameters,
        NetworkConnectionParameters::Standard(sample_standard_parameters(T2O_CONNECTION_SIZE))
    );

    let expected_request_object = RequestObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(70),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterRequest {
                service_container: ServiceContainer::new_request(ServiceCode::ForwardOpen),
                request_data: RequestData {
                    total_word_size: 4,
                    cip_path: CipPath::new(
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
        RequestObjectAssembly::new_forward_open(CLEARLINK_IO_SESSION_HANDLE, request.clone());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request_object.write(&mut writer).unwrap();

    assert_eq!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = RequestObjectAssembly::read(&mut buf_reader).unwrap();

    assert_eq!(expected_request_object, deserialized);
    assert_eq!(forward_open_request_of(&deserialized, false), request);
}

#[test]
fn test_serialize_large_forward_open_request() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 74
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
                    Length: 58
    Common Industrial Protocol
        Service: Unknown Service (0x5b) (Request)
            0... .... = Request/Response: Request (0x0)
            .101 1011 = Service: Unknown (0x5b)
        Request Path Size: 4 words
        Request Path: Connection Manager, Instance: 0x0001
            Path Segment: 0x21 (16-Bit Class Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 00.. = Logical Segment Type: Class ID (0)
                .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
                Class: Connection Manager (0x0006)
            Path Segment: 0x25 (16-Bit Instance Segment)
                001. .... = Path Segment Type: Logical Segment (1)
                ...0 01.. = Logical Segment Type: Instance ID (1)
                .... ..01 = Logical Segment Format: 16-bit Logical Segment (1)
                Instance: 0x0001
    CIP Connection Manager
        Service: Large Forward Open (Request)
            0... .... = Request/Response: Request (0x0)
            .101 1011 = Service: Large Forward Open (0x5b)
        Command Specific Data
            ...0 .... = Priority: 0
            .... 1010 = Tick time: 10
            Time-out ticks: 5
            Actual Time Out: 5120ms
            O->T Network Connection ID: 0x00000000
            T->O Network Connection ID: 0x12345678
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            Connection Timeout Multiplier: *4 (0)
            Reserved: 0x000000
            O->T RPI: 1000.000ms
            O->T Network Connection Parameters: 0x48000026
                0... .... .... .... .... .... .... .... = Redundant Owner: Non-Redundant (0)
                .10. .... .... .... .... .... .... .... = Connection Type: Point to Point (2)
                .... 10.. .... .... .... .... .... .... = Priority: Scheduled (2)
                .... ..0. .... .... .... .... .... .... = Connection Size Type: Fixed (0)
                .... .... .... .... 0000 0000 0010 0110 = Connection Size: 38 bytes
            T->O RPI: 1000.000ms
            T->O Network Connection Parameters: 0x48000022
                0... .... .... .... .... .... .... .... = Redundant Owner: Non-Redundant (0)
                .10. .... .... .... .... .... .... .... = Connection Type: Point to Point (2)
                .... 10.. .... .... .... .... .... .... = Priority: Scheduled (2)
                .... ..0. .... .... .... .... .... .... = Connection Size Type: Fixed (0)
                .... .... .... .... 0000 0000 0010 0010 = Connection Size: 34 bytes
            Transport Type/Trigger: 0x01, Trigger: Cyclic, Class: 1
                0... .... = Direction: Client
                .000 .... = Trigger: Cyclic (0)
                .... 0001 = Class: 1 (1)
            Connection Path Size: 4 words
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
    0000   6f 00 4a 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 3a 00 5b 04 21 00 06 00 25 00
    0030   01 00 0a 05 00 00 00 00 78 56 34 12 01 00 56 01
    0040   45 23 01 00 00 00 00 00 40 42 0f 00 26 00 00 48
    0050   40 42 0f 00 22 00 00 48 01 04 20 04 24 97 2c 96
    0060   2c 64
    */
    let expected_byte_array: Vec<CipByte> = vec![
        0x6f, 0x00, 0x4a, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x3a, 0x00, 0x5b, 0x04, 0x21, 0x00, 0x06,
        0x00, 0x25, 0x00, 0x01, 0x00, 0x0a, 0x05, 0x00, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
        0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x42, 0x0f,
        0x00, 0x26, 0x00, 0x00, 0x48, 0x40, 0x42, 0x0f, 0x00, 0x22, 0x00, 0x00, 0x48, 0x01, 0x04,
        0x20, 0x04, 0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
    ];

    let request = sample_request(true);

    assert_eq!(request.service_code(), ServiceCode::LargeForwardOpen);
    assert_eq!(
        request.o2t_network_connection_parameters,
        NetworkConnectionParameters::Large(sample_large_parameters(O2T_CONNECTION_SIZE))
    );
    assert_eq!(
        request.t2o_network_connection_parameters,
        NetworkConnectionParameters::Large(sample_large_parameters(T2O_CONNECTION_SIZE))
    );

    let expected_request_object = RequestObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(74),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterRequest {
                service_container: ServiceContainer::new_request(ServiceCode::LargeForwardOpen),
                request_data: RequestData {
                    total_word_size: 4,
                    cip_path: CipPath::new(
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

    // The constructor picks the Large_Forward_Open service from the parameter width
    let request_object =
        RequestObjectAssembly::new_forward_open(CLEARLINK_IO_SESSION_HANDLE, request.clone());

    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    request_object.write(&mut writer).unwrap();

    assert_eq!(expected_byte_array, byte_array_buffer);

    let byte_cursor = std::io::Cursor::new(expected_byte_array);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let deserialized = RequestObjectAssembly::read(&mut buf_reader).unwrap();

    assert_eq!(expected_request_object, deserialized);
    assert_eq!(forward_open_request_of(&deserialized, true), request);
}

#[test]
fn test_deserialize_forward_open_success_response() {
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
            [Request In: 1]
            [Time: 0.000001000 seconds]
    Common Industrial Protocol
        Service: Unknown Service (0x54) (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Unknown (0x54)
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
        Service: Forward Open (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Forward Open (0x54)
        Command Specific Data
            O->T Network Connection ID: 0xa1b2c3d4
            T->O Network Connection ID: 0x12345678
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            O->T API: 1000.000ms
            T->O API: 1000.000ms
            Application Reply Size: 0 words
            Reserved: 0x00
            [Connection Information]
                [Connection Path Size: 4 words]
                [Route/Connection Path: Assembly, Instance: 0x97, Connection Point: 0x96, Connection Point: 0x64]
                    [Path Segment: 0x20 (8-Bit Class Segment)]
                        [001. .... = Path Segment Type: Logical Segment (1)]
                        [...0 00.. = Logical Segment Type: Class ID (0)]
                        [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                        [Class: Assembly (0x04)]
                    [Path Segment: 0x24 (8-Bit Instance Segment)]
                        [001. .... = Path Segment Type: Logical Segment (1)]
                        [...0 01.. = Logical Segment Type: Instance ID (1)]
                        [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                        [Instance: 0x97]
                    [Path Segment: 0x2c (8-Bit Connection Point Segment)]
                        [001. .... = Path Segment Type: Logical Segment (1)]
                        [...0 11.. = Logical Segment Type: Connection Point (3)]
                        [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                        [Connection Point: 0x96]
                    [Path Segment: 0x2c (8-Bit Connection Point Segment)]
                        [001. .... = Path Segment Type: Logical Segment (1)]
                        [...0 11.. = Logical Segment Type: Connection Point (3)]
                        [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                        [Connection Point: 0x64]

    Hex Dump:
    0000   6f 00 2e 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 1e 00 d4 00 00 00 d4 c3 b2 a1
    0030   78 56 34 12 01 00 56 01 45 23 01 00 40 42 0f 00
    0040   40 42 0f 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x2e, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1e, 0x00, 0xd4, 0x00, 0x00, 0x00, 0xd4,
        0xc3, 0xb2, 0xa1, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00,
        0x40, 0x42, 0x0f, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x00, 0x00,
    ];

    let expected_response = ForwardOpenResponse {
        o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: sample_connection_triad(),
        o2t_actual_packet_interval: REQUESTED_PACKET_INTERVAL_MICROSECONDS,
        t2o_actual_packet_interval: REQUESTED_PACKET_INTERVAL_MICROSECONDS,
        application_reply_size: 0,
        application_reply: vec![],
    };

    let expected_response_object = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(46),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::ForwardOpen),
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
    let response_object = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    assert_eq!(expected_response_object, response_object);

    // The reply data as the typed reply
    assert_eq!(
        ConnectionManagerResponse::from_message_router_response(
            response_object.response().unwrap()
        )
        .unwrap(),
        ConnectionManagerResponse::ForwardOpen(expected_response)
    );

    // Writing the typed reply (what an adapter does) reproduces the packet, reserved byte included
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    expected_response_object.write(&mut writer).unwrap();

    assert_eq_hex!(raw_bytes, byte_array_buffer);
}

#[test]
fn test_deserialize_forward_open_rejected_response() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 32
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
                    Length: 16
            [Request In: 1]
            [Time: 0.000001000 seconds]
    Common Industrial Protocol
        Service: Unknown Service (0x54) (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Unknown (0x54)
        Status: Connection failure:
            General Status: Connection failure (0x01)
            Additional Status Size: 1 word
            Additional Status
                Additional Status: 0x0100
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
        Service: Forward Open (Response)
            1... .... = Request/Response: Response (0x1)
            .101 0100 = Service: Forward Open (0x54)
        Status: Connection failure, Extended: Connection in use or duplicate Forward Open
            General Status: Connection failure (0x01)
            Additional Status Size: 1 word
            Extended Status: Connection in use or duplicate Forward Open (0x0100)
            Additional Status
        Command Specific Data
            Connection Serial Number: 0x0001
            Originator Vendor ID: Bekaert Engineering NV (0x0156)
            Originator Serial Number: 0x00012345
            Remaining Path Size: 0 words
            Reserved: 0x00
            [Connection Path Size: 4 words]
            [Route/Connection Path: Assembly, Instance: 0x97, Connection Point: 0x96, Connection Point: 0x64]
                [Path Segment: 0x20 (8-Bit Class Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 00.. = Logical Segment Type: Class ID (0)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Class: Assembly (0x04)]
                [Path Segment: 0x24 (8-Bit Instance Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 01.. = Logical Segment Type: Instance ID (1)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Instance: 0x97]
                [Path Segment: 0x2c (8-Bit Connection Point Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 11.. = Logical Segment Type: Connection Point (3)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Connection Point: 0x96]
                [Path Segment: 0x2c (8-Bit Connection Point Segment)]
                    [001. .... = Path Segment Type: Logical Segment (1)]
                    [...0 11.. = Logical Segment Type: Connection Point (3)]
                    [.... ..00 = Logical Segment Format: 8-bit Logical Segment (0)]
                    [Connection Point: 0x64]

    Hex Dump:
    0000   6f 00 20 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 02 00
    0020   00 00 00 00 b2 00 10 00 d4 00 01 01 00 01 01 00
    0030   56 01 45 23 01 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x20, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x10, 0x00, 0xd4, 0x00, 0x01, 0x01, 0x00,
        0x01, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
    ];

    let expected_response = UnsuccessfulResponse {
        connection_triad: sample_connection_triad(),
        remaining_path_size: Some(0),
        reserved: Some(0),
    };

    let expected_response_object = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(32),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData(RRPacketData::new_unconnected(
            CIP_INTERFACE_HANDLE,
            NO_ENCAPSULATION_TIMEOUT,
            MessageRouterResponse {
                service_container: ServiceContainer::new_response(ServiceCode::ForwardOpen),
                response_data: ResponseData {
                    status: ResponseStatusCode::ConnectionFailure,
                    additional_status_size: 1,
                    additional_status: vec![0x0100],
                    data: CipDataOpt::Typed(Box::new(expected_response.clone())),
                },
            },
        )),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes);
    let mut buf_reader = std::io::BufReader::new(byte_cursor);
    let response_object = ResponseObjectAssembly::read_response(&mut buf_reader).unwrap();

    assert_eq!(expected_response_object, response_object);

    // The reply data as the typed reply: a rejection parses as `Ok`, not as an error
    let response = response_object.response().unwrap();
    assert_eq!(
        ConnectionManagerResponse::from_message_router_response(response).unwrap(),
        ConnectionManagerResponse::Unsuccessful(expected_response)
    );

    // The general status and the extended status stay on the Message Router response
    assert_eq!(
        response.response_data.status,
        ResponseStatusCode::ConnectionFailure
    );
    assert_eq!(
        ConnectionManagerExtendedStatus::from_additional_status(
            &response.response_data.additional_status
        ),
        Some(ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen)
    );
}
