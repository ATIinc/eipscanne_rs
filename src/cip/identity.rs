use bilge::prelude::{bitsize, u4, Bitsized, DebugBits, FromBits, Number};

use binrw::{
    binrw, // #[binrw] attribute
};

use crate::cip::types::{CipByte, CipShortString, CipUint};

/*
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

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[repr(u16)]
pub enum VendorId {
    #[brw(magic = 0x01a8u16)]
    TeknicInc,
    Unknown(u16),
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
#[repr(u16)]
pub enum DeviceType {
    #[brw(magic = 0x002bu16)]
    GenericDevice,
    Unknown(u16),
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct Revision {
    pub major: CipByte,
    pub minor: CipByte,
}

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits)]
pub struct IdentityStatusBits {
    pub owned: bool,
    pub unused1: bool,
    pub configured: bool,
    pub unused2: bool,
    pub extended_device_status: u4,
    pub minor_recoverable_fault: bool,
    pub minor_unrecoverable_fault: bool,
    pub major_recoverable_fault: bool,
    pub major_unrecoverable_fault: bool,
    pub extended_device_status_2: u4,
}

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IdentityStatus {
    status_representation: CipUint,
}

// ======= Start of IdentityStatus impl ========

impl From<IdentityStatus> for IdentityStatusBits {
    fn from(segment: IdentityStatus) -> Self {
        IdentityStatusBits::from(segment.status_representation)
    }
}

impl From<IdentityStatusBits> for IdentityStatus {
    fn from(segment: IdentityStatusBits) -> Self {
        IdentityStatus {
            status_representation: segment.value,
        }
    }
}

// ^^^^^^^^ End of IdentityStatus impl ^^^^^^^^

#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq)]
pub struct IdentityResponse {
    pub vendor_id: VendorId,
    pub device_type: DeviceType,
    pub product_code: CipUint,
    pub revision: Revision,
    pub status: IdentityStatus,
    pub serial_number: u32,
    pub product_name: CipShortString,
}
