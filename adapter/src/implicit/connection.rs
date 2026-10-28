//! Opening and closing the one connection, and the task that exchanges its I/O.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use binrw::{BinRead, BinWrite};
use tokio::net::UdpSocket;
use tokio::task::AbortHandle;
use tokio::time::{Instant, interval, sleep_until};

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, ConnectionType, NetworkConnectionParameters, ProductionTrigger,
    RealTimeFormat, TransportClass, connection_size,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::path::{CipPath, LogicalSegmentType};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::device::{Device, ImplicitDevice};
use crate::explicit::{message_router_response, segment_id};
use crate::implicit::o2t::{FIRST_PACKET_GRACE, accept_o2t_packet, recv_io_packet};
use crate::implicit::t2o::{build_t2o_packet, send_io_packet, to_bytes};

/// The I/O socket and the one connection it may carry
pub(crate) struct ConnectionManager {
    io_socket: Arc<UdpSocket>,
    connection: Mutex<Option<OpenConnection>>,
}

impl ConnectionManager {
    pub(crate) fn new(io_socket: UdpSocket) -> Self {
        ConnectionManager {
            io_socket: Arc::new(io_socket),
            connection: Mutex::new(None),
        }
    }

    /// The Socket Address Info items of a reply: the I/O port on a granted Forward_Open, unless it
    /// is 2222
    pub(crate) fn socket_addr_info_items(&self, reply: &CipMessage) -> SocketAddrInfoItems {
        let granted_forward_open = matches!(
            reply,
            CipMessage::Response(response)
                if response.is_success()
                    && matches!(
                        response.service_container.service(),
                        ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen
                    )
        );
        let port = self
            .io_socket
            .local_addr()
            .expect("a bound socket has a local address")
            .port();
        SocketAddrInfoItems {
            o2t: (granted_forward_open && port != ETHERNET_IP_IO_UDP_PORT)
                .then(|| SocketAddrInfo::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))),
            t2o: None,
        }
    }

    /// The response to a Forward_Open or Large_Forward_Open; `t2o_socket_addr_info` is the
    /// request's T->O item
    pub(crate) fn handle_forward_open(
        &self,
        device: &Arc<dyn Device>,
        request: &MessageRouterRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> MessageRouterResponse {
        match self.open(device, request, originator_ip, t2o_socket_addr_info) {
            Ok(response) => message_router_response(
                request.service_container.service(),
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(response)),
            ),
            Err(rejection) => refused(rejection),
        }
    }

    /// The response to a Forward_Close
    pub(crate) fn handle_forward_close(
        &self,
        request: &MessageRouterRequest,
    ) -> MessageRouterResponse {
        match self.close(request) {
            Ok(response) => message_router_response(
                request.service_container.service(),
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(response)),
            ),
            Err(rejection) => refused(rejection),
        }
    }

    /// Opens the one connection, or says why not
    fn open(
        &self,
        device: &Arc<dyn Device>,
        request: &MessageRouterRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<ForwardOpenResponse, Rejection> {
        let service = request.service_container.service();
        let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
            unreachable!("data read from the wire is always raw");
        };
        let large = service == ServiceCode::LargeForwardOpen;
        let forward_open = ForwardOpenRequest::read_le_args(&mut Cursor::new(data), (large,))
            .map_err(|error| refusal(service, decode_status(&error), vec![]))?;
        let refuse = |extended_status| connection_failure(service, extended_status);

        // 1. Class 1, cyclic, point-to-point both ways, packets due at some interval
        let trigger = forward_open.transport_type_trigger;
        if trigger.transport_class() != TransportClass::Class1
            || trigger.production_trigger() != ProductionTrigger::Cyclic
        {
            return Err(refuse(
                ConnectionManagerExtendedStatus::TransportClassAndTriggerCombinationNotSupported,
            ));
        }
        let o2t_parameters = &forward_open.o2t_network_connection_parameters;
        let t2o_parameters = &forward_open.t2o_network_connection_parameters;
        if [o2t_parameters, t2o_parameters]
            .iter()
            .any(|parameters| connection_parameters(parameters).2 != ConnectionType::PointToPoint)
        {
            return Err(refuse(
                ConnectionManagerExtendedStatus::InvalidNetworkConnectionParameter,
            ));
        }
        if forward_open.o2t_requested_packet_interval == 0
            || forward_open.t2o_requested_packet_interval == 0
        {
            return Err(refuse(
                ConnectionManagerExtendedStatus::RequestedPacketIntervalNotSupported,
            ));
        }

        // 2. Assemblies the device exchanges, at the sizes asked for
        let Some((output_instance, input_instance)) =
            assembly_connection_points(&forward_open.connection_path)
        else {
            return Err(refuse(
                ConnectionManagerExtendedStatus::InvalidSegmentInConnectionPath,
            ));
        };
        let (Some((output_size, o2t_real_time_format)), Some((input_size, t2o_real_time_format))) = (
            implicit_device(device).implicit_assembly(output_instance),
            implicit_device(device).implicit_assembly(input_instance),
        ) else {
            return Err(refuse(
                ConnectionManagerExtendedStatus::InvalidProducedOrConsumedApplicationPath,
            ));
        };
        if !connection_size_fits(o2t_parameters, output_size, o2t_real_time_format)
            || !connection_size_fits(t2o_parameters, input_size, t2o_real_time_format)
        {
            return Err(refuse(
                ConnectionManagerExtendedStatus::InvalidConnectionSize,
            ));
        }

        // 3. No connection is open
        let mut open_connection = self
            .connection
            .lock()
            .expect("nothing panics while holding the connection lock");
        if let Some(current) = open_connection
            .as_ref()
            .filter(|current| !current.task.is_finished())
        {
            return Err(refuse(
                if current.connection_triad == forward_open.connection_triad {
                    ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
                } else {
                    ConnectionManagerExtendedStatus::OutOfConnections
                },
            ));
        }

        let response = ForwardOpenResponse {
            o2t_network_connection_id: rand::random(),
            t2o_network_connection_id: forward_open.t2o_network_connection_id,
            connection_triad: forward_open.connection_triad,
            o2t_actual_packet_interval: forward_open.o2t_requested_packet_interval,
            t2o_actual_packet_interval: forward_open.t2o_requested_packet_interval,
            application_reply_size: 0,
            application_reply: vec![],
        };
        let connection = ImplicitConnection {
            request: forward_open,
            response: response.clone(),
            o2t_real_time_format,
            t2o_real_time_format,
            originator_ip,
            t2o_endpoint: t2o_endpoint(t2o_socket_addr_info, originator_ip),
            output_instance,
            input_instance,
        };
        tracing::info!(
            o2t_id = format_args!("{:#010x}", response.o2t_network_connection_id),
            t2o_id = format_args!("{:#010x}", response.t2o_network_connection_id),
            o2t_api_us = response.o2t_actual_packet_interval,
            t2o_api_us = response.t2o_actual_packet_interval,
            output_instance,
            input_instance,
            t2o_endpoint = %connection.t2o_endpoint,
            "class 1 connection opened"
        );
        let task = tokio::spawn(exchange_io(
            connection,
            Arc::clone(&self.io_socket),
            Arc::clone(device),
        ))
        .abort_handle();
        *open_connection = Some(OpenConnection {
            connection_triad: response.connection_triad,
            task,
        });
        Ok(response)
    }

    /// Closes the open connection the Forward_Close's triad names, or says why not
    fn close(&self, request: &MessageRouterRequest) -> Result<ForwardCloseResponse, Rejection> {
        let service = request.service_container.service();
        let CipDataOpt::Raw(data) = &request.request_data.additional_data else {
            unreachable!("data read from the wire is always raw");
        };
        let forward_close = ForwardCloseRequest::read_le(&mut Cursor::new(data))
            .map_err(|error| refusal(service, decode_status(&error), vec![]))?;

        let mut open_connection = self
            .connection
            .lock()
            .expect("nothing panics while holding the connection lock");
        match open_connection.take() {
            Some(current)
                if current.connection_triad == forward_close.connection_triad
                    && !current.task.is_finished() =>
            {
                current.task.abort();
                tracing::info!(
                    connection_serial_number =
                        forward_close.connection_triad.connection_serial_number,
                    "connection closed"
                );
                Ok(ForwardCloseResponse {
                    connection_triad: forward_close.connection_triad,
                    application_reply_size: 0,
                    application_reply: vec![],
                })
            }
            other => {
                *open_connection = other;
                Err(connection_failure(
                    service,
                    ConnectionManagerExtendedStatus::TargetConnectionNotFound,
                ))
            }
        }
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        if let Ok(Some(open_connection)) = self.connection.get_mut().map(Option::take) {
            open_connection.task.abort();
        }
    }
}

