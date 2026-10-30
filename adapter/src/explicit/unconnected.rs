//! Unconnected explicit messaging: each Send RR Data request answered, by the session's class 3
//! connection when it opens or closes it, otherwise by the Message Router.

use std::net::IpAddr;

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::eip::command::{CommandSpecificData, EncapsStatusCode, RRPacketData};
use eipscanne_rs::eip::constants::{CIP_INTERFACE_HANDLE, NO_ENCAPSULATION_TIMEOUT};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::explicit::connected::ExplicitConnection;
use crate::explicit::handle_message_router_request;
use crate::message_router::{ExplicitOptions, MessageRouter};
use crate::packet::error_reply;

/// The Send RR Data reply carrying the response to the request inside: the class 3 connection's
/// when it opens or closes it, otherwise the Message Router's; a bare reply refusing a packet
/// without a request
pub(crate) fn handle_send_rr_data(
    message_router: &impl MessageRouter,
    options: ExplicitOptions,
    header: &EncapsulationHeader,
    command_specific_data: &CommandSpecificData,
    originator_ip: IpAddr,
    class_3_connection: &mut Option<ExplicitConnection>,
) -> EnIpPacket {
    let CommandSpecificData::SendRrData(request_rr_data) = command_specific_data else {
        return error_reply(header, EncapsStatusCode::InvalidFormatOrData);
    };
    let CipMessage::Request(request) = &request_rr_data.cip_message else {
        return error_reply(header, EncapsStatusCode::InvalidFormatOrData);
    };

    let (response, socket_addr_info_items) = handle_message_router_request(
        message_router,
        options,
        request,
        originator_ip,
        request_rr_data.socket_addr_info_items.t2o,
        class_3_connection,
    );

    let mut reply_rr_data =
        RRPacketData::new_unconnected(CIP_INTERFACE_HANDLE, NO_ENCAPSULATION_TIMEOUT, response);
    reply_rr_data.socket_addr_info_items = socket_addr_info_items;
    EnIpPacket {
        header: EncapsulationHeader {
            length: None,
            ..*header
        },
        command_specific_data: CommandSpecificData::SendRrData(reply_rr_data),
    }
}
