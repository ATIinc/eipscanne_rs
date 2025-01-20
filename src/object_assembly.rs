use binrw::{
    binrw,    // #[binrw] attribute
    BinRead,  // trait for reading
    BinWrite, // trait for writing
};

use crate::cip::message::{MessageRouter, ServiceCode};
use crate::cip::path::CipPath;
use crate::cip::types::{CipByte, CipUdint};
use crate::eip::packet::EnIpPacketDescription;

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct ObjectAssembly<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    pub packet_description: EnIpPacketDescription,

    // TODO: Only return a None option if there are no remaining bytes to be read
    #[br(try)]
    pub cip_message: Option<MessageRouter<T>>,
}

impl ObjectAssembly<CipByte> {
    pub fn new_registration() -> Self {
        ObjectAssembly {
            packet_description: EnIpPacketDescription::new_registration_description(),
            cip_message: None,
        }
    }

    pub fn new_unregistration(session_handle: CipUdint) -> Self {
        ObjectAssembly {
            packet_description: EnIpPacketDescription::new_unregistration_description(
                session_handle,
            ),
            cip_message: None,
        }
    }
}

impl ObjectAssembly<CipPath> {
    pub fn new_identity(session_handle: CipUdint) -> Self {
        let identity_cip_message =
            MessageRouter::new_request(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

        ObjectAssembly {
            packet_description: EnIpPacketDescription::new_cip_description(
                session_handle,
                0,
                &identity_cip_message,
            ),
            cip_message: Some(identity_cip_message),
        }
    }
}
