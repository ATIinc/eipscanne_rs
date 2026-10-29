//! Request: each Send Unit Data request on the session's class 3 connection, answered by the
//! Message Router.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinWrite;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::command::{CommandSpecificData, UnitPacketData};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::connected::{ExplicitConnection, delete_timed_out_connection};
use crate::explicit::handle_request;
use crate::message_router::{MessageRouter, message_router_response, size_and_types};

/// Bytes of the CIP sequence count in front of each message
const CIP_SEQUENCE_COUNT_LEN: usize = 2;

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

    // Over a connection: no class 3 open or close, no Socket Address Info
    let (response, _) = handle_request(message_router, None, request, originator_ip, None);
    Some(EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            ..*header
        },
        command_specific_data: CommandSpecificData::SendUnitData(reply_unit_data(
            connection,
            cip_sequence_count,
            response,
        )),
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
    delete_timed_out_connection(explicit_connection);
    let Some(connection) = explicit_connection.as_mut() else {
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

/// The Send Unit Data reply data carrying `response` on `connection` to the request with
/// `cip_sequence_count`; Reply Data Too Large when `response` does not fit the T->O connection size
fn reply_unit_data(
    connection: &ExplicitConnection,
    cip_sequence_count: CipUint,
    mut response: MessageRouterResponse,
) -> UnitPacketData {
    let mut response_bytes = Vec::new();
    response
        .write_le(&mut Cursor::new(&mut response_bytes))
        .expect("a Message Router response writes to memory");
    let (t2o_connection_size, _, _) =
        size_and_types(&connection.request.t2o_network_connection_parameters);
    if CIP_SEQUENCE_COUNT_LEN + response_bytes.len() > usize::from(t2o_connection_size) {
        let service = response.service_container.service();
        tracing::warn!(?service, t2o_connection_size, "reply data too large");
        response = message_router_response(
            service,
            ResponseStatusCode::ReplyDataTooLarge,
            vec![],
            CipDataOpt::Raw(vec![]),
        );
    }
    UnitPacketData::new_connected(
        connection.response.t2o_network_connection_id,
        cip_sequence_count,
        response,
    )
}
