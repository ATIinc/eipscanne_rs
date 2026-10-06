//! Parameters shared by the services that open a connection.

use bilge::prelude::{
    BuilderBits, DebugBits, DefaultBits, FromBits, Integer, bitsize, u1, u2, u3, u4, u9,
};
use binrw::{BinRead, BinWrite, binrw};

use crate::cip::types::CipUsint;

/// Whether every packet carries exactly the connection size or at most the connection size
/// (Connection Size Type in Wireshark)
#[bitsize(1)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum ConnectionSizeType {
    #[default]
    Fixed = 0,
    Variable = 1,
}

#[bitsize(2)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum ConnectionPriority {
    #[default]
    Low = 0,
    High = 1,
    Scheduled = 2,
    Urgent = 3,
}

#[bitsize(2)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum ConnectionType {
    /// No data flows in this direction (also used to reconfigure an existing connection)
    #[default]
    Null = 0,
    Multicast = 1,
    PointToPoint = 2,
    Reserved = 3,
}

/// Whether more than one originator may own the target at the same time
#[bitsize(1)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum RedundantOwner {
    #[default]
    Exclusive = 0,
    Redundant = 1,
}

/// Network Connection Parameters of a Forward_Open (16 bits)
#[bitsize(16)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct StandardNetworkConnectionParameters {
    /// Bytes per packet, including the sequence count and real-time header (see [`connection_size`])
    pub connection_size: u9,
    pub connection_size_type: ConnectionSizeType,
    pub priority: ConnectionPriority,
    reserved: u1,
    pub connection_type: ConnectionType,
    pub redundant_owner: RedundantOwner,
}

/// Network Connection Parameters of a Large_Forward_Open (32 bits)
#[bitsize(32)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u32::into)]
#[bw(map = |&x| u32::from(x))]
pub struct LargeNetworkConnectionParameters {
    /// Bytes per packet, including the sequence count and real-time header (see [`connection_size`])
    pub connection_size: u16,
    reserved: u9,
    pub connection_size_type: ConnectionSizeType,
    pub priority: ConnectionPriority,
    reserved: u1,
    pub connection_type: ConnectionType,
    pub redundant_owner: RedundantOwner,
}

/// Network Connection Parameters in the width of the service that carries them:
/// 16 bits for Forward_Open, 32 bits for Large_Forward_Open
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Clone, Copy)]
#[br(import(large: bool))]
pub enum NetworkConnectionParameters {
    #[br(pre_assert(!large))]
    Standard(StandardNetworkConnectionParameters),

    #[br(pre_assert(large))]
    Large(LargeNetworkConnectionParameters),
}

#[bitsize(4)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum TransportClass {
    #[default]
    Class0 = 0,
    Class1 = 1,
    Class2 = 2,
    Class3 = 3,

    #[fallback]
    Unknown(u4),
}

/// What makes the producing end point send a packet
#[bitsize(3)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum ProductionTrigger {
    #[default]
    Cyclic = 0,
    ChangeOfState = 1,
    ApplicationObject = 2,

    #[fallback]
    Unknown(u3),
}

/// Which side of the connection this end point plays
#[bitsize(1)]
#[derive(FromBits, PartialEq, Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Client = 0,
    Server = 1,
}

/// Transport Type/Trigger byte of a Forward_Open
#[bitsize(8)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u8::into)]
#[bw(map = |&x| u8::from(x))]
pub struct TransportTypeTrigger {
    pub transport_class: TransportClass,
    pub production_trigger: ProductionTrigger,
    pub direction: Direction,
}

/// Priority/Time_tick byte: the length of one tick of the request timeout
/// (1 ms shifted left by `tick_time`) and the priority of the unconnected request
#[bitsize(8)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u8::into)]
#[bw(map = |&x| u8::from(x))]
pub struct PriorityTimeTick {
    pub tick_time: u4,
    pub priority: bool,
    reserved: u3,
}

/// Connection Timeout Multiplier: the connection times out after this many packet intervals
/// without data. Values of 8 and above are reserved and kept as `Unknown`.
#[binrw]
#[brw(little)]
#[derive(Debug, PartialEq, Copy, Clone, Default)]
pub enum ConnectionTimeoutMultiplier {
    #[default]
    #[brw(magic = 0u8)]
    X4,
    #[brw(magic = 1u8)]
    X8,
    #[brw(magic = 2u8)]
    X16,
    #[brw(magic = 3u8)]
    X32,
    #[brw(magic = 4u8)]
    X64,
    #[brw(magic = 5u8)]
    X128,
    #[brw(magic = 6u8)]
    X256,
    #[brw(magic = 7u8)]
    X512,
    Unknown(CipUsint),
}

