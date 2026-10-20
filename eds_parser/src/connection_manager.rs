//! `[Connection Manager]`: the `ConnectionN` entries, each a connection the device accepts, and
//! the two mask words that start one. A set mask bit means the device supports that option on the
//! connection. Their `Debug` names every bit, which is how a refusal shows what a connection
//! offers.

use anyhow::{Context, Result, bail};
use bilge::prelude::{DebugBits, FromBits, bitsize, u2, u3, u5, u9};

use eipscanne_rs::cip::connection_manager::parameters::{ConnectionPriority, RealTimeFormat};

use crate::syntax::Fields;
use crate::values::{DataFormat, FromField, NumberOrParam, Path, Unread};

/// `ConnectionN` of `[Connection Manager]`
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub trigger_and_transport: TriggerAndTransport,
    pub connection_parameters: ConnectionParameters,
    /// Microseconds
    pub o2t_rpi: Option<NumberOrParam>,
    /// Bytes of data, without the sequence count and real-time header
    pub o2t_size: Option<NumberOrParam>,
    pub o2t_format: Option<DataFormat>,
    pub t2o_rpi: Option<NumberOrParam>,
    pub t2o_size: Option<NumberOrParam>,
    pub t2o_format: Option<DataFormat>,
    proxy_config_size: Unread,
    proxy_config_format: Unread,
    target_config_size: Unread,
    target_config_format: Unread,
    pub name: String,
    pub help: String,
    pub path: Path,
    safety_async: Unread,
    safety_max_consumer_number: Unread,
}

// ======= Start of Connection impl ========

impl Connection {
    /// ```text
    /// Connection1 =
    ///     0x04010002,                 $ trigger and transport
    ///     0x44440405,                 $ connection parameters
    ///     Param1,32,Assem150,         $ O->T RPI, size, format
    ///     Param1,,Assem100,           $ T->O RPI, size (empty: from Assem100), format
    ///     ,,                          $ proxy config size, format
    ///     0,Assem151,                 $ target config size, format
    ///     "Exclusive Owner",          $ name
    ///     "",                         $ help
    ///     "20 04 24 [Param2] 2C 96 2C 64";    $ path
    /// ```
    pub(crate) fn read(mut fields: Fields) -> Result<Connection> {
        let connection = Connection {
            trigger_and_transport: fields.next()?,
            connection_parameters: fields.next()?,
            o2t_rpi: fields.next()?,
            o2t_size: fields.next()?,
            o2t_format: fields.next()?,
            t2o_rpi: fields.next()?,
            t2o_size: fields.next()?,
            t2o_format: fields.next()?,
            proxy_config_size: fields.next()?,
            proxy_config_format: fields.next()?,
            target_config_size: fields.next()?,
            target_config_format: fields.next()?,
            name: fields.next()?,
            help: fields.next()?,
            path: fields.next()?,
            safety_async: fields.next()?,
            safety_max_consumer_number: fields.next()?,
        };
        fields.finish()?;
        Ok(connection)
    }

    /// How the outputs signal run/idle: `0x44440405` is `Header32Bit` (format 4, bits 8-10)
    pub fn o2t_real_time_format(&self) -> Result<RealTimeFormat> {
        real_time_format(self.connection_parameters.o2t_real_time_format()).context("O->T")
    }

    /// How the inputs signal run/idle: `0x44440405` is `Modeless` (format 0, bits 12-14)
    pub fn t2o_real_time_format(&self) -> Result<RealTimeFormat> {
        real_time_format(self.connection_parameters.t2o_real_time_format()).context("T->O")
    }
}

// ^^^^^^^^ End of Connection impl ^^^^^^^^

/// Field 1 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the transport
/// classes, triggers and application types the connection supports. `0x04010002` is class 1
/// (bit 1), cyclic (bit 16), exclusive owner (bit 26).
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone)]
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

/// `0x04010002`
impl FromField for TriggerAndTransport {
    fn from_field(text: &str) -> Result<TriggerAndTransport> {
        u32::from_field(text).map(TriggerAndTransport::from)
    }
}

/// Field 2 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the sizes,
/// real-time formats, connection types and priorities the connection supports, per direction.
/// `0x44440405` is fixed sizes both ways (bits 0, 2), an O->T 32-bit header (format 4, bits
/// 8-10), modeless T->O (format 0, bits 12-14), point-to-point both ways (bits 18, 22) and
/// scheduled priority both ways (bits 26, 30).
#[bitsize(32)]
#[derive(FromBits, PartialEq, DebugBits, Copy, Clone)]
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

/// `0x44440405`
impl FromField for ConnectionParameters {
    fn from_field(text: &str) -> Result<ConnectionParameters> {
        u32::from_field(text).map(ConnectionParameters::from)
    }
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
/// 0 `Modeless`. 2 (reserved) and 5 to 7 are not supported by this scanner.
fn real_time_format(code: u3) -> Result<RealTimeFormat> {
    match u8::from(code) {
        0 => Ok(RealTimeFormat::Modeless),
        1 => Ok(RealTimeFormat::ZeroLength),
        3 => Ok(RealTimeFormat::Heartbeat),
        4 => Ok(RealTimeFormat::Header32Bit),
        _ => bail!("real-time format {code} is not supported"),
    }
}
