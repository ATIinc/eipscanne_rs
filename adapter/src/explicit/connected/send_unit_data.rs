//! Request: each Send Unit Data request on the session's class 3 connection, answered like an
//! unconnected one.

use std::io::Cursor;
use std::net::IpAddr;

use binrw::BinWrite;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, ResponseStatusCode};
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::command::{CommandSpecificData, UnitPacketData};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::connected::ExplicitConnection;
use crate::explicit::handle_message_router_request;
use crate::message_router::{
    ExplicitOptions, MessageRouter, message_router_response, network_connection_size,
};

/// Bytes of the CIP sequence count in front of each message
const CIP_SEQUENCE_COUNT_LEN: usize = 2;

/// The Send Unit Data reply carrying the response to the connected request inside; `None` when the
/// request is discarded
pub(crate) fn handle_send_unit_data(
    message_router: &impl MessageRouter,
    options: ExplicitOptions,
    header: &EncapsulationHeader,
    originator_ip: IpAddr,
    command_specific_data: &CommandSpecificData,
    class_3_connection: &mut Option<ExplicitConnection>,
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
    let Some(connection) = class_3_connection.as_mut() else {
        tracing::debug!("discarded a request: no class 3 connection");
        return None;
    };
    if let Err(error) =
        connection.consume_request(request_unit_data.connection_id, cip_sequence_count)
    {
        tracing::debug!(%error, "discarded a request");
        return None;
    }
    // The reply goes out on this connection, even when the request closes it
    let t2o_network_connection_id = connection.response.t2o_network_connection_id;
    let t2o_connection_size =
        network_connection_size(&connection.request.t2o_network_connection_parameters);

    // Over a connection: no Socket Address Info
    let (response, _) = handle_message_router_request(
        message_router,
        options,
        request,
        originator_ip,
        None,
        class_3_connection,
    );
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
    UnitPacketData::new_connected(t2o_network_connection_id, cip_sequence_count, response)
}