/// The open connection: its triad names it in a Forward_Close; it is open while its task runs
struct OpenConnection {
    connection_triad: ConnectionTriad,
    task: AbortHandle,
}

/// An open connection: the Forward_Open received, the reply sent, each direction's real-time
/// format (from the device) and where the I/O goes
#[derive(Debug)]
pub(crate) struct ImplicitConnection {
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
    /// How the outputs signal run/idle
    pub o2t_real_time_format: RealTimeFormat,
    /// How the inputs signal run/idle
    pub t2o_real_time_format: RealTimeFormat,
    /// The scanner's IP address: where the outputs come from
    pub originator_ip: IpAddr,
    /// Where the inputs are sent
    pub t2o_endpoint: SocketAddr,
    /// The O->T connection point
    pub output_instance: u16,
    /// The T->O connection point
    pub input_instance: u16,
}

/// Exchanges the I/O of `connection` until its outputs stop for longer than its timeout; a
/// Forward_Close aborts it
async fn exchange_io(
    connection: ImplicitConnection,
    io_socket: Arc<UdpSocket>,
    device: Arc<dyn Device>,
) {
    let implicit_device = implicit_device(&device);
    // The inputs: the encapsulation sequence number starts at random and advances every packet;
    // the CIP sequence count advances when the inputs change
    let mut production = interval(Duration::from_micros(u64::from(
        connection.response.t2o_actual_packet_interval,
    )));
    let mut encapsulation_sequence_number: CipUdint = rand::random();
    let mut cip_sequence_count: CipUint = 0;
    let mut last_inputs: Option<Vec<u8>> = None;

    // The outputs: the last accepted number, and the deadline every accepted packet moves
    let timeout = output_timeout(&connection);
    let mut last_sequence_number: Option<CipUdint> = None;
    let mut deadline = Instant::now() + FIRST_PACKET_GRACE.max(timeout);

    loop {
        tokio::select! {
            _ = production.tick() => {
                let inputs = match to_bytes(&implicit_device.produce_inputs(connection.input_instance)) {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        tracing::warn!(%error, "inputs not sent");
                        continue;
                    }
                };
                if last_inputs.as_ref() != Some(&inputs) {
                    cip_sequence_count = cip_sequence_count.wrapping_add(1);
                    last_inputs = Some(inputs.clone());
                }
                let packet = match build_t2o_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    inputs,
                ) {
                    Ok(packet) => packet,
                    Err(error) => {
                        tracing::warn!(%error, "inputs not sent");
                        continue;
                    }
                };
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                if let Err(error) = send_io_packet(&io_socket, &packet, connection.t2o_endpoint).await {
                    tracing::warn!(%error, "inputs not sent");
                }
            }

            received = recv_io_packet(&io_socket) => {
                let accepted = received.and_then(|(packet, from)| {
                    accept_o2t_packet(&connection, last_sequence_number, &packet, from)
                });
                let (address, outputs) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::debug!(%error, "discarded an output packet");
                        continue;
                    }
                };
                last_sequence_number = Some(address.encapsulation_sequence_number);
                deadline = Instant::now() + timeout;
                // Idle outputs keep the connection open but are not applied
                let run = outputs.run_idle_header.is_none_or(|header| header.run_idle());
                // Read from the wire, so the data is raw bytes
                if let CipDataOpt::Raw(data) = &outputs.data
                    && run
                    && !data.is_empty()
                {
                    implicit_device.consume_outputs(connection.output_instance, data);
                }
            }

            _ = sleep_until(deadline) => {
                tracing::warn!("the connection timed out: no output packet in time");
                return;
            }
        }
    }
}

