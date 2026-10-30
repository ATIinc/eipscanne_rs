//! The Message Router of a session: the server's, plus the session's class 3 connection.

use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

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
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::error::{Error, Result};
use crate::explicit::connected::ExplicitConnection;
use crate::message_router::{
    MessageRouter, class_instance_and_attribute, connection_failure_rejection,
    network_connection_size, network_connection_type,
};

/// The server's Message Router, plus the class 3 Forward_Open and Forward_Close of the session's
/// connection when the server accepts them
pub(crate) struct ConnectedMessageRouter<M> {
    message_router: Arc<M>,
    accepted: bool,
    /// The session's connection, until it is closed or times out
    established: Mutex<Option<ExplicitConnection>>,
}

impl<M: MessageRouter> ConnectedMessageRouter<M> {
    pub(crate) fn new(message_router: Arc<M>, accepted: bool) -> Self {
        ConnectedMessageRouter {
            message_router,
            accepted,
            established: Mutex::new(None),
        }
    }

    /// How long the session's connection has until it times out; zero without one
    pub(crate) fn class_3_time_left(&self) -> Duration {
        self.established_connection()
            .as_ref()
            .map_or(Duration::ZERO, |connection| {
                connection
                    .deadline
                    .saturating_duration_since(Instant::now())
            })
    }

    /// Consumes a request on `connection_id` and restarts the inactivity timer; the T->O
    /// connection ID and size of the reply, or `Error::UnexpectedPacket` when the session has no
    /// such connection or `cip_sequence_count` repeats the last one
    pub(crate) fn consume_request(
        &self,
        connection_id: CipUdint,
        cip_sequence_count: CipUint,
    ) -> Result<(CipUdint, u16)> {
        let mut established = self.established_connection();
        let Some(connection) = established.as_mut() else {
            return Err(Error::UnexpectedPacket("no class 3 connection".into()));
        };
        if connection.response.o2t_network_connection_id != connection_id {
            return Err(Error::UnexpectedPacket(format!(
                "request for another connection ({connection_id:#010x})"
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

    /// The session's connection, once a timed-out one is dropped
    fn established_connection(&self) -> MutexGuard<'_, Option<ExplicitConnection>> {
        let mut established = self
            .established
            .lock()
            .expect("nothing panics while holding the class 3 connection lock");
        established.take_if(|connection| connection.deadline <= Instant::now());
        established
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

    /// Opens the session's connection when `forward_open_request` asks for class 3, the server
    /// accepts it and none is established; any other Forward_Open is the server's
    fn handle_forward_open(
        &self,
        forward_open_request: ForwardOpenRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<(ForwardOpenResponse, SocketAddrInfoItems), Rejection> {
        if !self.accepted
            || forward_open_request
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
        let mut established = self.established_connection();
        // While the session's connection is established, every Forward_Open is refused before it
        // is checked
        if let Some(established_connection) = established.as_ref() {
            let refusal_status = if established_connection.request.connection_triad == triad {
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
            } else {
                ConnectionManagerExtendedStatus::OutOfConnections
            };
            return Err(connection_failure_rejection(service, refusal_status));
        }

        let connection = explicit_connection_from_forward_open(forward_open_request)?;
        let forward_open_response = connection.response.clone();
        tracing::info!(
            o2t_id = format_args!("{:#010x}", forward_open_response.o2t_network_connection_id),
            t2o_id = format_args!("{:#010x}", forward_open_response.t2o_network_connection_id),
            o2t_api_us = forward_open_response.o2t_actual_packet_interval,
            "class 3 connection opened"
        );
        *established = Some(connection);
        Ok((forward_open_response, SocketAddrInfoItems::empty()))
    }

    /// Closes the session's connection when `forward_close_request` names it; any other
    /// Forward_Close is the server's
    fn handle_forward_close(
        &self,
        forward_close_request: ForwardCloseRequest,
        originator_ip: IpAddr,
    ) -> Result<ForwardCloseResponse, Rejection> {
        let triad = forward_close_request.connection_triad;
        let closed_connection = self
            .established_connection()
            .take_if(|connection| connection.request.connection_triad == triad);
        if closed_connection.is_none() {
            return self
                .message_router
                .handle_forward_close(forward_close_request, originator_ip);
        }
        tracing::info!(
            connection_serial_number = triad.connection_serial_number,
            "class 3 connection closed"
        );
        Ok(ForwardCloseResponse {
            connection_triad: triad,
            application_reply_size: 0,
            application_reply: vec![],
        })
    }
}

/// The connection `forward_open_request` asks for: point-to-point both ways, to the Message Router
/// as server, with a packet interval
fn explicit_connection_from_forward_open(
    forward_open_request: ForwardOpenRequest,
) -> Result<ExplicitConnection, Rejection> {
    let service = forward_open_request.service_code();

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
    if class_instance_and_attribute(&forward_open_request.connection_path) != Some(message_router) {
        return Err(connection_failure_rejection(
            service,
            ConnectionManagerExtendedStatus::InvalidSegmentInConnectionPath,
        ));
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
    Ok(ExplicitConnection::new(
        forward_open_request,
        forward_open_response,
    ))
}
