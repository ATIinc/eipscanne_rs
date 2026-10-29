//! Open and Close: the Forward_Open and Forward_Close of the session's class 3 connection.

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{ConnectionType, Direction};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};

use crate::explicit::connected::ExplicitConnection;
use crate::message_router::{
    class_instance_and_attribute, connection_failure_rejection, message_router_response,
    network_connection_type, rejection_response,
};

/// The response to `forward_open`; the connection it opens becomes the session's
pub(crate) fn handle_connected_forward_open(
    forward_open: ForwardOpenRequest,
    class_3_connection: &mut Option<ExplicitConnection>,
) -> MessageRouterResponse {
    let service = forward_open.service_code();
    let triad = forward_open.connection_triad;
    match explicit_connection_from_forward_open(forward_open, class_3_connection.as_ref()) {
        Ok(opened_connection) => {
            let forward_open_response = opened_connection.response.clone();
            *class_3_connection = Some(opened_connection);
            message_router_response(
                service,
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(forward_open_response)),
            )
        }
        Err(rejection) => rejection_response(rejection, triad),
    }
}

/// The connection `forward_open` asks for, unless the session's connection is established or it
/// asks for more than a point-to-point connection to the Message Router
fn explicit_connection_from_forward_open(
    forward_open: ForwardOpenRequest,
    class_3_connection: Option<&ExplicitConnection>,
) -> Result<ExplicitConnection, Rejection> {
    let service = forward_open.service_code();
    let triad = forward_open.connection_triad;
    // While the session's connection is established, every Forward_Open is refused before it is
    // checked
    if let Some(established_connection) = class_3_connection {
        let refusal_status = if established_connection.request.connection_triad == triad {
            ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
        } else {
            ConnectionManagerExtendedStatus::OutOfConnections
        };
        return Err(connection_failure_rejection(service, refusal_status));
    }
    if let Some(refusal_status) = unsupported_parameter_status(&forward_open) {
        return Err(connection_failure_rejection(service, refusal_status));
    }

    let forward_open_response = ForwardOpenResponse {
        // Chosen by the target; the T->O id is the scanner's
        o2t_network_connection_id: rand::random(),
        t2o_network_connection_id: forward_open.t2o_network_connection_id,
        connection_triad: triad,
        // Granted as requested
        o2t_actual_packet_interval: forward_open.o2t_requested_packet_interval,
        t2o_actual_packet_interval: forward_open.t2o_requested_packet_interval,
        application_reply_size: 0,
        application_reply: vec![],
    };
    tracing::info!(
        o2t_id = format_args!("{:#010x}", forward_open_response.o2t_network_connection_id),
        t2o_id = format_args!("{:#010x}", forward_open_response.t2o_network_connection_id),
        o2t_api_us = forward_open_response.o2t_actual_packet_interval,
        "class 3 connection opened"
    );
    Ok(ExplicitConnection::new(forward_open, forward_open_response))
}

/// The extended status refusing what `forward_open` asks for beyond a point-to-point connection
/// to the Message Router as server, with a packet interval; `None` when it asks for nothing more
fn unsupported_parameter_status(
    forward_open: &ForwardOpenRequest,
) -> Option<ConnectionManagerExtendedStatus> {
    // The production trigger is ignored
    if forward_open.transport_type_trigger.direction() != Direction::Server {
        return Some(
            ConnectionManagerExtendedStatus::TransportClassAndTriggerCombinationNotSupported,
        );
    }
    if (network_connection_type(&forward_open.o2t_network_connection_parameters)
        != ConnectionType::PointToPoint)
        || (network_connection_type(&forward_open.t2o_network_connection_parameters)
            != ConnectionType::PointToPoint)
    {
        return Some(ConnectionManagerExtendedStatus::InvalidNetworkConnectionParameter);
    }
    if forward_open.o2t_requested_packet_interval == 0
        || forward_open.t2o_requested_packet_interval == 0
    {
        return Some(ConnectionManagerExtendedStatus::RequestedPacketIntervalNotSupported);
    }
    let message_router = (
        u16::from(MESSAGE_ROUTER_CLASS_ID),
        u16::from(MESSAGE_ROUTER_INSTANCE_ID),
        None,
    );
    if class_instance_and_attribute(&forward_open.connection_path) != Some(message_router) {
        return Some(ConnectionManagerExtendedStatus::InvalidSegmentInConnectionPath);
    }
    None
}

/// The response to `forward_close` when it names the session's connection; `None` leaves any other
/// connection to the Message Router
pub(crate) fn handle_connected_forward_close(
    forward_close: &ForwardCloseRequest,
    class_3_connection: Option<&ExplicitConnection>,
) -> Option<MessageRouterResponse> {
    let triad = forward_close.connection_triad;
    // Any other connection is the Message Router's
    if class_3_connection?.request.connection_triad != triad {
        return None;
    }
    tracing::info!(
        connection_serial_number = triad.connection_serial_number,
        "class 3 connection closed"
    );
    Some(message_router_response(
        ServiceCode::ForwardClose,
        ResponseStatusCode::Success,
        vec![],
        CipDataOpt::Typed(Box::new(ForwardCloseResponse {
            connection_triad: triad,
            application_reply_size: 0,
            application_reply: vec![],
        })),
    ))
}
