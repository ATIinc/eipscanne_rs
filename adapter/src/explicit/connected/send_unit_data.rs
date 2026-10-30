//! Request: each Send Unit Data request on a class 3 connection, answered like an unconnected
//! one.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinWrite;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::command::{CommandSpecificData, UnitPacketData};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::{handle_message_router_request, message_router_response};
use crate::message_router::MessageRouter;

/// The Send Unit Data reply carrying the response to the connected request inside; `None` when the
/// request is discarded
pub(crate) fn handle_send_unit_data(
    message_router: &impl MessageRouter,
    header: &EncapsulationHeader,
    command_specific_data: &CommandSpecificData,
    originator_ip: IpAddr,
) -> Option<EnIpPacket> {
    let CommandSpecificData::SendUnitData(request_unit_data) = command_specific_data else {
        tracing::warn!("discarded a Send Unit Data packet that did not decode");
        return None;
    };
    let CipMessage::Request(request) = &request_unit_data.cip_message else {
        tracing::warn!("discarded a Send Unit Data packet without a request");
        return None;
    };
    let cip_sequence_count = request_unit_data.cip_sequence_count;
    // The reply goes out on this connection, even when the request closes it
    let (t2o_network_connection_id, t2o_connection_size) = match message_router
        .extract_t2o_info_from_connected_request(
            request_unit_data.connection_id,
            cip_sequence_count,
            originator_ip,
        ) {
        Ok(reply_connection) => reply_connection,
        Err(error) => {
            tracing::debug!(%error, "discarded a request");
            return None;
        }
    };

    // Over a connection: no Socket Address Info
    let (response, _) = handle_message_router_request(message_router, request, originator_ip, None);
    let response = refuse_if_too_large(response, t2o_connection_size);
    Some(EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            ..*header
        },
        command_specific_data: CommandSpecificData::SendUnitData(UnitPacketData::new_connected(
            t2o_network_connection_id,
            cip_sequence_count,
            response,
        )),
    })
}

/// `response`, or Reply Data Too Large when it does not fit `t2o_connection_size`
fn refuse_if_too_large(
    response: MessageRouterResponse,
    t2o_connection_size: u16,
) -> MessageRouterResponse {
    let mut response_bytes = Vec::new();
    response
        .write_le(&mut Cursor::new(&mut response_bytes))
        .expect("a Message Router response writes to memory");
    // The CIP sequence count precedes the response
    if size_of::<CipUint>() + response_bytes.len() <= usize::from(t2o_connection_size) {
        return response;
    }
    let service = response.service_container.service();
    tracing::warn!(?service, t2o_connection_size, "reply data too large");
    message_router_response(
        service,
        ResponseStatusCode::ReplyDataTooLarge,
        vec![],
        CipDataOpt::Raw(vec![]),
    )
}
