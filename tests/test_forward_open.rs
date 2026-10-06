mod common;

use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};

use bilge::prelude::{Integer, u4, u9};

use hex_test_macros::prelude::*;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ConnectionParameters, ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionDirection, ConnectionPriority, ConnectionSizeError, ConnectionSizeType,
    ConnectionTimeoutMultiplier, ConnectionType, Direction, LargeNetworkConnectionParameters,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RealTimeFormat,
    RedundantOwner, StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::connection_manager::response::{
    ConnectionManagerError, ConnectionManagerExtendedStatus, ConnectionManagerFailure,
    ConnectionManagerResponse,
};
use eipscanne_rs::cip::connection_manager::shared::{
    ApplicationReply, ConnectionTriad, RemainingPath, UnsuccessfulResponse,
};
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
    ETHERNET_IP_IO_UDP_PORT, NO_ENCAPSULATION_TIMEOUT,
};
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::packet::EncapsulationHeader;
use eipscanne_rs::eip::sockaddr::SockaddrInfo;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

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

/// Connection serial number and originator identity of the captures
fn sample_connection_triad() -> ConnectionTriad {
    ConnectionTriad {
        connection_serial_number: CONNECTION_SERIAL_NUMBER,
        originator_vendor_id: ORIGINATOR_VENDOR_ID,
        originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
    }
}

/// The UDP end point a target announces for the O->T packets in its Forward_Open reply
fn sample_address() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), ETHERNET_IP_IO_UDP_PORT)
}

