//! The five stages against a fake adapter on the loopback interface: the fake answers a
//! RegisterSession, a Forward_Open (with a Socket Address Info O->T item pointing at its own
//! UDP port), echoes one I/O packet, then answers the Forward_Close and reads the
//! UnregisterSession. No real adapter is needed; `tests/integration` has the OpENer steps.

use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddrV4};

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use bilge::prelude::{u4, u9};
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
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{
    MessageRouterResponse, ResponseData, ResponseStatusCode,
};
use eipscanne_rs::cip::message::shared::{ServiceCode, ServiceContainer};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{
    CommandSpecificData, EnIpCommand, EncapsStatusCode, RRPacketData, RegisterData,
};
use eipscanne_rs::eip::constants::{
    CIP_INTERFACE_HANDLE, DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT,
    ENCAPSULATION_PROTOCOL_VERSION, NO_ENCAPSULATION_TIMEOUT, REGISTER_SESSION_OPTION_FLAGS,
};
use eipscanne_rs::eip::description::CommonPacketItem;
use eipscanne_rs::eip::io_packet::IoPacket;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::eip::sockaddr::SockaddrInfo;

use scanner::implicit::connection::{forward_close, forward_open};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{accept_t2o_packet, recv_io_packet};
use scanner::session::Session;

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

/// 4 output bytes behind a sequence count and a run/idle header (10), 4 input bytes behind a
/// sequence count (6), both every 100 ms
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

/// Reads one encapsulation packet the way an adapter does: the header, then its Length
async fn read_request(stream: &mut TcpStream) -> EnIpPacket {
    let mut bytes = vec![0u8; 24];
    stream.read_exact(&mut bytes).await.unwrap();
    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes)).unwrap();
    bytes.resize(24 + usize::from(header.length.unwrap()), 0);
    stream.read_exact(&mut bytes[24..]).await.unwrap();
    EnIpPacket::read(&mut Cursor::new(&bytes)).unwrap()
}

async fn write_reply(stream: &mut TcpStream, reply: &EnIpPacket) {
    let mut bytes = Cursor::new(Vec::new());
    reply.write(&mut bytes).unwrap();
    stream.write_all(bytes.get_ref()).await.unwrap();
}

fn header(command: EnIpCommand) -> EncapsulationHeader {
    EncapsulationHeader {
        command,
        length: None,
        session_handle: SESSION_HANDLE,
        status_code: EncapsStatusCode::Success,
        sender_context: EMPTY_SENDER_CONTEXT,
        options: DEFAULT_ENCAPSULATION_OPTIONS,
    }
}

/// A successful Connection Manager reply carrying `data`, plus any extra items
fn connection_manager_reply(
    service: ServiceCode,
    data: impl eipscanne_rs::cip::message::data::CipData + 'static,
    extra_items: Vec<CommonPacketItem>,
) -> EnIpPacket {
    let mut rr_data = RRPacketData::new_unconnected(
        CIP_INTERFACE_HANDLE,
        NO_ENCAPSULATION_TIMEOUT,
        MessageRouterResponse {
            service_container: ServiceContainer::new_response(service),
            response_data: ResponseData {
                status: ResponseStatusCode::Success,
                additional_status_size: 0,
                additional_status: vec![],
                data: CipDataOpt::Typed(Box::new(data)),
            },
        },
    );
    rr_data.items.extend(extra_items);
    EnIpPacket {
        header: header(EnIpCommand::SendRrData),
        command_specific_data: CommandSpecificData::SendRrData(rr_data),
    }
}

/// The Forward_Open request inside a packet, as the adapter parses it
fn forward_open_request_of(packet: &EnIpPacket) -> ForwardOpenRequest {
    let Some(CipMessage::Request(message)) = packet.cip_message() else {
        panic!("expected a request, got {:?}", packet.cip_message());
    };
    assert_eq!(
        message.service_container.service(),
        ServiceCode::ForwardOpen
    );
    let CipDataOpt::Raw(data) = &message.request_data.additional_data else {
        panic!("expected raw request data");
    };
    ForwardOpenRequest::read_le_args(&mut Cursor::new(data), (false,)).unwrap()
}

