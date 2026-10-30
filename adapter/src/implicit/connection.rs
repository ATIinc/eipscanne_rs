//! A class 1 connection: what its I/O exchange task runs on, what the Message Router keeps while it
//! is open, and the Forward_Open checks that make it.

use std::net::{IpAddr, SocketAddr};
use std::ops::RangeInclusive;

use tokio::sync::mpsc::Sender;
use tokio::task::AbortHandle;

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionSizeType, ConnectionType, ProductionTrigger, RealTimeFormat, TransportClass,
    connection_size,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::response::Rejection;
use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::path::{CipPath, LogicalSegmentType};
use eipscanne_rs::cip::types::CipInt;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::io_packet::EnIpIoPacket;
use eipscanne_rs::eip::socket_addr::SocketAddrInfo;

use crate::implicit::ImplicitDevice;
use crate::message_router::{
    connection_failure_rejection, network_connection_size, network_connection_size_type,
    network_connection_type, segment_id,
};

/// What the I/O exchange task runs on: the Forward_Open received, the reply sent, where the I/O
/// goes and each direction's assembly (from the device)
#[derive(Debug)]
pub(crate) struct ImplicitConnection {
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
    /// The scanner's IP address: where the outputs come from
    pub originator_ip: IpAddr,
    /// Where the inputs are sent
    pub t2o_endpoint: SocketAddr,
    pub o2t: ConnectionPoint,
    pub t2o: ConnectionPoint,
}

/// One direction's assembly: its connection point, data size and run/idle signalling
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConnectionPoint {
    pub instance: u16,
    /// Data bytes of each packet
    pub size: u16,
    /// How the packets signal run/idle
    pub real_time_format: RealTimeFormat,
}

/// What the Message Router keeps of a connection: the triad a Forward_Close names, the IP address
/// that opened it, and its I/O exchange task, which dropping it stops
pub(crate) struct OpenConnection {
    pub connection_triad: ConnectionTriad,
    pub originator_ip: IpAddr,
    pub output_instance: u16,
    /// Where [`route_outputs`](crate::implicit::o2t::route_outputs) hands its O->T packets
    pub output_packet_sender: Sender<(EnIpIoPacket, SocketAddr)>,
    pub io_exchange_task: AbortHandle,
}

impl Drop for OpenConnection {
    fn drop(&mut self) {
        self.io_exchange_task.abort();
    }
}

/// `sin_family` of an IPv4 Socket Address Info
const AF_INET: CipInt = 2;

/// The connection `forward_open_request` asks for: class 1, cyclic, point-to-point and fixed size
/// both ways, at RPIs within `packet_interval`, between assemblies the device exchanges at those
/// sizes
pub(crate) fn connection_from_forward_open(
    device: &impl ImplicitDevice,
    forward_open_request: ForwardOpenRequest,
    packet_interval: &RangeInclusive<u32>,
    originator_ip: IpAddr,
    t2o_socket_addr_info: Option<SocketAddrInfo>,
) -> Result<ImplicitConnection, Rejection> {
    let service = forward_open_request.service_code();

    let trigger = forward_open_request.transport_type_trigger;
    if trigger.transport_class() != TransportClass::Class1
        || trigger.production_trigger() != ProductionTrigger::Cyclic
    {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::TransportClassAndTriggerCombinationNotSupported,
        ));
    }
    let o2t_parameters = &forward_open_request.o2t_network_connection_parameters;
    let t2o_parameters = &forward_open_request.t2o_network_connection_parameters;
    if network_connection_size_type(o2t_parameters) != ConnectionSizeType::Fixed
        || network_connection_size_type(t2o_parameters) != ConnectionSizeType::Fixed
        || network_connection_type(o2t_parameters) != ConnectionType::PointToPoint
        || network_connection_type(t2o_parameters) != ConnectionType::PointToPoint
    {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::InvalidNetworkConnectionParameter,
        ));
    }
    // Never 0, whatever `packet_interval` allows
    if forward_open_request.o2t_requested_packet_interval == 0
        || forward_open_request.t2o_requested_packet_interval == 0
        || !packet_interval.contains(&forward_open_request.o2t_requested_packet_interval)
        || !packet_interval.contains(&forward_open_request.t2o_requested_packet_interval)
    {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::RequestedPacketIntervalNotSupported,
        ));
    }
    if t2o_socket_addr_info.is_some_and(|info| info.sin_family != AF_INET) {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::ParameterErrorInUnconnectedRequestService,
        ));
    }

    let Some((output_instance, input_instance)) =
        output_and_input_instances(&forward_open_request.connection_path)
    else {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::InvalidSegmentInConnectionPath,
        ));
    };
    let (Some((output_size, o2t_real_time_format)), Some((input_size, t2o_real_time_format))) = (
        device.assembly_layout(output_instance),
        device.assembly_layout(input_instance),
    ) else {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::InvalidProducedOrConsumedApplicationPath,
        ));
    };
    let o2t = ConnectionPoint {
        instance: output_instance,
        size: output_size,
        real_time_format: o2t_real_time_format,
    };
    let t2o = ConnectionPoint {
        instance: input_instance,
        size: input_size,
        real_time_format: t2o_real_time_format,
    };
    // Each size is its assembly's plus headers; a refusal names the size that fits
    for (parameters, connection_point) in [(o2t_parameters, o2t), (t2o_parameters, t2o)] {
        let supported_size = connection_size(
            connection_point.size,
            TransportClass::Class1,
            connection_point.real_time_format,
        );
        if network_connection_size(parameters) != supported_size {
            let mut rejection = connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::InvalidConnectionSize,
            );
            rejection.additional_status.push(supported_size);
            return Err(rejection);
        }
    }

    let forward_open_response = ForwardOpenResponse {
        // Chosen by the target; the T->O id is the scanner's
        o2t_network_connection_id: rand::random(),
        t2o_network_connection_id: forward_open_request.t2o_network_connection_id,
        connection_triad: forward_open_request.connection_triad,
        // Granted as requested
        o2t_actual_packet_interval: forward_open_request.o2t_requested_packet_interval,
        t2o_actual_packet_interval: forward_open_request.t2o_requested_packet_interval,
        application_reply_size: 0,
        application_reply: vec![],
    };
    Ok(ImplicitConnection {
        request: forward_open_request,
        response: forward_open_response,
        originator_ip,
        // The scanner, on the port its T->O item names; the item's address is ignored
        t2o_endpoint: SocketAddr::new(
            originator_ip,
            t2o_socket_addr_info.map_or(ETHERNET_IP_IO_UDP_PORT, |info| info.sin_port),
        ),
        o2t,
        t2o,
    })
}

/// The O->T and T->O connection points of an assembly connection path: class, configuration
/// instance, then the two connection points
fn output_and_input_instances(path: &CipPath) -> Option<(u16, u16)> {
    let [class, configuration, output, input] = path.segments.as_slice() else {
        return None;
    };
    if segment_id(class, LogicalSegmentType::ClassId)? != u16::from(ASSEMBLY_CLASS_ID) {
        return None;
    }
    segment_id(configuration, LogicalSegmentType::InstanceId)?;
    Some((
        segment_id(output, LogicalSegmentType::ConnectionPoint)?,
        segment_id(input, LogicalSegmentType::ConnectionPoint)?,
    ))
}