// ======= Start of ConnectionTimeoutMultiplier impl ========

impl ConnectionTimeoutMultiplier {
    /// The factor applied to the packet interval to get the connection timeout.
    ///
    /// Reserved values follow the same rule as the defined ones (4 doubled once per step) and
    /// saturate once the result no longer fits.
    pub fn multiplier(&self) -> u32 {
        let value = match self {
            ConnectionTimeoutMultiplier::X4 => 0,
            ConnectionTimeoutMultiplier::X8 => 1,
            ConnectionTimeoutMultiplier::X16 => 2,
            ConnectionTimeoutMultiplier::X32 => 3,
            ConnectionTimeoutMultiplier::X64 => 4,
            ConnectionTimeoutMultiplier::X128 => 5,
            ConnectionTimeoutMultiplier::X256 => 6,
            ConnectionTimeoutMultiplier::X512 => 7,
            ConnectionTimeoutMultiplier::Unknown(value) => *value,
        };
        1u32.checked_shl(u32::from(value) + 2).unwrap_or(u32::MAX)
    }
}

// ^^^^^^^^ End of ConnectionTimeoutMultiplier impl ^^^^^^^^

/// How the packets of a class 0/1 connection signal run/idle
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum RealTimeFormat {
    /// Application data only, no run/idle notification
    #[default]
    Modeless,
    /// Idle is signalled by sending no application data
    ZeroLength,
    /// Never carries application data
    Heartbeat,
    /// A 32-bit header with the run/idle flag precedes the application data
    Header32Bit,
}

// ======= Start of RealTimeFormat impl ========

impl RealTimeFormat {
    /// Number of bytes the real-time header takes in front of the application data
    pub fn header_len(&self) -> u16 {
        match self {
            RealTimeFormat::Header32Bit => 4,
            RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => 0,
        }
    }
}

// ^^^^^^^^ End of RealTimeFormat impl ^^^^^^^^

/// Connection size as sent on the wire: the application data, the 16-bit sequence count that
/// transport classes 1, 2 and 3 prepend to every packet, and the real-time header, if any
pub fn connection_size(
    data_size: u16,
    transport_class: TransportClass,
    real_time_format: RealTimeFormat,
) -> u32 {
    let sequence_count_len: u32 = match transport_class {
        TransportClass::Class1 | TransportClass::Class2 | TransportClass::Class3 => 2,
        TransportClass::Class0 | TransportClass::Unknown(_) => 0,
    };
    u32::from(data_size) + sequence_count_len + u32::from(real_time_format.header_len())
}

/// What the caller asks for in one direction of the connection
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct ConnectionDirection {
    pub connection_type: ConnectionType,
    pub priority: ConnectionPriority,
    pub connection_size_type: ConnectionSizeType,
    pub redundant_owner: RedundantOwner,
    /// Application data bytes per packet, without the sequence count and the real-time header
    pub data_size: u16,
    pub real_time_format: RealTimeFormat,
}

/// A connection size that does not fit the parameter width of the chosen service
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ConnectionSizeError {
    pub connection_size: u32,
    pub maximum: u32,
}

// ======= Start of NetworkConnectionParameters impl ========

impl NetworkConnectionParameters {
    /// The parameters of one direction in the width of the chosen service: the flags as
    /// requested and the connection size computed from the application data size
    pub fn new(
        direction: &ConnectionDirection,
        transport_class: TransportClass,
        large: bool,
    ) -> Result<Self, ConnectionSizeError> {
        let size = connection_size(
            direction.data_size,
            transport_class,
            direction.real_time_format,
        );

        if large {
            let connection_size = u16::try_from(size).map_err(|_| ConnectionSizeError {
                connection_size: size,
                maximum: u32::from(u16::MAX),
            })?;
            return Ok(NetworkConnectionParameters::Large(
                LargeNetworkConnectionParameters::builder()
                    .connection_size(connection_size)
                    .connection_size_type(direction.connection_size_type)
                    .priority(direction.priority)
                    .connection_type(direction.connection_type)
                    .redundant_owner(direction.redundant_owner)
                    .build(),
            ));
        }

        let maximum = u32::from(u9::MAX.value());
        if size > maximum {
            return Err(ConnectionSizeError {
                connection_size: size,
                maximum,
            });
        }
        Ok(NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(size as u16))
                .connection_size_type(direction.connection_size_type)
                .priority(direction.priority)
                .connection_type(direction.connection_type)
                .redundant_owner(direction.redundant_owner)
                .build(),
        ))
    }
}

// ^^^^^^^^ End of NetworkConnectionParameters impl ^^^^^^^^
