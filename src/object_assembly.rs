use binrw::{
    binrw, // #[binrw] attribute
           // BinRead,  // trait for reading
           // BinWrite, // trait for writing
};

//  Tried to use Deku but that didn't support nested structs: https://github.com/sharksforarms/deku
use bilge::prelude::{bitsize, u4, Bitsized, DebugBits, Number, TryFromBits};

use crate::cip::message::{MessageRouter, ServiceCode};
use crate::cip::path::CipPath;
use crate::cip::types::{CipByte, CipShortString, CipUdint, CipUint};
use crate::eip::packet::EnIpPacketDescription;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct ObjectAssembly {
    pub packet_description: EnIpPacketDescription,
    pub cip_message: Option<MessageRouter>,
}

impl ObjectAssembly {
    pub fn new_registration() -> Self {
        ObjectAssembly {
            packet_description: EnIpPacketDescription::new_registration(),
            cip_message: None,
        }
    }

    pub fn new_identity(session_handle: CipUdint) -> Self {
        let identity_cip_message =
            MessageRouter::new_request(ServiceCode::GetAttributeAll, CipPath::new(0x1, 0x1));

        ObjectAssembly {
            packet_description: EnIpPacketDescription::new_cip(
                session_handle,
                0,
                identity_cip_message.generate_packet_descriptors(),
            ),
            cip_message: Some(identity_cip_message),
        }
    }
}

/*/
Attribute: 1 (Vendor ID)
    Vendor ID: Teknic, Inc. (0x01a8)
Attribute: 2 (Device Type)
    Device Type: Generic Device (keyable) (0x002b)
Attribute: 3 (Product Code)
    Product Code: 1
Attribute: 4 (Revision)
    Major Revision: 2
    Minor Revision: 93
Attribute: 5 (Status)
    Status: 0x0000
        .... .... .... ...0 = Owned: 0
        .... .... .... .0.. = Configured: 0
        .... .... 0000 .... = Extended Device Status: 0x0
        .... ...0 .... .... = Minor Recoverable Fault: 0
        .... ..0. .... .... = Minor Unrecoverable Fault: 0
        .... .0.. .... .... = Major Recoverable Fault: 0
        .... 0... .... .... = Major Unrecoverable Fault: 0
        0000 .... .... .... = Extended Device Status 2: 0x0
Attribute: 6 (Serial Number)
    Serial Number: 0x01ff3d32
Attribute: 7 (Product Name)
    Product Name: ClearLink
*/

#[derive(Debug, PartialEq)]
#[repr(u16)]
pub enum VendorId {
    TeknicInc = 0x01a8,
    Unknown(u16),
}

#[derive(Debug, PartialEq)]
#[repr(u16)]
pub enum DeviceType {
    GenericDevice = 0x002b,
    Unknown(u16),
}

#[derive(Debug, PartialEq)]
pub struct Revision {
    major: CipByte,
    minor: CipByte,
}

#[bitsize(16)]
#[derive(TryFromBits, PartialEq, DebugBits)]
pub struct Status {
    owned: bool,
    unused1: bool,
    configured: bool,
    unused2: bool,
    extended_device_status: u4,
    minor_recoverable_fault: bool,
    minor_unrecoverable_fault: bool,
    major_recoverable_fault: bool,
    major_unrecoverable_fault: bool,
    extended_device_status_2: u4,
}

pub struct IdentityResponse {
    pub vendor_id: VendorId,
    pub device_type: DeviceType,
    pub product_code: CipUint,
    pub revision: Revision,
    pub status: Status,
    pub serial_number: u32,
    pub product_name: CipShortString,
}
