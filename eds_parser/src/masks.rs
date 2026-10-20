//! The numeric fields read as bits and codes:
//!
//! * the two mask words that start a `ConnectionN` entry. A set bit means the device supports that
//!   option on the connection; one value is picked from them for each Forward_Open field. Their
//!   `Debug` names every bit, which is how a refusal shows what a connection offers.
//! * a `ParamN` entry's descriptor and data type.

use bilge::prelude::{DebugBits, FromBits, bitsize, u2, u3, u5, u9};
use serde::Deserialize;

use eipscanne_rs::cip::connection_manager::parameters::{ConnectionPriority, RealTimeFormat};

/// Field 1 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the transport
/// classes, triggers and application types the connection supports. `0x04010002` is class 1
/// (bit 1), cyclic (bit 16), exclusive owner (bit 26).
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone, Deserialize)]
#[serde(from = "u32")]
pub struct TriggerAndTransport {
    pub class_0: bool,
    pub class_1: bool,
    pub class_2: bool,
    pub class_3: bool,
    pub class_4: bool,
    pub class_5: bool,
    pub class_6: bool,
    reserved_7_15: u9,
    pub cyclic: bool,
    pub change_of_state: bool,
    pub application: bool,
    reserved_19_23: u5,
    pub listen_only: bool,
    pub input_only: bool,
    pub exclusive_owner: bool,
    pub redundant_owner: bool,
    reserved_28_30: u3,
    /// Not read: vendors disagree on it, and a Forward_Open's originator is always the client
    server: bool,
}

/// Field 2 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the sizes,
/// real-time formats, connection types and priorities the connection supports, per direction. `0x44440405` is fixed sizes both ways (bits 0, 2), an O->T 32-bit
/// header (format 4, bits 8-10), modeless T->O (format 0, bits 12-14), point-to-point both ways
/// (bits 18, 22) and scheduled priority both ways (bits 26, 30).
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone, Deserialize)]
#[serde(from = "u32")]
pub struct ConnectionParameters {
    pub o2t_fixed_size: bool,
    pub o2t_variable_size: bool,
    pub t2o_fixed_size: bool,
    pub t2o_variable_size: bool,
    obsolete_o2t_bytes_per_slot: u2,
    obsolete_t2o_bytes_per_slot: u2,
    /// 0 modeless, 1 zero length, 2 (reserved), 3 heartbeat, 4 32-bit header, 5-7 others
    pub o2t_real_time_format: u3,
    reserved_11: bool,
    pub t2o_real_time_format: u3,
    reserved_15: bool,
    pub o2t_null: bool,
    pub o2t_multicast: bool,
    pub o2t_point_to_point: bool,
    reserved_19: bool,
    pub t2o_null: bool,
    pub t2o_multicast: bool,
    pub t2o_point_to_point: bool,
    reserved_23: bool,
    pub o2t_low_priority: bool,
    pub o2t_high_priority: bool,
    pub o2t_scheduled_priority: bool,
    reserved_27: bool,
    pub t2o_low_priority: bool,
    pub t2o_high_priority: bool,
    pub t2o_scheduled_priority: bool,
    reserved_31: bool,
}

// ======= Start of ConnectionParameters impl ========

impl ConnectionParameters {
    /// The best priority both directions support: scheduled, else high, else low. `0x44440405`
    /// (scheduled only, both ways) is `Scheduled`; `0x3775_0405` (T->O low and high) is `High`.
    pub fn shared_priority(&self) -> Option<ConnectionPriority> {
        if self.o2t_scheduled_priority() && self.t2o_scheduled_priority() {
            Some(ConnectionPriority::Scheduled)
        } else if self.o2t_high_priority() && self.t2o_high_priority() {
            Some(ConnectionPriority::High)
        } else if self.o2t_low_priority() && self.t2o_low_priority() {
            Some(ConnectionPriority::Low)
        } else {
            None
        }
    }
}

// ^^^^^^^^ End of ConnectionParameters impl ^^^^^^^^