/// The implicit side of a device that has a Connection Manager
fn implicit_device(device: &Arc<dyn Device>) -> &dyn ImplicitDevice {
    device
        .as_implicit_device()
        .expect("only a device with implicit messaging has a Connection Manager")
}

/// How long the outputs may stop before the connection times out: the timeout multiplier times
/// the O->T actual packet interval
fn output_timeout(connection: &ImplicitConnection) -> Duration {
    // The timeout multiplier as a factor: 4, 8, ... 512
    let multiplier: u32 = connection
        .request
        .connection_timeout_multiplier
        .multiplier();
    // x512 on a 10 s interval overflows 32-bit microseconds
    Duration::from_micros(
        u64::from(multiplier) * u64::from(connection.response.o2t_actual_packet_interval),
    )
}

/// Where the inputs go: the request's T->O Socket Address Info address (`0.0.0.0` meaning the
/// scanner), or the scanner on the I/O port
fn t2o_endpoint(t2o_socket_addr_info: Option<SocketAddrInfo>, originator_ip: IpAddr) -> SocketAddr {
    match t2o_socket_addr_info.map(|info| info.socket_address()) {
        Some(address) if address.ip().is_unspecified() => {
            SocketAddr::new(originator_ip, address.port())
        }
        Some(address) => SocketAddr::V4(address),
        None => SocketAddr::new(originator_ip, ETHERNET_IP_IO_UDP_PORT),
    }
}

