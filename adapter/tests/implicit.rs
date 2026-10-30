//! One class 1 connection opened and exchanged on the loopback interface.

use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bilge::prelude::{u2, u4, u9};
use binrw::{BinRead, BinWrite};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::timeout;

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RealTimeFormat,
    RedundantOwner, StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::io_data::{CipIoData, OutputControlHeader, RunOrIdle};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::CommandSpecificData;
use eipscanne_rs::eip::io_packet::EnIpIoPacket;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::eip::socket_addr::SocketAddrInfo;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use adapter::implicit::{ImplicitDevice, ImplicitOptions};
use adapter::message_router::{ExplicitOptions, MessageRouter};
use adapter::packet::{read_packet, write_packet};
use adapter::server::serve_implicit;

/// The handle of the first session a server grants
const SESSION_HANDLE: CipUdint = 1;
const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;
const OUTPUT_INSTANCE: u8 = 150;
const INPUT_INSTANCE: u8 = 100;
/// Data bytes of each assembly; 10 and 6 bytes with the overhead of each direction
const DATA_SIZE: u16 = 4;
const O2T_CONNECTION_SIZE: u16 = 10;
const T2O_CONNECTION_SIZE: u16 = 6;
/// How long a test waits for a packet that is due
const PACKET_WAIT: Duration = Duration::from_secs(2);

/// Implicit messaging only: echoes its outputs as its inputs and counts its `enter_idle` calls
struct EchoDevice {
    outputs: Mutex<[u8; DATA_SIZE as usize]>,
    idle_count: AtomicUsize,
}

impl MessageRouter for EchoDevice {}

impl ImplicitDevice for EchoDevice {
    type Outputs = [u8; DATA_SIZE as usize];
    type Inputs = [u8; DATA_SIZE as usize];

    fn assembly_layout(&self, instance: u16) -> Option<(u16, RealTimeFormat)> {
        match u8::try_from(instance) {
            Ok(OUTPUT_INSTANCE) => Some((DATA_SIZE, RealTimeFormat::Header32Bit)),
            Ok(INPUT_INSTANCE) => Some((DATA_SIZE, RealTimeFormat::Modeless)),
            _ => None,
        }
    }

    fn consume_outputs(&self, _instance: u16, outputs: [u8; DATA_SIZE as usize]) {
        *self
            .outputs
            .lock()
            .expect("nothing panics while holding the outputs lock") = outputs;
    }

    fn produce_inputs(&self, _instance: u16) -> [u8; DATA_SIZE as usize] {
        *self
            .outputs
            .lock()
            .expect("nothing panics while holding the outputs lock")
    }

    fn enter_idle(&self, _instance: u16) {
        self.idle_count.fetch_add(1, Ordering::SeqCst);
    }
}

/// A registered session to an `EchoDevice`, and the scanner's I/O socket
struct Scanner {
    device: Arc<EchoDevice>,
    stream: TcpStream,
    io_socket: UdpSocket,
}

impl Scanner {
    /// Serves an `EchoDevice` on the loopback interface and registers a session with it
    async fn connect() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("port 0 lets the OS pick a free loopback port");
        let address = listener
            .local_addr()
            .expect("a bound listener has a local address");
        let device = Arc::new(EchoDevice {
            outputs: Mutex::new([0; DATA_SIZE as usize]),
            idle_count: AtomicUsize::new(0),
        });
        tokio::spawn(serve_implicit(
            Arc::clone(&device),
            listener,
            ExplicitOptions::default(),
            // Any free port, RPIs from 1 ms to 1 s
            ImplicitOptions {
                io_port: 0,
                packet_interval: 1_000..=1_000_000,
                ..Default::default()
            },
        ));

        let mut stream = TcpStream::connect(address)
            .await
            .expect("the server accepts on its bound port");
        write_packet(&mut stream, &RequestObjectAssembly::new_registration())
            .await
            .expect("the adapter keeps the stream open until Unregister Session");
        read_packet(&mut stream)
            .await
            .expect("the adapter answers Register Session");