/// The real-time format a 3-bit code of `ConnectionParameters` stands for: 4 is `Header32Bit`,
/// 0 `Modeless`. `None` for 2 (reserved) and 5 to 7, which this scanner does not support.
pub fn real_time_format(code: u3) -> Option<RealTimeFormat> {
    match u8::from(code) {
        0 => Some(RealTimeFormat::Modeless),
        1 => Some(RealTimeFormat::ZeroLength),
        3 => Some(RealTimeFormat::Heartbeat),
        4 => Some(RealTimeFormat::Header32Bit),
        _ => None,
    }
}

/// Field 4 of `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`: what the param
/// supports and how a tool treats it. `0x0004` is supports_scaling (bit 2); `0x0010` is read-only
/// (bit 4).
#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone, Deserialize)]
#[serde(from = "u16")]
pub struct ParamDescriptor {
    pub supports_settable_path: bool,
    pub supports_enumerated_strings: bool,
    pub supports_scaling: bool,
    pub supports_scaling_links: bool,
    pub read_only: bool,
    pub monitor: bool,
    pub supports_extended_precision_scaling: bool,
    obsolete_7_8: u2,
    pub non_displayed: bool,
    pub indirect_reference: bool,
    pub not_addressable: bool,
    pub save_supported: bool,
    pub apply_supported: bool,
    pub write_only: bool,
    reserved_15: bool,
}

/// Field 5 of `Param1 = 0,,,0x0004,0xC8,4,"RPI","us","",1000,1000000,1000000;`: the CIP data type
/// of the param's value. `0xC8` is UDINT. Each variant is the EDS spelling in Rust case
/// (`TimeOfDay` is TIME_OF_DAY); an unlisted code is `Unknown(code)`.
#[bitsize(8)]
#[derive(FromBits, PartialEq, Debug, Copy, Clone, Deserialize)]
#[serde(from = "u8")]
#[repr(u8)]
pub enum CipDataType {
    Bool = 0xC1,
    Sint = 0xC2,
    Int = 0xC3,
    Dint = 0xC4,
    Lint = 0xC5,
    Usint = 0xC6,
    Uint = 0xC7,
    Udint = 0xC8,
    Ulint = 0xC9,
    Real = 0xCA,
    Lreal = 0xCB,
    Stime = 0xCC,
    Date = 0xCD,
    TimeOfDay = 0xCE,
    DateAndTime = 0xCF,
    String = 0xD0,
    Byte = 0xD1,
    Word = 0xD2,
    Dword = 0xD3,
    Lword = 0xD4,
    String2 = 0xD5,
    Ftime = 0xD6,
    Ltime = 0xD7,
    Itime = 0xD8,
    StringN = 0xD9,
    ShortString = 0xDA,
    Time = 0xDB,
    Epath = 0xDC,
    Engunit = 0xDD,
    StringI = 0xDE,

    #[fallback]
    Unknown(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_priority_is_the_best_one_both_directions_support() {
        let priority = |word: u32| ConnectionParameters::from(word).shared_priority();

        assert_eq!(priority(0x7775_0405), Some(ConnectionPriority::Scheduled));
        assert_eq!(priority(0x3775_0405), Some(ConnectionPriority::High)); // T->O: low + high
        assert_eq!(priority(0x1375_0405), Some(ConnectionPriority::Low)); // T->O low, O->T low + high
        assert_eq!(priority(0x1275_0405), None); // T->O low, O->T high
    }

    #[test]
    fn real_time_formats_map_or_are_refused() {
        let formats: Vec<Option<RealTimeFormat>> =
            (0..8).map(|code| real_time_format(u3::new(code))).collect();
        assert_eq!(
            formats,
            vec![
                Some(RealTimeFormat::Modeless),
                Some(RealTimeFormat::ZeroLength),
                None,
                Some(RealTimeFormat::Heartbeat),
                Some(RealTimeFormat::Header32Bit),
                None,
                None,
                None
            ]
        );
    }
}
