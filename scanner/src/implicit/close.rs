//! Stage 4: closing the connection with a Forward_Close over the session.

use std::fmt;

use eipscanne_rs::cip::connection_manager::forward_close::ForwardCloseRequest;
use eipscanne_rs::cip::connection_manager::response::{
    ConnectionManagerExtendedStatus, ConnectionManagerResponse,
};
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::implicit::open::OpenConnection;
use crate::session::{Session, SessionError};

/// Why a connection could not be closed cleanly. The adapter drops the connection on its own
/// once it times out, so a failed close is not fatal.
#[derive(Debug)]
pub enum CloseError {
    Session(SessionError),
    /// The adapter refused the close, e.g. because the connection was already gone
    Rejected {
        general_status: ResponseStatusCode,
        extended_status: Option<ConnectionManagerExtendedStatus>,
    },
    /// The reply parsed, but was not a Forward_Close reply
    UnexpectedReply(String),
}

/// Sends the Forward_Close that matches the Forward_Open of `connection` and reads the reply
pub async fn forward_close(
    session: &mut Session,
    connection: &OpenConnection,
) -> Result<(), CloseError> {
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

    let Some(router_response) = reply.response() else {
        return Err(CloseError::UnexpectedReply(
            "the reply carries no Message Router response".to_string(),
        ));
    };
    let response = ConnectionManagerResponse::from_message_router_response(router_response)
        .map_err(|error| CloseError::UnexpectedReply(error.to_string()))?;

    match response {
        ConnectionManagerResponse::ForwardClose(_) => Ok(()),
        ConnectionManagerResponse::Unsuccessful(_) => Err(CloseError::Rejected {
            general_status: router_response.response_data.status,
            extended_status: ConnectionManagerExtendedStatus::from_additional_status(
                &router_response.response_data.additional_status,
            ),
        }),
        ConnectionManagerResponse::ForwardOpen(_) => Err(CloseError::UnexpectedReply(
            "a Forward_Open reply answered the Forward_Close".to_string(),
        )),
    }
}

// ======= Start of CloseError impl ========

impl fmt::Display for CloseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloseError::Session(error) => write!(f, "{error}"),
            CloseError::Rejected {
                general_status,
                extended_status: Some(extended_status),
            } => write!(
                f,
                "the adapter rejected the Forward_Close: {general_status}, {extended_status}"
            ),
            CloseError::Rejected {
                general_status,
                extended_status: None,
            } => write!(
                f,
                "the adapter rejected the Forward_Close: {general_status}"
            ),
            CloseError::UnexpectedReply(what) => {
                write!(f, "unexpected reply to the Forward_Close: {what}")
            }
        }
    }
}

impl std::error::Error for CloseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CloseError::Session(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SessionError> for CloseError {
    fn from(error: SessionError) -> Self {
        CloseError::Session(error)
    }
}

// ^^^^^^^^ End of CloseError impl ^^^^^^^^
