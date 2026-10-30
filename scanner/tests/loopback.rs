//! The five stages against a fake adapter on the loopback interface, which echoes one I/O
//! packet. `tests/integration` has the steps for a real adapter.

use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddrV4};

use bilge::prelude::{u4, u9};
use binrw::BinRead;
use tokio::net::{TcpListener, UdpSocket};

use adapter::packet::{read_packet, write_packet};
use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseResponse;
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RealTimeFormat,
    RedundantOwner, StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::io_data::RunOrIdle;
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand};
use eipscanne_rs::eip::constants::NO_ENCAPSULATION_TIMEOUT;
use eipscanne_rs::eip::io_packet::EnIpIoPacket;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};
use scanner::implicit::connection::{forward_close, forward_open_implicit};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{accept_t2o_packet, recv_io_packet};
use scanner::session::{Session, SessionConfig};

const SESSION_HANDLE: CipUdint = 0x0000_0042;
const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;
const LOCALHOST: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// The 16-bit parameter word of a point-to-point, exclusive-owner direction
fn parameters(connection_size: u16) -> NetworkConnectionParameters {
    NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::new(connection_size))
            .connection_size_type(ConnectionSizeType::Fixed)
            .priority(ConnectionPriority::Scheduled)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    )
}

/// 4 bytes each way (10 and 6 with overhead), every 100 ms
fn request() -> ForwardOpenRequest {
    ForwardOpenRequest {
        priority_time_tick: PriorityTimeTick::builder()
            .tick_time(u4::new(10))
            .priority(false)
            .build(),
        timeout_ticks: 5,
        o2t_network_connection_id: 0,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: ConnectionTriad {
            connection_serial_number: 1,
            originator_vendor_id: 342,
            originator_serial_number: 0x0001_2345,
        },
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        o2t_requested_packet_interval: 100_000,
        o2t_network_connection_parameters: parameters(10),
        t2o_requested_packet_interval: 100_000,
        t2o_network_connection_parameters: parameters(6),
        transport_type_trigger: TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class1)
            .production_trigger(ProductionTrigger::Cyclic)
            .direction(Direction::Client)
            .build(),
        connection_path: CipPath::new_assembly_connection(151, 150, 100),
    }
}

// ======= The fake adapter ========

/// The Message Router request a Send RR Data packet carries
fn extract_message_router_request(packet: &EnIpPacket) -> Option<&MessageRouterRequest> {
    let CommandSpecificData::SendRrData(rr_data) = &packet.command_specific_data else {
        return None;
    };
    match &rr_data.cip_message {
        CipMessage::Request(request) => Some(request),
        CipMessage::Response(_) => None,
    }
}

/// A successful Connection Manager reply carrying `data` and `socket_addr_info_items`
fn connection_manager_reply(
    service: ServiceCode,
    data: impl CipData + 'static,
    socket_addr_info_items: SocketAddrInfoItems,
) -> EnIpPacket {
    let response = MessageRouterResponse {
        service_container: ServiceContainer::new_response(service),
        response_data: ResponseData {
            status: ResponseStatusCode::Success,
            additional_status_size: 0,
            additional_status: vec![],
            data: CipDataOpt::Typed(Box::new(data)),
        },
    };
    let mut reply =
        EnIpPacket::new_send_rr_data(SESSION_HANDLE, NO_ENCAPSULATION_TIMEOUT, response);
    let CommandSpecificData::SendRrData(rr_data) = &mut reply.command_specific_data else {
        unreachable!("a Send RR Data packet");
    };
    rr_data.socket_addr_info_items = socket_addr_info_items;
    reply
}

