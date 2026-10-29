//! One class 3 connection opened and used on the loopback interface.

use std::io::Cursor;
use std::net::Ipv4Addr;
use std::sync::Arc;

use bilge::prelude::{u4, u9};
use binrw::BinRead;
use tokio::net::{TcpListener, TcpStream};

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RedundantOwner,
    StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{
    IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID, MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID,
};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand};
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use adapter::message_router::MessageRouter;
use adapter::packet::{read_packet, write_packet};
use adapter::server::serve;

/// The handle of the first session a server grants
const SESSION_HANDLE: CipUdint = 1;
const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5679;
/// Bytes per message each way, with the sequence count
const CONNECTION_SIZE: u16 = 504;

/// Answers Get_Attributes_All with one byte
struct SampleDevice {
    accepts_explicit_connections: bool,
}

impl MessageRouter for SampleDevice {
    fn handle_get_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        Ok(CipDataOpt::Raw(vec![0x2a]))
    }

    fn accepts_explicit_connections(&self) -> bool {
        self.accepts_explicit_connections
    }
}

/// A registered session to a `SampleDevice`
struct Scanner {
    stream: TcpStream,
}

impl Scanner {
    /// Serves a `SampleDevice` on the loopback interface and registers a session with it
    async fn connect(accepts_explicit_connections: bool) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("port 0 lets the OS pick a free loopback port");
        let address = listener
            .local_addr()
            .expect("a bound listener has a local address");
        let device = SampleDevice {
            accepts_explicit_connections,
        };
        tokio::spawn(serve(Arc::new(device), listener));

        let mut scanner = Scanner {
            stream: TcpStream::connect(address)
                .await
                .expect("the server accepts on its bound port"),
        };
        scanner
            .request(&RequestObjectAssembly::new_registration())
            .await;
        scanner
    }

    /// Sends `request` without waiting for a reply
    async fn send(&mut self, request: &EnIpPacket) {
        write_packet(&mut self.stream, request)
            .await
            .expect("the adapter keeps the stream open until Unregister Session");
    }

    /// Sends `request` and reads the next packet
    async fn request(&mut self, request: &EnIpPacket) -> EnIpPacket {
        self.send(request).await;
        read_packet(&mut self.stream)
            .await
            .expect("the adapter answers the request")
    }

    /// Opens a connection with `request` and returns its O->T ID
    async fn forward_open(&mut self, request: ForwardOpenRequest) -> CipUdint {
        let reply = self
            .request(&RequestObjectAssembly::new_forward_open(
                SESSION_HANDLE,
                request,
            ))
            .await;
        let response = response(&reply);
        assert_eq!(Rejection::from_response(response), None);
        let CipDataOpt::Raw(data) = &response.response_data.data else {
            unreachable!("data read from the wire is raw");
        };
        let response = ForwardOpenResponse::read_le(&mut Cursor::new(data))
            .expect("a successful Forward_Open reply carries its response");
        assert_eq!(
            response.t2o_network_connection_id,
            T2O_NETWORK_CONNECTION_ID
        );
        response.o2t_network_connection_id
    }

    /// Sends a connected Get_Attributes_All of the Identity object and reads the next packet
    async fn connected_request(
        &mut self,
        o2t_id: CipUdint,
        cip_sequence_count: CipUint,
    ) -> EnIpPacket {
        self.request(&connected_request(o2t_id, cip_sequence_count))
            .await
    }
}

/// A Send Unit Data packet carrying Get_Attributes_All of the Identity object on `o2t_id`
fn connected_request(o2t_id: CipUdint, cip_sequence_count: CipUint) -> EnIpPacket {
    EnIpPacket::new_send_unit_data(
        SESSION_HANDLE,
        o2t_id,
        cip_sequence_count,
        MessageRouterRequest::new(
            ServiceCode::GetAttributeAll,
            CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
        ),
    )
}

/// The 16-bit parameter word of a point-to-point, variable-size direction
fn parameters(connection_size: u16) -> NetworkConnectionParameters {
    NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::new(connection_size))
            .connection_size_type(ConnectionSizeType::Variable)
            .priority(ConnectionPriority::Low)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    )
}

/// A class 3 Forward_Open to the Message Router that times out after 4 s without a request, named
/// by `connection_serial_number`
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
        o2t_requested_packet_interval: 1_000_000,
        o2t_network_connection_parameters: parameters(CONNECTION_SIZE),
        t2o_requested_packet_interval: 1_000_000,
        t2o_network_connection_parameters: parameters(CONNECTION_SIZE),
        transport_type_trigger: TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class3)
            .production_trigger(ProductionTrigger::ApplicationObject)
            .direction(Direction::Server)
            .build(),
        connection_path: CipPath::new_u8(MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID),
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

/// The Message Router response a reply carries
fn response(reply: &EnIpPacket) -> &MessageRouterResponse {
    reply
        .response()
        .expect("a Send RR Data or Send Unit Data reply carries a Message Router response")
}

/// Checks that `reply` answers the connected request with `cip_sequence_count`, and returns its data
fn connected_reply_data(reply: &EnIpPacket, cip_sequence_count: CipUint) -> Vec<u8> {
    let CommandSpecificData::SendUnitData(unit_data) = &reply.command_specific_data else {
        panic!(
            "expected a Send Unit Data reply, got {:?}",
            reply.header.command
        );
    };
    assert_eq!(unit_data.connection_id, T2O_NETWORK_CONNECTION_ID);
    assert_eq!(unit_data.cip_sequence_count, cip_sequence_count);
    let response = response(reply);
    assert_eq!(response.response_data.status, ResponseStatusCode::Success);
    let CipDataOpt::Raw(data) = &response.response_data.data else {
        unreachable!("data read from the wire is raw");
    };
    data.clone()
}

#[tokio::test]
async fn open_request_and_close() {
    let mut scanner = Scanner::connect(true).await;
    let request = forward_open_request(1);

    let o2t_id = scanner.forward_open(request.clone()).await;
    let reply = scanner.connected_request(o2t_id, 1).await;
    assert_eq!(connected_reply_data(&reply, 1), [0x2a]);

    let reply = scanner.request(&forward_close(&request)).await;
    assert!(response(&reply).is_success());
    // A request on the closed connection is discarded, so the next packet answers Send RR Data
    scanner.send(&connected_request(o2t_id, 2)).await;
    let reply = scanner
        .request(&RequestObjectAssembly::new_identity(SESSION_HANDLE))
        .await;
    assert_eq!(reply.header.command, EnIpCommand::SendRrData);
    // The session takes another connection
    scanner.forward_open(forward_open_request(2)).await;
}

#[tokio::test]
async fn a_device_that_does_not_accept_explicit_connections_refuses_class_3() {
    let mut scanner = Scanner::connect(false).await;

    let reply = scanner
        .request(&RequestObjectAssembly::new_forward_open(
            SESSION_HANDLE,
            forward_open_request(1),
        ))
        .await;
    assert_eq!(
        response(&reply).response_data.status,
        ResponseStatusCode::ServiceNotSupported
    );
}
