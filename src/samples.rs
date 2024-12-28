use crate::cip::types::{CipRevision, CipShortString, CipUdint, CipUint, CipWord};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct IdentityObject {
    // Identity object attributes
    pub vendor_id: CipUint,
    pub device_type: CipUint,
    pub product_code: CipUint,
    pub revision: CipRevision,
    pub status: CipWord,
    pub serial_number: CipUdint,
    pub product_name: CipShortString,
}
