//! Open and Close: the Forward_Open and Forward_Close of the session's class 3 connection.

use tokio::time::Instant;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::parameters::{ConnectionType, Direction};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};

use crate::explicit::connected::{ExplicitConnection, delete_timed_out_connection};
use crate::message_router::{
    class_instance_and_attribute, connection_failure_rejection, message_router_response,
    rejection_response, size_and_types,
};

/// Opens the connection `forward_open` asks for, unless the session has one
pub(crate) fn handle_connected_forward_open(
    class_3_connection: &mut Option<ExplicitConnection>,
    forward_open: ForwardOpenRequest,
) -> MessageRouterResponse {
    let service = forward_open.service_code();
    let triad = forward_open.connection_triad;
    delete_timed_out_connection(class_3_connection);
    // While the session's connection is established, every Forward_Open is refused before it is
    // checked
    if let Some(current) = class_3_connection.as_ref() {
        let extended_status = if current.request.connection_triad == triad {
            ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
        } else {
            ConnectionManagerExtendedStatus::OutOfConnections
        };
        return rejection_response(
            connection_failure_rejection(service, extended_status),
            triad,
        );
    }
    if let Some(extended_status) = unsupported_parameter_status(&forward_open) {
        return rejection_response(
            connection_failure_rejection(service, extended_status),
            triad,
        );
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
    let mut connection = ExplicitConnection {
        request: forward_open,
        response: forward_open_response.clone(),
        deadline: Instant::now(),
        last_cip_sequence_count: None,
    };
    // The timer starts once the connection is established
    connection.restart_inactivity_timer();
    *class_3_connection = Some(connection);
    message_router_response(
        service,
        ResponseStatusCode::Success,
        vec![],
        CipDataOpt::Typed(Box::new(forward_open_response)),
    )
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
    if [
        &forward_open.o2t_network_connection_parameters,
        &forward_open.t2o_network_connection_parameters,
    ]
    .iter()
    .any(|parameters| size_and_types(parameters).2 != ConnectionType::PointToPoint)
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

/// Closes the session's connection when `forward_close` names it; `None` leaves any other
/// connection to the Message Router
pub(crate) fn handle_connected_forward_close(
    class_3_connection: &mut Option<ExplicitConnection>,
    forward_close: &ForwardCloseRequest,
) -> Option<MessageRouterResponse> {
    let triad = forward_close.connection_triad;
    delete_timed_out_connection(class_3_connection);
    // Any other connection is the Message Router's
    if class_3_connection.as_ref()?.request.connection_triad != triad {
        return None;
    }
    *class_3_connection = None;
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
