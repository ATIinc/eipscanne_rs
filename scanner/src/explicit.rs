//! Explicit (unconnected) messaging: one request, one reply, over the session.
//!
//! The request is a Message Router request for a service on a path; the reply's general status
//! says whether the service was done, and its data (if any) is whatever the service returns,
//! declared by the caller as a `binrw` type.

use std::fmt;
use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUint;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::session::{Session, SessionError};

/// What can go wrong with an explicit request
#[derive(Debug)]
pub enum ExplicitError {
    Session(SessionError),
    /// The reply carried no Message Router response
    NoResponse,
    /// The adapter answered with a general status other than success
    Status {
        general_status: ResponseStatusCode,
        additional_status: Vec<CipUint>,
    },
    /// The reply's data did not decode as the expected type
    Parse(binrw::Error),
}

/// Sends `service` on `request_path` with the optional request `data`, reads the reply and
/// returns it once its general status is success
pub async fn send_request(
    session: &mut Session,
    request_path: CipPath,
    service: ServiceCode,
    data: Option<Box<dyn CipData>>,
) -> Result<EnIpPacket, ExplicitError> {
    session
        .send(&RequestObjectAssembly::new_service_request(
            session.session_handle(),
            request_path,
            service,
            data,
        ))
        .await?;
    let reply = session.read_reply().await?;

    let Some(response) = reply.response() else {
        return Err(ExplicitError::NoResponse);
    };
    if !response.is_success() {
        return Err(ExplicitError::Status {
            general_status: response.response_data.status,
            additional_status: response.response_data.additional_status.clone(),
        });
    }

    Ok(reply)
}

/// The data of a reply's Message Router response decoded as a `T`
pub fn typed_data<T>(reply: &EnIpPacket) -> Result<T, ExplicitError>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let Some(response) = reply.response() else {
        return Err(ExplicitError::NoResponse);
    };
    // A reply read from the wire always holds its data raw
    let CipDataOpt::Raw(raw) = &response.response_data.data else {
        return Err(ExplicitError::NoResponse);
    };
    Ok(T::read_le(&mut Cursor::new(raw))?)
}

/// Get_Attributes_All on the Identity object: who the adapter is
pub async fn read_identity(session: &mut Session) -> Result<IdentityResponse, ExplicitError> {
    let reply = send_request(
        session,
        CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
        ServiceCode::GetAttributeAll,
        None,
    )
    .await?;
    typed_data(&reply)
}

// ======= Start of ExplicitError impl ========

impl fmt::Display for ExplicitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExplicitError::Session(error) => write!(f, "{error}"),
            ExplicitError::NoResponse => {
                write!(f, "the reply carried no Message Router response")
            }
            ExplicitError::Status {
                general_status,
                additional_status,
            } if additional_status.is_empty() => {
                write!(f, "the adapter answered with status {general_status:?}")
            }
            ExplicitError::Status {
                general_status,
                additional_status,
            } => write!(
                f,
                "the adapter answered with status {general_status:?}, additional status {additional_status:#06x?}"
            ),
            ExplicitError::Parse(error) => write!(f, "the reply's data did not decode: {error}"),
        }
    }
}

impl std::error::Error for ExplicitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExplicitError::Session(error) => Some(error),
            ExplicitError::Parse(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SessionError> for ExplicitError {
    fn from(error: SessionError) -> Self {
        ExplicitError::Session(error)
    }
}

impl From<binrw::Error> for ExplicitError {
    fn from(error: binrw::Error) -> Self {
        ExplicitError::Parse(error)
    }
}

// ^^^^^^^^ End of ExplicitError impl ^^^^^^^^
