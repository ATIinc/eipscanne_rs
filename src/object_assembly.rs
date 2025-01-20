use binrw::{
    binread,
    binwrite,
    BinRead,
    BinWrite, // trait for writing
};

use crate::cip::message::{MessageRouterRequest, MessageRouterResponse, ServiceCode};
use crate::cip::path::CipPath;
use crate::cip::types::{CipByte, CipUdint};
use crate::eip::packet::EnIpPacketDescription;

#[binwrite]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct RequestObjectAssembly<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub packet_description: EnIpPacketDescription,
    pub cip_message: Option<MessageRouterRequest<T>>,
}

#[binread]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ResponseObjectAssembly<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub packet_description: EnIpPacketDescription,

    // TODO: Only return a None option if there are no remaining bytes to be read
    #[br(try)]
    pub cip_message: Option<MessageRouterResponse<T>>,
}

impl RequestObjectAssembly<CipByte> {
    pub fn new_registration() -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_registration_description(),
            cip_message: None,
        }
    }

    pub fn new_unregistration(session_handle: CipUdint) -> Self {
        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_unregistration_description(
                session_handle,
            ),
            cip_message: None,
        }
    }
}

impl RequestObjectAssembly<CipPath> {
    pub fn new_identity(session_handle: CipUdint) -> Self {
        let identity_cip_message =
            MessageRouterRequest::new(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

        RequestObjectAssembly {
            packet_description: EnIpPacketDescription::new_cip_description(
                session_handle,
                0,
                &identity_cip_message,
            ),
            cip_message: Some(identity_cip_message),
        }
    }
}