/// The Forward_Open request inside a packet, as the adapter parses it
fn extract_forward_open_request(packet: &EnIpPacket) -> ForwardOpenRequest {
    let Some(message) = extract_message_router_request(packet) else {
        panic!("expected a request, got {:?}", packet.command_specific_data);
    };
    assert_eq!(
        message.service_container.service(),
        ServiceCode::ForwardOpen
    );
    let CipDataOpt::Raw(data) = &message.request_data.additional_data else {
        panic!("expected raw request data");
    };
    ForwardOpenRequest::read_le_args(&mut Cursor::new(data), (false,))
        .expect("the scanner sends a well-formed Forward_Open")
}

/// Plays the adapter for one connection; returns the output packet's sequence number and data
async fn fake_adapter(listener: TcpListener, io_socket: UdpSocket) -> Vec<u8> {
    let (mut stream, _) = listener.accept().await.expect("the scanner connects once");

    // 1. RegisterSession
    let request = read_packet(&mut stream)
        .await
        .expect("the scanner sends Register Session");
    assert_eq!(request.header.command, EnIpCommand::RegisterSession);
    let mut reply = EnIpPacket::new_registration();
    reply.header.session_handle = SESSION_HANDLE;
    write_packet(&mut stream, &reply)
        .await
        .expect("the scanner waits for the Register Session reply");

    // 2. Forward_Open: grant the intervals, pick the O->T ID and the outputs' port
    let request = read_packet(&mut stream)
        .await
        .expect("the scanner sends Forward_Open");
    assert_eq!(request.header.session_handle, SESSION_HANDLE);
    let forward_open = extract_forward_open_request(&request);
    assert_eq!(
        forward_open.t2o_network_connection_id,
        T2O_NETWORK_CONNECTION_ID
    );
    // The inputs go to the port the T->O Socket Address Info item names
    let scanner_io_port: u16 = request
        .command_specific_data
        .as_send_rr_data()
        .and_then(|rr_data| rr_data.socket_addr_info_items.t2o)
        .expect("the scanner names its ephemeral I/O port")
        .socket_address()
        .port();
    let io_port = io_socket
        .local_addr()
        .expect("a bound socket has a local address")
        .port();
    write_packet(
        &mut stream,
        &connection_manager_reply(
            ServiceCode::ForwardOpen,
            ForwardOpenResponse {
                o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
                t2o_network_connection_id: forward_open.t2o_network_connection_id,
                connection_triad: forward_open.connection_triad,
                o2t_actual_packet_interval: forward_open.o2t_requested_packet_interval,
                t2o_actual_packet_interval: forward_open.t2o_requested_packet_interval,
                application_reply_size: 0,
                application_reply: vec![],
            },
            SocketAddrInfoItems {
                o2t: Some(SocketAddrInfo::from(SocketAddrV4::new(
                    Ipv4Addr::UNSPECIFIED,
                    io_port,
                ))),
                t2o: None,
            },
        ),
    )
    .await
    .expect("the scanner waits for the Forward_Open reply");

    // 3. One output packet in, the same bytes back as an input packet
    let (packet, _) = recv_io_packet(&io_socket)
        .await
        .expect("the scanner sends one output packet");
    assert_eq!(
        packet.sequenced_address.connection_id,
        O2T_NETWORK_CONNECTION_ID
    );
    let CipDataOpt::Raw(data) = &packet.connected_data else {
        panic!("expected raw connected data");
    };
    // Sequence count (2) and run/idle header (4) precede the outputs
    let outputs = data[6..].to_vec();
    let mut input_data = 1u16.to_le_bytes().to_vec();
    input_data.extend_from_slice(&outputs);
    send_io_packet(
        &io_socket,
        &EnIpIoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(input_data)),
        SocketAddrV4::new(LOCALHOST, scanner_io_port),
    )
    .await
    .expect("UDP sends to the scanner's bound loopback port");

    // 4. Forward_Close
    let request = read_packet(&mut stream)
        .await
        .expect("the scanner sends Forward_Close");
    let Some(message) = extract_message_router_request(&request) else {
        panic!("expected a Forward_Close request");
    };
    assert_eq!(
        message.service_container.service(),
        ServiceCode::ForwardClose
    );
    write_packet(
        &mut stream,
        &connection_manager_reply(
            ServiceCode::ForwardClose,
            ForwardCloseResponse {
                connection_triad: forward_open.connection_triad,
                application_reply_size: 0,
                application_reply: vec![],
            },
            SocketAddrInfoItems::empty(),
        ),
    )
    .await
    .expect("the scanner waits for the Forward_Close reply");

    // 5. UnregisterSession
    let request = read_packet(&mut stream)
        .await
        .expect("the scanner sends Unregister Session");
    assert_eq!(request.header.command, EnIpCommand::UnRegisterSession);

    outputs
}

