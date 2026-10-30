//! A Message Router plus class 3 connections.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::time::Instant;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionType, Direction, TransportClass,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::connection_helpers::{
    class_instance_and_attribute, connection_failure_rejection, network_connection_size,
    network_connection_type, o2t_timeout, successful_forward_close_response,
    successful_forward_open_response,
};
use crate::error::{Error, Result};
use crate::explicit::connected::ExplicitConnection;
use crate::message_router::MessageRouter;

/// `message_router`, plus the class 3 Forward_Open, Forward_Close and requests of up to
/// `max_explicit_connections` connections
pub struct ConnectedMessageRouter<M> {
    message_router: Arc<M>,
    max_explicit_connections: usize,
    /// The established connections by O->T network connection id, until they are closed or time out
    established_connections: Mutex<HashMap<CipUdint, ExplicitConnection>>,
}

impl<M: MessageRouter> ConnectedMessageRouter<M> {
    pub fn new(message_router: Arc<M>, max_explicit_connections: usize) -> Self {
        ConnectedMessageRouter {
            message_router,
            max_explicit_connections,
            established_connections: Mutex::new(HashMap::new()),
        }
    }

    /// The established connections, once timed-out ones are dropped
    fn established_connections(&self) -> MutexGuard<'_, HashMap<CipUdint, ExplicitConnection>> {
        let mut established_connections = self
            .established_connections
            .lock()
            .expect("nothing panics while holding the class 3 connections lock");
        let now = Instant::now();
        established_connections.retain(|_, connection| connection.deadline > now);
        established_connections
    }
}

impl<M: MessageRouter> MessageRouter for ConnectedMessageRouter<M> {
    // The server's Get and Set
    fn handle_get_attributes_all(
        &self,
        class: u16,
        instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        self.message_router
            .handle_get_attributes_all(class, instance)
    }

