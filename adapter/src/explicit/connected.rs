//! Connected explicit messaging: one class 3 connection per session, to the Message Router.
//!
//! ```text
//! Stage    What happens on the wire                                         Function
//! -------  ---------------------------------------------------------------  -------------------------
//! Open     SendRRData(Forward_Open) -> the reply                            handle_connection_request
//! Request  SendUnitData(request) on O->T -> SendUnitData(response) on T->O  handle_send_unit_data
//! Close    SendRRData(Forward_Close) -> the reply                           handle_connection_request
//! ```
//!
//! Only a [`MessageRouter`] that accepts explicit connections opens one. A request whose CIP
//! sequence count repeats the last one is discarded; the connection is deleted when no request
//! comes within its timeout, and when its session ends.
//!
//! [`MessageRouter`]: crate::message_router::MessageRouter

use std::io::Cursor;
use std::net::IpAddr;
use std::time::Duration;

use binrw::{BinRead, BinWrite};
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
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::command::{CommandSpecificData, UnitPacketData};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::{class_instance_and_attribute, handle_request, size_and_types};
use crate::message_router::{
    MessageRouter, connection_failure, message_router_response, rejection_response,
};

/// Bytes of the CIP sequence count in front of each message
const CIP_SEQUENCE_COUNT_LEN: usize = 2;

/// An established class 3 connection: the Forward_Open received and the reply sent
#[derive(Debug)]
pub(crate) struct ExplicitConnection {
    request: ForwardOpenRequest,
    response: ForwardOpenResponse,
    /// When it times out unless a request comes first
    deadline: Instant,
    /// The CIP sequence count of the last request; `None` before the first
    last_cip_sequence_count: Option<CipUint>,
}

impl ExplicitConnection {
    /// Restarts the inactivity timer: the timeout multiplier times the O->T actual packet interval
    fn restart_inactivity_timer(&mut self) {
        self.deadline = Instant::now()
            + Duration::from_micros(
                u64::from(self.request.connection_timeout_multiplier.multiplier())
                    * u64::from(self.response.o2t_actual_packet_interval),
            );
    }

    /// The Send Unit Data reply data carrying `response` to the request with `cip_sequence_count`;
    /// Reply Data Too Large when `response` does not fit the T->O connection size
    fn reply_unit_data(
        &self,
        cip_sequence_count: CipUint,
        response: MessageRouterResponse,
    ) -> UnitPacketData {
        let mut bytes = Cursor::new(Vec::new());
        response
            .write_le(&mut bytes)
            .expect("a Message Router response writes to memory");
        let (t2o_connection_size, _, _) =
            size_and_types(&self.request.t2o_network_connection_parameters);
        let response =
            if CIP_SEQUENCE_COUNT_LEN + bytes.get_ref().len() > usize::from(t2o_connection_size) {
                let service = response.service_container.service();
                tracing::warn!(?service, t2o_connection_size, "reply data too large");
                message_router_response(
                    service,
                    ResponseStatusCode::ReplyDataTooLarge,
                    vec![],
                    CipDataOpt::Raw(vec![]),
                )
            } else {
                response
            };
        UnitPacketData::new_connected(
            self.response.t2o_network_connection_id,
            cip_sequence_count,
            response,
        )
    }
}

/// The response to a class 3 Forward_Open, which opens the session's connection, or to the
/// Forward_Close of that connection; `None` leaves `request` to the Message Router
pub(crate) fn handle_connection_request(
    explicit_connection: &mut Option<ExplicitConnection>,
    request: &MessageRouterRequest,
) -> Option<MessageRouterResponse> {
    let service = request.service_container.service();
    let CipDataOpt::Raw(request_data) = &request.request_data.additional_data else {
        unreachable!("data read from the wire is always raw");
    };
    // A request that does not decode is refused by the Message Router
    match service {
        ServiceCode::ForwardOpen | ServiceCode::LargeForwardOpen => {
            let forward_open = ForwardOpenRequest::read_le_args(
                &mut Cursor::new(request_data),
                (service == ServiceCode::LargeForwardOpen,),
            )
            .ok()?;
            if forward_open.transport_type_trigger.transport_class() != TransportClass::Class3 {
                return None;
            }
            Some(handle_forward_open(explicit_connection, forward_open))
        }
        ServiceCode::ForwardClose => {
            let triad = ForwardCloseRequest::read_le(&mut Cursor::new(request_data))
                .ok()?
                .connection_triad;
            // Any other connection is the Message Router's
            if established(explicit_connection)?.request.connection_triad != triad {
                return None;
            }
            *explicit_connection = None;
            tracing::info!(
                connection_serial_number = triad.connection_serial_number,
                "class 3 connection closed"
            );
            Some(message_router_response(
                service,
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(ForwardCloseResponse {
                    connection_triad: triad,
                    application_reply_size: 0,
                    application_reply: vec![],
                })),
            ))
        }
        _ => None,
    }
}