// ======= The test ========

#[tokio::test]
async fn one_connection_against_a_fake_adapter() {
    let listener = TcpListener::bind((LOCALHOST, 0))
        .await
        .expect("port 0 lets the OS pick a free loopback port");
    let adapter_port = listener
        .local_addr()
        .expect("a bound listener has a local address")
        .port();
    let adapter_io_socket = UdpSocket::bind((LOCALHOST, 0))
        .await
        .expect("port 0 lets the OS pick a free loopback port");
    // An ephemeral port, so the test never collides with a running scanner
    let scanner_io_socket = UdpSocket::bind((LOCALHOST, 0))
        .await
        .expect("port 0 lets the OS pick a free loopback port");
    let scanner_io_port = scanner_io_socket
        .local_addr()
        .expect("a bound socket has a local address")
        .port();

    let adapter = tokio::spawn(fake_adapter(listener, adapter_io_socket));

    // 1. Session
    let mut session = Session::register((LOCALHOST, adapter_port), SessionConfig::default())
        .await
        .expect("the fake adapter grants Register Session");
    assert_eq!(session.session_handle(), SESSION_HANDLE);
    assert_eq!(session.peer_ip(), LOCALHOST);

    // 2. Open
    let connection = forward_open_implicit(
        &mut session,
        request(),
        RealTimeFormat::Header32Bit,
        RealTimeFormat::Modeless,
        scanner_io_port,
    )
    .await
    .expect("the fake adapter grants the Forward_Open");
    assert_eq!(
        connection.response.o2t_network_connection_id,
        O2T_NETWORK_CONNECTION_ID
    );
    assert_eq!(connection.o2t_endpoint.ip(), &LOCALHOST);
    assert_ne!(
        connection.o2t_endpoint.port(),
        2222,
        "the Socket Address Info port is used"
    );

    // 3. One cycle each way
    let outputs = [0xde, 0xad, 0xbe, 0xef];
    let packet = build_o2t_packet(
        &connection,
        7,
        1,
        CipDataOpt::Raw(outputs.to_vec()),
        RunOrIdle::Run,
    )
    .expect("the four output bytes fit the O->T size");
    send_io_packet(&scanner_io_socket, &packet, connection.o2t_endpoint)
        .await
        .expect("UDP sends to the fake adapter's bound loopback port");

    let (packet, from) = recv_io_packet(&scanner_io_socket)
        .await
        .expect("the fake adapter echoes the outputs as one input packet");
    let (address, inputs) = accept_t2o_packet(&connection, &packet, from, None)
        .expect("the fake adapter sends on the granted T->O connection ID");
    assert_eq!(address.encapsulation_sequence_number, 1);
    assert_eq!(inputs.cip_sequence_count, Some(1));
    assert_eq!(inputs.output_control_header, None);
    assert_eq!(inputs.data, CipDataOpt::Raw(outputs.to_vec()));

    // 4. Close, 5. End session
    forward_close(&mut session, &connection.request)
        .await
        .expect("the fake adapter grants the Forward_Close");
    session
        .unregister()
        .await
        .expect("the fake adapter reads Unregister Session");

    assert_eq!(
        adapter.await.expect("the fake adapter's asserts hold"),
        outputs
    );
}
