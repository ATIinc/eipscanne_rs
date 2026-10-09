//! The Connection Manager's two services, shared by both kinds of connection: Forward_Open opens a
//! connection, Forward_Close closes it. Both are unconnected requests over the session; `implicit`
//! (class 1) and `explicit::connected` (class 3) open and close their connections through them.

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::Result;
use crate::explicit::decode_reply;
use crate::session::Session;

/// Sends `request` (a Forward_Open or Large_Forward_Open, by the width of its connection
/// parameters) and returns the adapter's reply decoded, and as read for its other items
pub async fn forward_open(
    session: &mut Session,
    request: &ForwardOpenRequest,
) -> Result<(ForwardOpenResponse, EnIpPacket)> {
    let reply: EnIpPacket = session
        .request(&RequestObjectAssembly::new_forward_open(
            session.session_handle(),
            request.clone(),
        ))
        .await?;
    let response: ForwardOpenResponse = decode_reply(&reply)?;
    Ok((response, reply))
}

/// Closes the connection `request` opened and returns the adapter's reply. A failed close is not
/// fatal: the adapter drops the connection once it times out.
pub async fn forward_close(
    session: &mut Session,
    request: &ForwardOpenRequest,
) -> Result<ForwardCloseResponse> {
    // The connection is named by its Forward_Open's triad and path
    let close_request = ForwardCloseRequest {
        priority_time_tick: request.priority_time_tick,
        timeout_ticks: request.timeout_ticks,
        connection_triad: request.connection_triad,
        connection_path: request.connection_path.clone(),
    };

    let reply: EnIpPacket = session
        .request(&RequestObjectAssembly::new_forward_close(
            session.session_handle(),
            close_request,
        ))
        .await?;
    decode_reply(&reply)
}