/// Opens the connection `forward_open` asks for, unless the session has one
fn handle_forward_open(
    explicit_connection: &mut Option<ExplicitConnection>,
    forward_open: ForwardOpenRequest,
) -> MessageRouterResponse {
    let service = forward_open.service_code();
    let triad = forward_open.connection_triad;
    // While the session's connection is established, every Forward_Open is refused before it is
    // checked
    if let Some(current) = established(explicit_connection) {
        let extended_status = if current.request.connection_triad == triad {
            ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
        } else {
            ConnectionManagerExtendedStatus::OutOfConnections
        };
        return rejection_response(connection_failure(service, extended_status), triad);
    }
    if let Some(extended_status) = unsupported_parameter(&forward_open) {
        return rejection_response(connection_failure(service, extended_status), triad);
    }

    let response = ForwardOpenResponse {
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
        o2t_id = format_args!("{:#010x}", response.o2t_network_connection_id),
        t2o_id = format_args!("{:#010x}", response.t2o_network_connection_id),
        o2t_api_us = response.o2t_actual_packet_interval,
        "class 3 connection opened"
    );
    let mut connection = ExplicitConnection {
        request: forward_open,
        response: response.clone(),
        deadline: Instant::now(),
        last_cip_sequence_count: None,
    };
    // The timer starts once the connection is established
    connection.restart_inactivity_timer();
    *explicit_connection = Some(connection);
    message_router_response(
        service,
        ResponseStatusCode::Success,
        vec![],
        CipDataOpt::Typed(Box::new(response)),
    )
}

/// The extended status refusing what `forward_open` asks for beyond a point-to-point connection
/// to the Message Router as server, with a packet interval; `None` when it asks for nothing more
fn unsupported_parameter(
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

/// The Send Unit Data reply carrying the Message Router's response to the connected request
/// inside; `None` when the request is discarded
pub(crate) fn handle_send_unit_data(
    message_router: &impl MessageRouter,
    explicit_connection: &mut Option<ExplicitConnection>,
    header: &EncapsulationHeader,
    command_specific_data: Option<&CommandSpecificData>,
    originator_ip: IpAddr,
) -> Option<EnIpPacket> {
    let Some(CommandSpecificData::SendUnitData(request_unit_data)) = command_specific_data else {
        tracing::warn!("discarded a Send Unit Data packet that did not decode");
        return None;
    };
    let CipMessage::Request(request) = &request_unit_data.cip_message else {
        tracing::warn!("discarded a Send Unit Data packet without a request");
        return None;
    };
    let cip_sequence_count = request_unit_data.cip_sequence_count;
    let connection = consume_request(
        explicit_connection,
        request_unit_data.connection_id,
        cip_sequence_count,
    )?;

    let (response, _) = handle_request(message_router, request, originator_ip, None);
    Some(EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            ..*header
        },
        command_specific_data: CommandSpecificData::SendUnitData(
            connection.reply_unit_data(cip_sequence_count, response),
        ),
    })
}

/// Consumes a connected request on `connection_id`: restarts the inactivity timer and returns the
/// connection, or `None` when the request is discarded because no established connection has
/// that O->T ID or `cip_sequence_count` repeats the last one
fn consume_request(
    explicit_connection: &mut Option<ExplicitConnection>,
    connection_id: CipUdint,
    cip_sequence_count: CipUint,
) -> Option<&ExplicitConnection> {
    let Some(connection) = established(explicit_connection) else {
        tracing::debug!("discarded a request: no class 3 connection");
        return None;
    };
    if connection.response.o2t_network_connection_id != connection_id {
        tracing::debug!(
            connection_id = format_args!("{connection_id:#010x}"),
            "discarded a request for another connection"
        );
        return None;
    }
    connection.restart_inactivity_timer();
    if connection.last_cip_sequence_count == Some(cip_sequence_count) {
        tracing::debug!(cip_sequence_count, "discarded a repeated request");
        return None;
    }
    connection.last_cip_sequence_count = Some(cip_sequence_count);
    Some(connection)
}

/// The connection while it is established; one that has timed out is deleted
fn established(
    explicit_connection: &mut Option<ExplicitConnection>,
) -> Option<&mut ExplicitConnection> {
    if let Some(connection) = explicit_connection.as_ref()
        && connection.deadline <= Instant::now()
    {
        tracing::warn!(
            connection_serial_number = connection.request.connection_triad.connection_serial_number,
            "class 3 connection timed out: no request in time"
        );
        *explicit_connection = None;
    }
    explicit_connection.as_mut()
}