        let io_socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("port 0 lets the OS pick a free loopback port");
        Scanner {
            device,
            stream,
            io_socket,
        }
    }

    /// Sends `request` and reads the reply
    async fn request(&mut self, request: &EnIpPacket) -> EnIpPacket {
        write_packet(&mut self.stream, request)
            .await
            .expect("the adapter keeps the stream open until Unregister Session");
        read_packet(&mut self.stream)
            .await
            .expect("the adapter answers Send RR Data")
    }

    /// Sends a Forward_Open whose T->O Socket Address Info names this scanner's I/O socket
    async fn forward_open(&mut self, request: ForwardOpenRequest) -> EnIpPacket {
        let mut packet = RequestObjectAssembly::new_forward_open(SESSION_HANDLE, request);
        let CommandSpecificData::SendRrData(rr_data) = &mut packet.command_specific_data else {
            unreachable!("a Forward_Open is a Send RR Data packet");
        };
        let SocketAddr::V4(local) = self
            .io_socket
            .local_addr()
            .expect("a bound socket has a local address")
        else {
            unreachable!("the socket is bound to an IPv4 address");
        };
        rr_data.socket_addr_info_items.t2o = Some(SocketAddrInfo::from(local));
        self.request(&packet).await
    }

    /// The inputs of the next T->O packet
    async fn next_inputs(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; 512];
        let len = timeout(PACKET_WAIT, self.io_socket.recv(&mut bytes))
            .await
            .expect("the adapter sends inputs every packet interval")
            .expect("receiving on a bound socket succeeds");
        let packet = EnIpIoPacket::read(&mut Cursor::new(&bytes[..len]))
            .expect("the adapter sends I/O packets");
        assert_eq!(
            packet.sequenced_address.connection_id,
            T2O_NETWORK_CONNECTION_ID
        );
        let CipDataOpt::Raw(data) = &packet.connected_data else {
            unreachable!("data read from the wire is raw");
        };
        // The sequence count precedes the inputs
        data[2..].to_vec()
    }

    /// Receives T->O packets until one carries `inputs`
    async fn recv_inputs(&self, inputs: &[u8]) {
        while self.next_inputs().await != inputs {}
    }

    /// Sends an O->T packet of connection `o2t_id` to `o2t_endpoint`, in `run_or_idle` mode
    async fn send_outputs(
        &self,
        o2t_endpoint: SocketAddrV4,
        o2t_id: CipUdint,
        sequence_number: CipUdint,
        cip_sequence_count: u16,
        run_or_idle: RunOrIdle,
        outputs: &[u8],
    ) {
        let packet = EnIpIoPacket::new(
            o2t_id,
            sequence_number,
            CipDataOpt::Typed(Box::new(CipIoData {
                cip_sequence_count: Some(cip_sequence_count),
                output_control_header: Some(
                    OutputControlHeader::builder()
                        .run_or_idle(run_or_idle)
                        .claim_output_ownership(false)
                        .ready_for_ownership_of_outputs(u2::new(0))
                        .build(),
                ),
                data: CipDataOpt::Raw(outputs.to_vec()),
            })),
        );
        let mut bytes = Cursor::new(Vec::new());
        packet
            .write(&mut bytes)
            .expect("writing to memory does not fail");
        self.io_socket
            .send_to(bytes.get_ref(), o2t_endpoint)
            .await
            .expect("UDP sends to the adapter's bound loopback port");
    }
}

/// The 16-bit parameter word of a point-to-point, exclusive-owner, fixed-size direction
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

/// A Forward_Open of assemblies 151/150/100 every 10 ms, named by `connection_serial_number`
fn forward_open_request(connection_serial_number: u16) -> ForwardOpenRequest {
    ForwardOpenRequest {
        priority_time_tick: PriorityTimeTick::builder()
            .tick_time(u4::new(10))
            .priority(false)
            .build(),
        timeout_ticks: 5,
        o2t_network_connection_id: 0,
        t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
        connection_triad: ConnectionTriad {
            connection_serial_number,
            originator_vendor_id: 342,
            originator_serial_number: 0x0001_2345,
        },
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
        o2t_requested_packet_interval: 10_000,
        o2t_network_connection_parameters: parameters(O2T_CONNECTION_SIZE),
        t2o_requested_packet_interval: 10_000,
        t2o_network_connection_parameters: parameters(T2O_CONNECTION_SIZE),
        transport_type_trigger: TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class1)
            .production_trigger(ProductionTrigger::Cyclic)
            .direction(Direction::Client)
            .build(),
        connection_path: CipPath::new_assembly_connection(151, OUTPUT_INSTANCE, INPUT_INSTANCE),
    }
}

/// The Forward_Close of the connection `request` opened
fn forward_close(request: &ForwardOpenRequest) -> EnIpPacket {
    RequestObjectAssembly::new_forward_close(
        SESSION_HANDLE,
        ForwardCloseRequest {
            priority_time_tick: request.priority_time_tick,
            timeout_ticks: request.timeout_ticks,
            connection_triad: request.connection_triad,
            connection_path: request.connection_path.clone(),
        },
    )
}