/// The O->T and T->O connection points of an assembly connection path: class, configuration
/// instance, then the two connection points
fn assembly_connection_points(path: &CipPath) -> Option<(u16, u16)> {
    let [class, configuration, o2t, t2o] = path.segments.as_slice() else {
        return None;
    };
    if segment_id(class, LogicalSegmentType::ClassId)? != u16::from(ASSEMBLY_CLASS_ID) {
        return None;
    }
    segment_id(configuration, LogicalSegmentType::InstanceId)?;
    Some((
        segment_id(o2t, LogicalSegmentType::ConnectionPoint)?,
        segment_id(t2o, LogicalSegmentType::ConnectionPoint)?,
    ))
}

/// The connection size, its type and the connection type of a direction
fn connection_parameters(
    parameters: &NetworkConnectionParameters,
) -> (u16, ConnectionSizeType, ConnectionType) {
    match parameters {
        NetworkConnectionParameters::Standard(parameters) => (
            parameters.connection_size().value(),
            parameters.connection_size_type(),
            parameters.connection_type(),
        ),
        NetworkConnectionParameters::Large(parameters) => (
            parameters.connection_size(),
            parameters.connection_size_type(),
            parameters.connection_type(),
        ),
    }
}

/// Application data bytes per packet of a direction: its connection size minus the sequence count
/// and real-time header, at least 0
pub(crate) fn data_size(
    parameters: &NetworkConnectionParameters,
    real_time_format: RealTimeFormat,
) -> (u16, ConnectionSizeType) {
    let (size, size_type, _) = connection_parameters(parameters);
    let overhead: u16 = connection_size(0, TransportClass::Class1, real_time_format);
    (size.saturating_sub(overhead), size_type)
}

/// Whether `data_len` bytes fit a direction of `data_size` bytes: exactly for a fixed size, at
/// most for a variable one
pub(crate) fn data_len_matches_connection(
    data_len: usize,
    data_size: u16,
    size_type: ConnectionSizeType,
) -> bool {
    match size_type {
        ConnectionSizeType::Fixed => data_len == usize::from(data_size),
        ConnectionSizeType::Variable => data_len <= usize::from(data_size),
    }
}

/// Whether a direction asked for carries an assembly of `assembly_size` bytes: exactly for a fixed
/// size, at least for a variable one
fn connection_size_fits(
    parameters: &NetworkConnectionParameters,
    assembly_size: u16,
    real_time_format: RealTimeFormat,
) -> bool {
    let (size, size_type, _) = connection_parameters(parameters);
    let needed: u16 = connection_size(assembly_size, TransportClass::Class1, real_time_format);
    match size_type {
        ConnectionSizeType::Fixed => size == needed,
        ConnectionSizeType::Variable => size >= needed,
    }
}

/// The General Status of request data that does not decode
fn decode_status(error: &binrw::Error) -> ResponseStatusCode {
    if error.is_eof() {
        ResponseStatusCode::NotEnoughData
    } else {
        ResponseStatusCode::InvalidParameterValue
    }
}

/// A Connection Manager refusal: Connection Failure with `extended_status` as the Additional Status
fn connection_failure(
    service: ServiceCode,
    extended_status: ConnectionManagerExtendedStatus,
) -> Rejection {
    let mut bytes = Cursor::new(Vec::new());
    extended_status
        .write_le(&mut bytes)
        .expect("writing to memory does not fail");
    let word = CipUint::from_le_bytes(
        bytes
            .into_inner()
            .try_into()
            .expect("an extended status is one word"),
    );
    refusal(service, ResponseStatusCode::ConnectionFailure, vec![word])
}

fn refusal(
    service: ServiceCode,
    general_status: ResponseStatusCode,
    additional_status: Vec<CipUint>,
) -> Rejection {
    let rejection = Rejection {
        service,
        general_status,
        additional_status,
    };
    tracing::warn!(?service, ?general_status, extended_status = ?rejection.extended_status(), "refused a Connection Manager request");
    rejection
}

/// The response carrying `rejection`
fn refused(rejection: Rejection) -> MessageRouterResponse {
    message_router_response(
        rejection.service,
        rejection.general_status,
        rejection.additional_status,
        CipDataOpt::Raw(vec![]),
    )
}
