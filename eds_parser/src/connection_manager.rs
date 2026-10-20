//! `[Connection Manager]`: the `ConnectionN` entries, each a connection the device accepts, and
//! the two mask words that start one. A set mask bit means the device supports that option on the
//! connection. Their `Debug` names every bit, which is how a refusal shows what a connection
//! offers.

use anyhow::{Context, Result, bail};
use bilge::prelude::{BuilderBits, DebugBits, FromBits, bitsize, u1, u2, u3, u4, u5, u9};

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

    /// How the outputs signal run/idle: `0x44440405` is `Header32Bit` (bits 8-10)
    pub fn o2t_real_time_format(&self) -> Result<RealTimeFormat> {
        RealTimeFormat::try_from(self.connection_parameters.o2t_real_time_transfer_format())
            .context("O->T")
    }

    /// How the inputs signal run/idle: `0x44440405` is `Modeless` (bits 12-14)
    pub fn t2o_real_time_format(&self) -> Result<RealTimeFormat> {
        RealTimeFormat::try_from(self.connection_parameters.t2o_real_time_transfer_format())
            .context("T->O")
    }
}

// ^^^^^^^^ End of Connection impl ^^^^^^^^

/// Field 1 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the transport
/// classes, triggers and application types the connection supports. `0x04010002` is class 1
/// (bit 1), cyclic (bit 16), exclusive owner (bit 26).
#[bitsize(32)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct TriggerAndTransport {
    /// Bits 0-15
    pub transport_classes: TransportClasses,
    /// Bits 16-23
    pub triggers: Triggers,
    /// Bits 24-27
    pub application_types: ApplicationTypes,
    reserved: u3,
    /// Bit 31, client 0 or server 1. Not read: vendors disagree on it, and a Forward_Open's
    /// originator is always the client.
    server: bool,
}

/// `0x04010002`
impl FromField for TriggerAndTransport {
    fn from_field(text: &str) -> Result<TriggerAndTransport> {
        u32::from_field(text).map(TriggerAndTransport::from)
    }
}

/// The transport classes a connection supports
#[bitsize(16)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct TransportClasses {
    pub class_0: bool,
    pub class_1: bool,
    pub class_2: bool,
    pub class_3: bool,
    pub class_4: bool,
    pub class_5: bool,
    pub class_6: bool,
    reserved: u9,
}

/// The production triggers a connection supports
#[bitsize(8)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct Triggers {
    pub cyclic: bool,
    pub change_of_state: bool,
    pub application: bool,
    reserved: u5,
}

/// The application types a connection supports
#[bitsize(4)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct ApplicationTypes {
    pub listen_only: bool,
    pub input_only: bool,
    pub exclusive_owner: bool,
    pub redundant_owner: bool,
}

/// Field 2 of `Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, ...`: the size types,
/// real-time transfer formats, connection types and priorities the connection supports, per
/// direction. `0x44440405` is fixed sizes both ways (bits 0, 2), an O->T 32-bit header (bits
/// 8-10), modeless T->O (bits 12-14), point-to-point both ways (bits 18, 22) and scheduled
/// priority both ways (bits 26, 30).
#[bitsize(32)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct ConnectionParameters {
    /// Bits 0-1
    pub o2t_size_types: SizeTypes,
    /// Bits 2-3
    pub t2o_size_types: SizeTypes,
    obsolete_o2t_bytes_per_slot: u2,
    obsolete_t2o_bytes_per_slot: u2,
    /// Bits 8-10
    pub o2t_real_time_transfer_format: RealTimeTransferFormat,
    reserved: u1,
    /// Bits 12-14
    pub t2o_real_time_transfer_format: RealTimeTransferFormat,
    reserved: u1,
    /// Bits 16-19
    pub o2t_connection_types: ConnectionTypes,
    /// Bits 20-23
    pub t2o_connection_types: ConnectionTypes,
    /// Bits 24-27
    pub o2t_priorities: Priorities,
    /// Bits 28-31
    pub t2o_priorities: Priorities,
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
        let (o2t, t2o) = (self.o2t_priorities(), self.t2o_priorities());
        if o2t.scheduled() && t2o.scheduled() {
            Some(ConnectionPriority::Scheduled)
        } else if o2t.high() && t2o.high() {
            Some(ConnectionPriority::High)
        } else if o2t.low() && t2o.low() {
            Some(ConnectionPriority::Low)
        } else {
            None
        }
    }
}

// ^^^^^^^^ End of ConnectionParameters impl ^^^^^^^^

/// The connection size types one direction supports
#[bitsize(2)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct SizeTypes {
    pub fixed: bool,
    pub variable: bool,
}

/// How one direction's packets signal run/idle, by its 3-bit code
#[bitsize(3)]
#[derive(FromBits, Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum RealTimeTransferFormat {
    Modeless = 0,
    ZeroLength = 1,
    Heartbeat = 3,
    Header32Bit = 4,
    /// 2 (reserved) and 5 to 7
    #[fallback]
    Unsupported(u3),
}

/// The library's `RealTimeFormat` for a code this scanner supports
impl TryFrom<RealTimeTransferFormat> for RealTimeFormat {
    type Error = anyhow::Error;

    fn try_from(format: RealTimeTransferFormat) -> Result<RealTimeFormat> {
        match format {
            RealTimeTransferFormat::Modeless => Ok(RealTimeFormat::Modeless),
            RealTimeTransferFormat::ZeroLength => Ok(RealTimeFormat::ZeroLength),
            RealTimeTransferFormat::Heartbeat => Ok(RealTimeFormat::Heartbeat),
            RealTimeTransferFormat::Header32Bit => Ok(RealTimeFormat::Header32Bit),
            RealTimeTransferFormat::Unsupported(code) => {
                bail!("real-time transfer format {code} is not supported")
            }
        }
    }
}

/// The connection types one direction supports
#[bitsize(4)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct ConnectionTypes {
    pub null: bool,
    pub multicast: bool,
    pub point_to_point: bool,
    reserved: u1,
}

/// The priorities one direction supports
#[bitsize(4)]
#[derive(FromBits, DebugBits, BuilderBits, PartialEq, Clone, Copy)]
pub struct Priorities {
    pub low: bool,
    pub high: bool,
    pub scheduled: bool,
    reserved: u1,
}