/// The Forward_Open response a reply carries
fn forward_open_response(reply: &EnIpPacket) -> ForwardOpenResponse {
    let response = reply
        .response()
        .expect("a Send RR Data reply carries a Message Router response");
    assert_eq!(Rejection::from_response(response), None);
    let CipDataOpt::Raw(data) = &response.response_data.data else {
        unreachable!("data read from the wire is raw");
    };
    ForwardOpenResponse::read_le(&mut Cursor::new(data))
        .expect("a successful Forward_Open reply carries its response")
}

/// Where the adapter takes the outputs: the port of the reply's O->T Socket Address Info, on the
/// loopback interface
fn o2t_endpoint(reply: &EnIpPacket) -> SocketAddrV4 {
    let CommandSpecificData::SendRrData(rr_data) = &reply.command_specific_data else {
        unreachable!("a Send RR Data reply");
    };
    let o2t = rr_data
        .socket_addr_info_items
        .o2t
        .expect("the adapter names its I/O port, which is not 2222")
        .socket_address();
    assert_eq!(o2t.ip(), &Ipv4Addr::UNSPECIFIED);
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, o2t.port())
}

/// The refusal a Connection Manager reply carries, and the triad of its data
fn refusal(reply: &EnIpPacket) -> (Rejection, ConnectionTriad) {
    let response = reply
        .response()
        .expect("a Send RR Data reply carries a Message Router response");
    let rejection = Rejection::from_response(response).expect("the request is refused");
    let CipDataOpt::Raw(data) = &response.response_data.data else {
        unreachable!("data read from the wire is raw");
    };
    let triad = ConnectionTriad::read_le(&mut Cursor::new(data))
        .expect("a refused Forward_Open or Forward_Close reply carries the request's triad");
    (rejection, triad)
}

/// The extended status a refused Connection Manager reply carries
fn extended_status(reply: &EnIpPacket) -> Option<ConnectionManagerExtendedStatus> {
    let (rejection, _) = refusal(reply);
    assert_eq!(
        rejection.general_status,
        ResponseStatusCode::ConnectionFailure
    );
    rejection.extended_status()
}

