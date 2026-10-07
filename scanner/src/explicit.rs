//! Explicit (unconnected) messaging: one request, one reply, over the session.
//!
//! The request is a Message Router request for a service on a path; the reply's general status
//! says whether the service was done, and its data (if any) is whatever the service returns,
//! declared by the caller as a `binrw` type.

use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::{Error, Result};
use crate::session::Session;

/// Sends `service` on `request_path` with the optional request `data` and returns the reply
/// once the adapter accepted the request
pub async fn send_request(
    session: &mut Session,
    request_path: CipPath,
    service: ServiceCode,
    data: Option<Box<dyn CipData>>,
) -> Result<EnIpPacket> {
    session
        .request(&RequestObjectAssembly::new_service_request(
            session.session_handle(),
            request_path,
            service,
            data,
        ))
        .await
}

/// The data of a reply's Message Router response, decoded as a `T` declared by the caller
pub fn decode_reply<T>(reply: &EnIpPacket) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    // A reply read from the wire always holds its data raw
    let CipDataOpt::Raw(raw) = &reply
        .response()
        .ok_or(Error::NoResponse)?
        .response_data
        .data
    else {
        return Err(Error::UnexpectedReply(
            "the reply's data is not raw bytes".to_string(),
        ));
    };
    Ok(T::read_le(&mut Cursor::new(raw))?)
}

/// Get_Attributes_All on the Identity object: who the adapter is
pub async fn read_identity(session: &mut Session) -> Result<IdentityResponse> {
    let reply = send_request(
        session,
        CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
        ServiceCode::GetAttributeAll,
        None,
    )
    .await?;
    decode_reply(&reply)
}