    fn handle_get_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        self.message_router
            .handle_get_attribute_single(class, instance, attribute)
    }

    fn handle_set_attributes_all(
        &self,
        class: u16,
        instance: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        self.message_router
            .handle_set_attributes_all(class, instance, data)
    }

    fn handle_set_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        self.message_router
            .handle_set_attribute_single(class, instance, attribute, data)
    }

    /// Opens the connection `forward_open_request` asks for when it is class 3 and a place is
    /// free: point-to-point both ways, to the Message Router as server, with a packet interval.
    /// Any other Forward_Open is `message_router`'s
    fn handle_forward_open(
        &self,
        forward_open_request: ForwardOpenRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<(ForwardOpenResponse, SocketAddrInfoItems), Rejection> {
        if forward_open_request
            .transport_type_trigger
            .transport_class()
            != TransportClass::Class3
        {
            return self.message_router.handle_forward_open(
                forward_open_request,
                originator_ip,
                t2o_socket_addr_info,
            );
        }

        let service = forward_open_request.service_code();
        let triad = forward_open_request.connection_triad;
        let mut established_connections = self.established_connections();
        // A duplicate, or one too many, is refused before it is checked
        if established_connections
            .values()
            .any(|connection| connection.request.connection_triad == triad)
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen,
            ));
        }
        if established_connections.len() >= self.max_explicit_connections {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::OutOfConnections,
            ));
        }

        // The production trigger is ignored
        if forward_open_request.transport_type_trigger.direction() != Direction::Server {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::TransportClassAndTriggerCombinationNotSupported,
            ));
        }
        if network_connection_type(&forward_open_request.o2t_network_connection_parameters)
            != ConnectionType::PointToPoint
            || network_connection_type(&forward_open_request.t2o_network_connection_parameters)
                != ConnectionType::PointToPoint
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::InvalidNetworkConnectionParameter,
            ));
        }
        if forward_open_request.o2t_requested_packet_interval == 0
            || forward_open_request.t2o_requested_packet_interval == 0
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::RequestedPacketIntervalNotSupported,
            ));
        }
        let message_router = (
            u16::from(MESSAGE_ROUTER_CLASS_ID),
            u16::from(MESSAGE_ROUTER_INSTANCE_ID),
            None,
        );
        if class_instance_and_attribute(&forward_open_request.connection_path)
            != Some(message_router)
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::InvalidSegmentInConnectionPath,
            ));
        }

        let mut forward_open_response = successful_forward_open_response(&forward_open_request);
        // Each established connection has its own O->T id
        while established_connections.contains_key(&forward_open_response.o2t_network_connection_id)
        {
            forward_open_response.o2t_network_connection_id = rand::random();
        }
        tracing::info!(
            o2t_id = format_args!("{:#010x}", forward_open_response.o2t_network_connection_id),
            t2o_id = format_args!("{:#010x}", forward_open_response.t2o_network_connection_id),
            o2t_api_us = forward_open_response.o2t_actual_packet_interval,
            "class 3 connection opened"
        );
        let inactivity_timeout = o2t_timeout(&forward_open_request, &forward_open_response);
        established_connections.insert(
            forward_open_response.o2t_network_connection_id,
            ExplicitConnection {
                request: forward_open_request,
                response: forward_open_response.clone(),
                originator_ip,
                inactivity_timeout,
                // Its inactivity timer starts now
                deadline: Instant::now() + inactivity_timeout,
                last_cip_sequence_count: None,
            },
        );
        Ok((forward_open_response, SocketAddrInfoItems::empty()))
    }

    /// Closes the established connection `forward_close_request` names, when `originator_ip`
    /// opened it; any other Forward_Close is `message_router`'s
    fn handle_forward_close(
        &self,
        forward_close_request: ForwardCloseRequest,
        originator_ip: IpAddr,
    ) -> Result<ForwardCloseResponse, Rejection> {
        let triad = forward_close_request.connection_triad;
        let mut established_connections = self.established_connections();
        let Some((&o2t_network_connection_id, connection)) = established_connections
            .iter()
            .find(|(_, connection)| connection.request.connection_triad == triad)
        else {
            return self
                .message_router
                .handle_forward_close(forward_close_request, originator_ip);
        };
        if connection.originator_ip != originator_ip {
            return Err(Rejection {
                service: ServiceCode::ForwardClose,
                general_status: ResponseStatusCode::PrivilegeViolation,
                additional_status: vec![],
            });
        }
        established_connections.remove(&o2t_network_connection_id);
        tracing::info!(
            connection_serial_number = triad.connection_serial_number,
            "class 3 connection closed"
        );
        Ok(successful_forward_close_response(triad))
    }

    /// Consumes a request on the established connection `connection_id`, when `originator_ip`
    /// opened it, and restarts its inactivity timer; `Error::UnexpectedPacket` when
    /// `cip_sequence_count` repeats the last one. Any other connection is `message_router`'s
    fn accept_connected_request(
        &self,
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
        originator_ip: IpAddr,
    ) -> Result<(CipUdint, u16)> {
        let mut established_connections = self.established_connections();
        let Some(connection) = established_connections.get_mut(&connection_id) else {
            return self.message_router.accept_connected_request(
                connection_id,
                cip_sequence_count,
                originator_ip,
            );
        };
        if connection.originator_ip != originator_ip {
            return Err(Error::UnexpectedPacket(format!(
                "request from another originator ({originator_ip})"
            )));
        }

        connection.deadline = Instant::now() + connection.inactivity_timeout;

        if connection.last_cip_sequence_count == Some(cip_sequence_count) {
            return Err(Error::UnexpectedPacket(format!(
                "repeated CIP sequence count {cip_sequence_count}"
            )));
        }

        connection.last_cip_sequence_count = Some(cip_sequence_count);
        Ok((
            connection.response.t2o_network_connection_id,
            network_connection_size(&connection.request.t2o_network_connection_parameters),
        ))
    }
}