/// Plays the adapter for one connection: the sequence number of the output packet it received
/// and the outputs themselves are returned so the test can check them
async fn fake_adapter(
    listener: TcpListener,
    io_socket: UdpSocket,
    scanner_io_port: u16,
) -> Vec<u8> {
    let (mut stream, _) = listener.accept().await.unwrap();

    // 1. RegisterSession
    let request = read_request(&mut stream).await;
    assert_eq!(request.header.command, EnIpCommand::RegisterSession);
    write_reply(
        &mut stream,
        &EnIpPacket {
            header: header(EnIpCommand::RegisterSession),
            command_specific_data: CommandSpecificData::RegisterSession(RegisterData {
                protocol_version: ENCAPSULATION_PROTOCOL_VERSION,
                option_flags: REGISTER_SESSION_OPTION_FLAGS,
            }),
        },
    )
    .await;

    // 2. Forward_Open: grant the requested intervals, pick the O->T connection ID and say
    //    which port the outputs go to
    let request = read_request(&mut stream).await;
    assert_eq!(request.header.session_handle, SESSION_HANDLE);
    let forward_open = forward_open_request_of(&request);
    assert_eq!(
        forward_open.t2o_network_connection_id,
        T2O_NETWORK_CONNECTION_ID
    );
    let io_port = io_socket.local_addr().unwrap().port();
    write_reply(
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
            vec![CommonPacketItem::O2TSockAddrInfo(SockaddrInfo::from(
                SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, io_port),
            ))],
        ),
    )
    .await;

    // 3. One output packet in, the same bytes back as an input packet
    let (packet, _) = recv_io_packet(&io_socket).await.unwrap();
    assert_eq!(
        packet.sequenced_address().unwrap().connection_id,
        O2T_NETWORK_CONNECTION_ID
    );
    let Some(CipDataOpt::Raw(data)) = packet.connected_data() else {
        panic!("expected raw connected data");
    };
    // Sequence count (2) and run/idle header (4) precede the outputs
    let outputs = data[6..].to_vec();
    let mut input_data = 1u16.to_le_bytes().to_vec();
    input_data.extend_from_slice(&outputs);
    send_io_packet(
        &io_socket,
        &IoPacket::new(T2O_NETWORK_CONNECTION_ID, 1, CipDataOpt::Raw(input_data)),
        SocketAddrV4::new(LOCALHOST, scanner_io_port),
    )
    .await
    .unwrap();

    // 4. Forward_Close
    let request = read_request(&mut stream).await;
    let Some(CipMessage::Request(message)) = request.cip_message() else {
        panic!("expected a Forward_Close request");
    };
    assert_eq!(
        message.service_container.service(),
        ServiceCode::ForwardClose
    );
    write_reply(
        &mut stream,
        &connection_manager_reply(
            ServiceCode::ForwardClose,
            ForwardCloseResponse {
                connection_triad: forward_open.connection_triad,
                application_reply_size: 0,
                application_reply: vec![],
            },
            vec![],
        ),
    )
    .await;

    // 5. UnregisterSession
    let request = read_request(&mut stream).await;
    assert_eq!(request.header.command, EnIpCommand::UnRegisterSession);

    outputs
}

// ======= The test ========

#[tokio::test]
async fn one_connection_against_a_fake_adapter() {
    let listener = TcpListener::bind((LOCALHOST, 0)).await.unwrap();
    let adapter_port = listener.local_addr().unwrap().port();
    let adapter_io_socket = UdpSocket::bind((LOCALHOST, 0)).await.unwrap();
    // An ephemeral port instead of 2222, so the test never collides with a running scanner
    let scanner_io_socket = UdpSocket::bind((LOCALHOST, 0)).await.unwrap();
    let scanner_io_port = scanner_io_socket.local_addr().unwrap().port();

    let adapter = tokio::spawn(fake_adapter(listener, adapter_io_socket, scanner_io_port));

    // 1. Session
    let mut session = Session::register((LOCALHOST, adapter_port)).await.unwrap();
    assert_eq!(session.session_handle(), SESSION_HANDLE);
    assert_eq!(session.peer_ip(), LOCALHOST);

    // 2. Open
    let connection = forward_open(
        &mut session,
        request(),
        RealTimeFormat::Header32Bit,
        RealTimeFormat::Modeless,
    )
    .await
    .unwrap();
    assert_eq!(
        connection.response.o2t_network_connection_id,
        O2T_NETWORK_CONNECTION_ID
    );
    assert_eq!(connection.o2t_endpoint.ip(), &LOCALHOST);
    assert_ne!(
        connection.o2t_endpoint.port(),
        2222,
        "the Sockaddr Info port is used"
    );

    // 3. One cycle each way
    let outputs = [0xde, 0xad, 0xbe, 0xef];
    let packet =
        build_o2t_packet(&connection, 7, 1, CipDataOpt::Raw(outputs.to_vec()), true).unwrap();
    send_io_packet(&scanner_io_socket, &packet, connection.o2t_endpoint)
        .await
        .unwrap();

    let (packet, from) = recv_io_packet(&scanner_io_socket).await.unwrap();
    let (address, inputs) = accept_t2o_packet(&connection, None, &packet, from).unwrap();
    assert_eq!(address.encapsulation_sequence_number, 1);
    assert_eq!(inputs.cip_sequence_count, Some(1));
    assert_eq!(inputs.run_idle_header, None);
    assert_eq!(inputs.data, CipDataOpt::Raw(outputs.to_vec()));

    // 4. Close, 5. End session
    forward_close(&mut session, &connection).await.unwrap();
    session.unregister().await.unwrap();

    assert_eq!(adapter.await.unwrap(), outputs);
}
