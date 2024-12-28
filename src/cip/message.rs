use crate::cip::types::{CipByte, CipPath, CipUsint};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MessageRouterRequest {
    pub service_code: CipUsint,
    pub path: CipPath,
    pub data: Vec<CipByte>,
    pub use_8_bit_path_segments: bool,
}
