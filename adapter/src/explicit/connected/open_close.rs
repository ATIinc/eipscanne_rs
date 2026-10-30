//! Open and Close: the Forward_Open and Forward_Close of the session's class 3 connection.

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{ConnectionType, Direction};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::response::Rejection;
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};

use crate::explicit::connected::{Class3Connection, ExplicitConnection};
use crate::message_router::{
    class_instance_and_attribute, connection_failure_rejection, network_connection_type,
};

/// Opens the connection `forward_open_request` asks for as the session's, unless one is
/// established
pub(crate) fn handle_connected_forward_open(
    forward_open_request: ForwardOpenRequest,
    class_3_connection: &mut Class3Connection,
) -> Result<ForwardOpenResponse, Rejection> {
    let service = forward_open_request.service_code();
    let triad = forward_open_request.connection_triad;
    // While the session's connection is established, every Forward_Open is refused before it is
    // checked
    if let Some(established_connection) = &class_3_connection.established {
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
    class_3_connection.established = Some(connection);
    Ok(forward_open_response)
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

/// Closes the session's connection when `forward_close_request` names it; `None` leaves any other
/// connection to the Message Router
pub(crate) fn handle_connected_forward_close(
    forward_close_request: &ForwardCloseRequest,
    class_3_connection: &mut Class3Connection,
) -> Option<ForwardCloseResponse> {
    let triad = forward_close_request.connection_triad;
    // Any other connection is the Message Router's
    class_3_connection
        .established
        .take_if(|connection| connection.request.connection_triad == triad)?;
    tracing::info!(
        connection_serial_number = triad.connection_serial_number,
        "class 3 connection closed"
    );
    Some(ForwardCloseResponse {
        connection_triad: triad,
        application_reply_size: 0,
        application_reply: vec![],
    })
}
