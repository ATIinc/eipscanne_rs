//! Stage 4: closing the connection with a Forward_Close over the session.

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::Error;
use crate::explicit::decode_reply;
use crate::implicit::open::OpenConnection;
use crate::session::Session;

/// Sends the Forward_Close that matches the Forward_Open of `connection` and returns the
/// adapter's reply.
/// The adapter drops the connection on its own once it times out, so a failed close is not
/// fatal.
pub async fn forward_close(
    session: &mut Session,
    connection: &OpenConnection,
) -> Result<ForwardCloseResponse, Error> {
    // A Forward_Close names the connection by the triad and path of the Forward_Open that
    // opened it, with the same timing for the unconnected request itself
    let request = ForwardCloseRequest {
        priority_time_tick: connection.request.priority_time_tick,
        timeout_ticks: connection.request.timeout_ticks,
        connection_triad: connection.request.connection_triad,
        connection_path: connection.request.connection_path.clone(),
    };

    let reply = session
        .request(&RequestObjectAssembly::new_forward_close(
            session.session_handle(),
            request,
        ))
        .await?;
    decode_reply(&reply)
}
