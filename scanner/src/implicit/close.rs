//! Stage 4: closing the connection with a Forward_Close over the session.

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerResponse;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::Error;
use crate::implicit::open::OpenConnection;
use crate::session::{Session, router_response};

/// Sends the Forward_Close that matches the Forward_Open of `connection` and reads the reply.
/// The adapter drops the connection on its own once it times out, so a failed close is not
/// fatal.
pub async fn forward_close(
    session: &mut Session,
    connection: &OpenConnection,
) -> Result<(), Error> {
    // A Forward_Close names the connection by the triad and path of the Forward_Open that
    // opened it, with the same timing for the unconnected request itself
    let request = ForwardCloseRequest {
        priority_time_tick: connection.request.priority_time_tick,
        timeout_ticks: connection.request.timeout_ticks,
        connection_triad: connection.request.connection_triad,
        connection_path: connection.request.connection_path.clone(),
    };

    session
        .send(&RequestObjectAssembly::new_forward_close(
            session.session_handle(),
            request,
        ))
        .await?;
    let reply = session.read_reply().await?;

    let router_response = router_response(&reply)?;
    let response = ConnectionManagerResponse::from_message_router_response(router_response)
        .map_err(|error| Error::UnexpectedReply(error.to_string()))?;

    match response {
        ConnectionManagerResponse::ForwardClose(_) => Ok(()),
        ConnectionManagerResponse::Unsuccessful(_) => {
            Err(Error::rejected(ServiceCode::ForwardClose, router_response))
        }
        ConnectionManagerResponse::ForwardOpen(_) => Err(Error::UnexpectedReply(
            "a Forward_Open reply answered the Forward_Close".to_string(),
        )),
    }
}
