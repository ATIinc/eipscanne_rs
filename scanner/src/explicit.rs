//! Explicit (unconnected) messaging: one Message Router request, one reply, over the session.
//! The caller declares the reply data as a `binrw` type. Connected (class 3) explicit messaging
//! lives in [`connected`].

pub mod connected;

use std::io::Cursor;

use binrw::BinRead;

use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::packet::EnIpPacket;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use crate::error::{Error, Result};
use crate::session::Session;

/// Sends `service` on `request_path` with optional `data` and returns the accepted reply
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

/// The data of a reply's Message Router response, decoded as `T`
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

/// Get_Attribute_Single on `path`, decoded as `T`
pub async fn get_attribute<T>(session: &mut Session, path: &CipPath) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let reply = send_request(session, path.clone(), ServiceCode::GetAttributeSingle, None).await?;
    decode_reply(&reply)
}

/// Set_Attribute_Single on `path`
pub async fn set_attribute(
    session: &mut Session,
    path: &CipPath,
    data: impl CipData + 'static,
) -> Result<()> {
    send_request(
        session,
        path.clone(),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(data)),
    )
    .await?;
    Ok(())
}