#[tokio::test]
async fn open_echo_and_close() {
    let mut scanner = Scanner::connect().await;
    let request = forward_open_request(1);

    let reply = scanner.forward_open(request.clone()).await;
    let response = forward_open_response(&reply);
    assert_eq!(
        response.t2o_network_connection_id,
        T2O_NETWORK_CONNECTION_ID
    );
    assert_eq!(response.o2t_actual_packet_interval, 10_000);
    let o2t_endpoint = o2t_endpoint(&reply);

    // Inputs from the start, then the outputs echoed
    scanner.recv_inputs(&[0; 4]).await;
    let outputs = [0xde, 0xad, 0xbe, 0xef];
    scanner
        .send_outputs(
            o2t_endpoint,
            response.o2t_network_connection_id,
            1,
            1,
            RunOrIdle::Run,
            &outputs,
        )
        .await;
    scanner.recv_inputs(&outputs).await;

    let reply = scanner.request(&forward_close(&request)).await;
    assert!(
        reply
            .response()
            .expect("a Send RR Data reply carries a Message Router response")
            .is_success()
    );
    timeout(PACKET_WAIT, async {
        while scanner.device.idle_count.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("closing a connection idles its outputs");
    // The Connection Manager takes another connection
    forward_open_response(&scanner.forward_open(forward_open_request(2)).await);
}

#[tokio::test]
async fn idle_outputs_are_not_applied_and_the_device_hears_once() {
    let mut scanner = Scanner::connect().await;
    let mut request = forward_open_request(1);
    // 5.12 s without outputs before the connection times out
    request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
    let reply = scanner.forward_open(request).await;
    let o2t_id = forward_open_response(&reply).o2t_network_connection_id;
    let o2t_endpoint = o2t_endpoint(&reply);

    let running = [1, 1, 1, 1];
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 1, 1, RunOrIdle::Run, &running)
        .await;
    scanner.recv_inputs(&running).await;
    // Idle with the same CIP sequence count, then new idle data
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 2, 1, RunOrIdle::Idle, &running)
        .await;
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 3, 2, RunOrIdle::Idle, &[2, 2, 2, 2])
        .await;
    for _ in 0..5 {
        assert_eq!(scanner.next_inputs().await, running);
    }
    // Run with the same CIP sequence count
    let resumed = [2, 2, 2, 2];
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 4, 2, RunOrIdle::Run, &resumed)
        .await;
    scanner.recv_inputs(&resumed).await;
    assert_eq!(scanner.device.idle_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_connection_is_idle_until_the_scanner_sends_run() {
    let mut scanner = Scanner::connect().await;
    let mut request = forward_open_request(1);
    // 5.12 s without outputs before the connection times out
    request.connection_timeout_multiplier = ConnectionTimeoutMultiplier::X512;
    let reply = scanner.forward_open(request).await;
    let o2t_id = forward_open_response(&reply).o2t_network_connection_id;
    let o2t_endpoint = o2t_endpoint(&reply);

    let running = [1, 1, 1, 1];
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 1, 1, RunOrIdle::Idle, &running)
        .await;
    for _ in 0..5 {
        assert_eq!(scanner.next_inputs().await, [0; 4]);
    }
    // Run with the same CIP sequence count
    scanner
        .send_outputs(o2t_endpoint, o2t_id, 2, 1, RunOrIdle::Run, &running)
        .await;
    scanner.recv_inputs(&running).await;
    assert_eq!(scanner.device.idle_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_second_forward_open_is_refused() {
    let mut scanner = Scanner::connect().await;
    forward_open_response(&scanner.forward_open(forward_open_request(1)).await);

    // Refused as a duplicate whatever else it asks for
    let mut duplicate = forward_open_request(1);
    duplicate.o2t_network_connection_parameters = parameters(O2T_CONNECTION_SIZE + 1);
    assert_eq!(
        extended_status(&scanner.forward_open(duplicate).await),
        Some(ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen)
    );
    assert_eq!(
        extended_status(&scanner.forward_open(forward_open_request(2)).await),
        Some(ConnectionManagerExtendedStatus::OutOfConnections)
    );
}

#[tokio::test]
async fn a_forward_open_the_device_cannot_serve_is_refused() {
    let mut scanner = Scanner::connect().await;

    let mut wrong_size = forward_open_request(1);
    wrong_size.o2t_network_connection_parameters = parameters(O2T_CONNECTION_SIZE + 1);
    let (rejection, triad) = refusal(&scanner.forward_open(wrong_size.clone()).await);
    assert_eq!(
        rejection.extended_status(),
        Some(ConnectionManagerExtendedStatus::InvalidConnectionSize)
    );
    assert_eq!(
        rejection.additional_status.get(1),
        Some(&O2T_CONNECTION_SIZE),
        "the connection size the device supports follows"
    );
    assert_eq!(triad, wrong_size.connection_triad);

    let mut variable_size = forward_open_request(1);
    variable_size.o2t_network_connection_parameters = NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::new(O2T_CONNECTION_SIZE))
            .connection_size_type(ConnectionSizeType::Variable)
            .priority(ConnectionPriority::Scheduled)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    );
    assert_eq!(
        extended_status(&scanner.forward_open(variable_size).await),
        Some(ConnectionManagerExtendedStatus::InvalidNetworkConnectionParameter)
    );

    let mut unknown_assembly = forward_open_request(1);
    unknown_assembly.connection_path = CipPath::new_assembly_connection(151, 1, INPUT_INSTANCE);
    assert_eq!(
        extended_status(&scanner.forward_open(unknown_assembly).await),
        Some(ConnectionManagerExtendedStatus::InvalidProducedOrConsumedApplicationPath)
    );

    let mut change_of_state = forward_open_request(1);
    change_of_state.transport_type_trigger = TransportTypeTrigger::builder()
        .transport_class(TransportClass::Class1)
        .production_trigger(ProductionTrigger::ChangeOfState)
        .direction(Direction::Client)
        .build();
    assert_eq!(
        extended_status(&scanner.forward_open(change_of_state).await),
        Some(ConnectionManagerExtendedStatus::TransportClassAndTriggerCombinationNotSupported)
    );

    let mut too_fast = forward_open_request(1);
    too_fast.t2o_requested_packet_interval = 500;
    assert_eq!(
        extended_status(&scanner.forward_open(too_fast).await),
        Some(ConnectionManagerExtendedStatus::RequestedPacketIntervalNotSupported)
    );
}

#[tokio::test]
async fn a_forward_close_of_no_open_connection_is_refused() {
    let mut scanner = Scanner::connect().await;
    let request = forward_open_request(1);

    let reply = scanner.request(&forward_close(&request)).await;
    let (rejection, triad) = refusal(&reply);
    assert_eq!(
        rejection.extended_status(),
        Some(ConnectionManagerExtendedStatus::TargetConnectionNotFound)
    );
    assert_eq!(triad, request.connection_triad);
}

#[tokio::test(start_paused = true)]
async fn a_connection_without_outputs_times_out_and_another_opens() {
    let mut scanner = Scanner::connect().await;
    forward_open_response(&scanner.forward_open(forward_open_request(1)).await);

    // Past the first output packet timeout
    tokio::time::sleep(Duration::from_secs(11)).await;
    forward_open_response(&scanner.forward_open(forward_open_request(2)).await);
    assert_eq!(scanner.device.idle_count.load(Ordering::SeqCst), 1);
}
