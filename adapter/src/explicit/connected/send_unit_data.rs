//! Request: each Send Unit Data request on a class 3 connection, answered like an unconnected
//! one.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinWrite;

use eipscanne_rs::cip::connection_manager::parameters::{
    RealTimeFormat, TransportClass, connected_data_header_len,
};
use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::command::{CommandSpecificData, UnitPacketData};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::handle_message_router_request;
use crate::message_router::{MessageRouter, message_router_response};

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
        .accept_connected_request(
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
    Some(EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            ..*header
        },
        command_specific_data: CommandSpecificData::SendUnitData(reply_unit_data(
            t2o_network_connection_id,
            t2o_connection_size,
            cip_sequence_count,
            response,
        )),
    })
}

/// The Send Unit Data reply data carrying `response` on T->O connection
/// `t2o_network_connection_id` to the request with `cip_sequence_count`; Reply Data Too Large when
/// `response` does not fit `t2o_connection_size`
fn reply_unit_data(
    t2o_network_connection_id: CipUdint,
    t2o_connection_size: u16,
    cip_sequence_count: CipUint,
    mut response: MessageRouterResponse,
) -> UnitPacketData {
    let mut response_bytes = Vec::new();
    response
        .write_le(&mut Cursor::new(&mut response_bytes))
        .expect("a Message Router response writes to memory");
    // A class 3 message has no real-time header
    let header_len = connected_data_header_len(TransportClass::Class3, RealTimeFormat::Modeless);
    if usize::from(header_len) + response_bytes.len() > usize::from(t2o_connection_size) {
        let service = response.service_container.service();
        tracing::warn!(?service, t2o_connection_size, "reply data too large");
        response = message_router_response(
            service,
            ResponseStatusCode::ReplyDataTooLarge,
            vec![],
            CipDataOpt::Raw(vec![]),
        );
    }
    UnitPacketData::new_connected(t2o_network_connection_id, cip_sequence_count, response)
}
