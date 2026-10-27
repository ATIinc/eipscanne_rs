//! Connected explicit messaging: requests over a class 3 connection to the Message Router.
//!
//! ```text
//! Stage    What happens on the wire                                        Function
//! -------  --------------------------------------------------------------  ----------------------
//! Open     SendRRData(Forward_Open), read the reply -> ExplicitConnection  forward_open_explicit
//! Request  SendUnitData(request) on O->T, read SendUnitData on T->O        send_connected_request
//! Close    SendRRData(Forward_Close), read the reply                       forward_close
//! ```
//!
//! Everything goes over the session's TCP connection. The scanner keeps no state between
//! requests: the caller owns the CIP Sequence Count (a new request advances it, a request sent
//! again keeps it) and sends often enough that the adapter does not time the connection out (see
//! the `read-identity-connected` example). Replies are read with [`decode_reply`]; the connection
//! is closed with [`forward_close`], as a class 1 connection is.
//!
//! [`decode_reply`]: crate::explicit::decode_reply
//! [`forward_close`]: crate::implicit::connection::forward_close

use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::message::data::CipData;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::Result;
use crate::explicit::decode_reply;
use crate::session::Session;

/// An open class 3 connection: the Forward_Open sent and the adapter's reply. The connection IDs
/// come from `response`; `request` names the connection in the Forward_Close.
#[derive(Debug)]
pub struct ExplicitConnection {
    pub request: ForwardOpenRequest,
    pub response: ForwardOpenResponse,
}

/// Sends `request` (a Forward_Open or Large_Forward_Open, by the width of its connection
/// parameters) and reads the adapter's reply.
pub async fn forward_open_explicit(
    session: &mut Session,
    request: ForwardOpenRequest,
) -> Result<ExplicitConnection> {
    let reply: EnIpPacket = session
        .request(&RequestObjectAssembly::new_forward_open(
            session.session_handle(),
            request.clone(),
        ))
        .await?;
    let response: ForwardOpenResponse = decode_reply(&reply)?;
    tracing::info!(
        o2t_id = format_args!("{:#010x}", response.o2t_network_connection_id),
        t2o_id = format_args!("{:#010x}", response.t2o_network_connection_id),
        o2t_api_us = response.o2t_actual_packet_interval,
        t2o_api_us = response.t2o_actual_packet_interval,
        "class 3 connection opened"
    );
    Ok(ExplicitConnection { request, response })
}

/// Sends `service` on `request_path` with optional `data` over `connection` and returns the
/// accepted reply
pub async fn send_connected_request(
    session: &mut Session,
    connection: &ExplicitConnection,
    cip_sequence_count: CipUint,
    request_path: &CipPath,
    service: ServiceCode,
    data: Option<Box<dyn CipData>>,
) -> Result<EnIpPacket> {
    tracing::debug!(?service, cip_sequence_count, "sent a connected request");
    session
        .request(&EnIpPacket::new_send_unit_data(
            session.session_handle(),
            connection.response.o2t_network_connection_id,
            cip_sequence_count,
            MessageRouterRequest::new_data(service, request_path.clone(), data),
        ))
        .await
}