/// The parameters of EIPScanner's implicit messaging example: 32-byte assemblies both ways,
/// a 1 s packet interval, class 1 cyclic, point-to-point with scheduled priority
fn sample_parameters(large: bool) -> ConnectionParameters {
    ConnectionParameters {
        priority_time_tick: sample_priority_time_tick(),
        timeout_ticks: TIMEOUT_TICKS,
        o2t_network_connection_id: REQUESTED_O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: sample_connection_triad(),
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
        connection_triad: sample_connection_triad(),
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

/// The request data of a packet read from the wire, parsed as a Forward_Open request of the
/// given parameter width (what an adapter does with the packet)
fn forward_open_request_of(packet: &RequestObjectAssembly, large: bool) -> ForwardOpenRequest {
    let Some(CipMessage::Request(message)) = packet.cip_message() else {
        panic!(
            "expected a Message Router request, got {:?}",
            packet.cip_message()
        );
    };
    let data = message.request_data.additional_data.to_bytes().unwrap();
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

    let request = ForwardOpenRequest::new(&sample_parameters(false)).unwrap();

    assert_eq!(request.service_code(), ServiceCode::ForwardOpen);
    assert_eq!(
        request,
        sample_request(
            NetworkConnectionParameters::Standard(sample_standard_parameters(O2T_CONNECTION_SIZE)),
            NetworkConnectionParameters::Standard(sample_standard_parameters(T2O_CONNECTION_SIZE)),
        )
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

    let request = ForwardOpenRequest::new(&sample_parameters(true)).unwrap();

    assert_eq!(request.service_code(), ServiceCode::LargeForwardOpen);
    assert_eq!(
        request,
        sample_request(
            NetworkConnectionParameters::Large(sample_large_parameters(O2T_CONNECTION_SIZE)),
            NetworkConnectionParameters::Large(sample_large_parameters(T2O_CONNECTION_SIZE)),
        )
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
        o2t_api: RPI_MICROSECONDS,
        t2o_api: RPI_MICROSECONDS,
        application_reply: ApplicationReply {
            application_reply_size: 0,
            application_reply: vec![],
        },
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
fn test_deserialize_forward_open_success_response_with_sockaddr_item() {
    /*
    EtherNet/IP (Industrial Protocol), Session: 0x00000003, Send RR Data
        Encapsulation Header
            Command: Send RR Data (0x006f)
            Length: 66
            Session Handle: 0x00000003
            Status: Success (0x00000000)
            Sender Context: 0000000000000000
            Options: 0x00000000
        Command Specific Data
            Interface Handle: CIP (0x00000000)
            Timeout: 0
            Item Count: 3
                Type ID: Null Address Item (0x0000)
                    Length: 0
                Type ID: Unconnected Data Item (0x00b2)
                    Length: 30
                Type ID: Socket Address Info O->T (0x8000)
                    Length: 16
                    sin_family: 2
                    sin_port: 2222
                    sin_addr: 192.168.1.10
                    sin_zero: 0000000000000000
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
    0000   6f 00 42 00 03 00 00 00 00 00 00 00 00 00 00 00
    0010   00 00 00 00 00 00 00 00 00 00 00 00 00 00 03 00
    0020   00 00 00 00 b2 00 1e 00 d4 00 00 00 d4 c3 b2 a1
    0030   78 56 34 12 01 00 56 01 45 23 01 00 40 42 0f 00
    0040   40 42 0f 00 00 00 00 80 10 00 00 02 08 ae c0 a8
    0050   01 0a 00 00 00 00 00 00 00 00
    */
    let raw_bytes: Vec<CipByte> = vec![
        0x6f, 0x00, 0x42, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x1e, 0x00, 0xd4, 0x00, 0x00, 0x00, 0xd4,
        0xc3, 0xb2, 0xa1, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00,
        0x40, 0x42, 0x0f, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x80, 0x10, 0x00, 0x00,
        0x02, 0x08, 0xae, 0xc0, 0xa8, 0x01, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let expected_response = ForwardOpenResponse {
        o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: sample_connection_triad(),
        o2t_api: RPI_MICROSECONDS,
        t2o_api: RPI_MICROSECONDS,
        application_reply: ApplicationReply {
            application_reply_size: 0,
            application_reply: vec![],
        },
    };

    let expected_response_object = ResponseObjectAssembly {
        header: EncapsulationHeader {
            command: EnIpCommand::SendRrData,
            length: Some(66),
            session_handle: CLEARLINK_IO_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::SendRrData({
            let mut rr_data = RRPacketData::new_unconnected(
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
            );
            rr_data
                .items
                .push(CommonPacketItem::O2TSockAddrInfo(sample_address().into()));
            rr_data
        }),
    };

    let byte_cursor = std::io::Cursor::new(raw_bytes);
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

    // The item announcing the target's UDP end point is reachable through the packet's items
    let sockaddr_items: Vec<&CommonPacketItem> = response_object.sockaddr_info_items().collect();
    let [sockaddr_item] = sockaddr_items.as_slice() else {
        panic!("expected one Sockaddr Info item, got {sockaddr_items:?}");
    };

    assert_eq!(
        **sockaddr_item,
        CommonPacketItem::O2TSockAddrInfo(sample_address().into())
    );
    assert_eq!(
        sockaddr_item
            .sockaddr_info()
            .map(SockaddrInfo::socket_address),
        Some(sample_address())
    );
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
        remaining_path: Some(RemainingPath {
            remaining_path_size: 0,
            reserved: 0,
        }),
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

    // The typed parse reports the rejection with its extended status
    let error = ConnectionManagerResponse::from_message_router_response(
        response_object.response().unwrap(),
    )
    .unwrap_err();
    let ConnectionManagerError::Rejected(failure) = error else {
        panic!("expected a rejected Forward_Open, got {error:?}");
    };

    assert_eq!(
        failure,
        ConnectionManagerFailure {
            general_status: ResponseStatusCode::ConnectionFailure,
            extended_status: Some(
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
            ),
            additional_status: vec![0x0100],
            response: expected_response,
        }
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

    let error = ConnectionManagerResponse::from_message_router_response(&message_router_response)
        .unwrap_err();

    assert!(matches!(error, ConnectionManagerError::Malformed(_)));
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

fn read_extended_status(raw_bytes: &[CipByte]) -> ConnectionManagerExtendedStatus {
    let mut reader = std::io::Cursor::new(raw_bytes);
    ConnectionManagerExtendedStatus::read(&mut reader).unwrap()
}

fn write_extended_status(status: &ConnectionManagerExtendedStatus) -> Vec<CipByte> {
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);
    status.write(&mut writer).unwrap();
    byte_array_buffer
}

#[test]
fn test_connection_manager_extended_status_codes() {
    // Each status is one 16-bit word, least significant byte first
    let known_statuses = [
        (
            [0x00, 0x01],
            ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen,
        ),
        (
            [0x07, 0x01],
            ConnectionManagerExtendedStatus::TargetConnectionNotFound,
        ),
        (
            [0x1b, 0x01],
            ConnectionManagerExtendedStatus::RpiSmallerThanProductionInhibitTime,
        ),
        (
            [0x07, 0x02],
            ConnectionManagerExtendedStatus::UnconnectedAcknowledgeWithoutReply,
        ),
        (
            [0x1f, 0x03],
            ConnectionManagerExtendedStatus::NoUserConfigurableLinkConsumerResourcesConfiguredInTheProducingModule,
        ),
        ([0x00, 0x08], ConnectionManagerExtendedStatus::NetworkLinkOffline),
        (
            [0x14, 0x08],
            ConnectionManagerExtendedStatus::InvalidProduceConsumeDataFormat,
        ),
    ];

    for (raw_bytes, status) in known_statuses {
        assert_eq!(read_extended_status(&raw_bytes), status);
        assert_eq_hex!(raw_bytes.to_vec(), write_extended_status(&status));
    }

    // A code without a name keeps its value and writes back unchanged
    for raw_bytes in [[0x01, 0x01], [0x34, 0x12]] {
        let status = read_extended_status(&raw_bytes);
        assert_eq!(
            status,
            ConnectionManagerExtendedStatus::Unknown(u16::from_le_bytes(raw_bytes))
        );
        assert_eq_hex!(raw_bytes.to_vec(), write_extended_status(&status));
    }
}
