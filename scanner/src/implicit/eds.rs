//! A `ConnectionN` of a device's EDS file (read by `eds_parser`) as the Forward_Open that opens
//! it, or why this scanner cannot open it: it opens class 1, cyclic, exclusive-owner connections,
//! point-to-point both ways. The caller sets what an EDS does not describe:
//!
//! ```text
//! let connection: &Connection = &eds.connections[&first_openable_connection(&eds, None)?];
//! let request = ForwardOpenRequest {
//!     priority_time_tick: ...,
//!     timeout_ticks: 5,
//!     o2t_network_connection_id: 0,
//!     t2o_network_connection_id: 0x1234_5678,
//!     connection_triad: ConnectionTriad { ... },
//!     connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
//!     ..forward_open_request(&eds, connection, None)?
//! };
//! let (o2t, t2o) = (o2t_real_time_format(connection)?, t2o_real_time_format(connection)?);
//! forward_open(&mut session, request, o2t, t2o)
//! ```

use std::io::Cursor;

use bilge::prelude::{Integer, u9};
use binrw::BinRead;

use eds_parser::Eds;
use eds_parser::connection_manager::{Connection, ConnectionParameters, RealTimeTransferFormat};
use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;

use crate::error::{Error, Result};

/// The N of the first `ConnectionN` this scanner can open, or why each one cannot be opened;
/// `large` as for `forward_open_request`
pub fn first_openable_connection(eds: &Eds, large: Option<bool>) -> Result<u32> {
    if eds.connections.is_empty() {
        return Err(Error::UnsupportedConnection(
            "[Connection Manager]: the EDS has no ConnectionN entries".to_string(),
        ));
    }
    let mut refusals = String::new();
    for (number, connection) in &eds.connections {
        match forward_open_request(eds, connection, large) {
            Ok(_) => return Ok(*number),
            Err(refusal) => refusals.push_str(&format!("\n  Connection{number}: {refusal}")),
        }
    }
    Err(Error::UnsupportedConnection(format!(
        "[Connection Manager]: no connection this scanner can open:{refusals}"
    )))
}

/// The Forward_Open fields a connection decides: both RPIs, both network connection parameters,
/// the transport class and trigger, and the connection path. Every other field is zero, for the
/// caller to set. `large` picks a Large_Forward_Open (32-bit connection parameters) or a
/// Forward_Open (16-bit); with `None`, a Large_Forward_Open only when a direction's connection size
/// needs it. For this entry, with `Param1` defaulting to 1000000, `Param2` to 0x97 and `Assem100`
/// 32 bytes:
///
/// ```text
/// Connection1 = 0x04010002, 0x44440405, Param1,32,Assem150, Param1,,Assem100, ,, 0,Assem151,
///               "Exclusive Owner", "", "20 04 24 [Param2] 2C 96 2C 64";
/// ```
///
/// the fields are:
///
/// ```text
/// transport_type_trigger: class 1, cyclic, client
/// O->T: every 1000000 us; 38 bytes (32 + 2 sequence count + 4 run/idle header), fixed,
///       scheduled, point-to-point, exclusive owner
/// T->O: every 1000000 us; 34 bytes (Assem100's 32 + 2), fixed, scheduled, point-to-point,
///       exclusive owner
/// connection_path: 20 04 24 97 2C 96 2C 64 (class 4, instance 0x97, connection points 0x96
///       and 0x64)
/// ```
pub fn forward_open_request(
    eds: &Eds,
    connection: &Connection,
    large: Option<bool>,
) -> Result<ForwardOpenRequest> {
    let trigger_and_transport = connection.trigger_and_transport;
    let connection_parameters = connection.connection_parameters;

    // 0x04010002: class 1 (bit 1), cyclic (bit 16), exclusive owner (bit 26)
    if !(trigger_and_transport.transport_classes().class_1()
        && trigger_and_transport.triggers().cyclic()
        && trigger_and_transport.application_types().exclusive_owner())
    {
        return Err(Error::UnsupportedConnection(format!(
            "this scanner opens class 1, cyclic, exclusive-owner connections; this one offers {trigger_and_transport:?}"
        )));
    }
    // 0x44440405: point-to-point O->T (bit 18) and T->O (bit 22), scheduled priority O->T
    // (bit 26) and T->O (bit 30)
    if !(connection_parameters
        .o2t_connection_types()
        .point_to_point()
        && connection_parameters
            .t2o_connection_types()
            .point_to_point())
    {
        return Err(Error::UnsupportedConnection(format!(
            "this scanner opens point-to-point connections both ways; this one offers {connection_parameters:?}"
        )));
    }
    let priority: ConnectionPriority = shared_priority(connection_parameters).ok_or_else(|| {
        Error::UnsupportedConnection(format!(
            "no priority is supported both ways; this one offers {connection_parameters:?}"
        ))
    })?;
    let o2t_real_time_format: RealTimeFormat = o2t_real_time_format(connection)?;
    let t2o_real_time_format: RealTimeFormat = t2o_real_time_format(connection)?;

    // Param1,32,Assem150, (fields 3-5): O->T every Param1's default microseconds, 32 bytes
    let o2t_requested_packet_interval: CipUdint = eds
        .requested_packet_interval(connection.o2t_rpi)
        .map_err(|error| Error::Eds(error.context("O->T")))?;
    let o2t_size: u16 = eds
        .data_size(connection.o2t_size, connection.o2t_format)
        .map_err(|error| Error::Eds(error.context("O->T")))?;
    // Param1,,Assem100, (fields 6-8): T->O every Param1's default microseconds, as many bytes as
    // Assem100
    let t2o_requested_packet_interval: CipUdint = eds
        .requested_packet_interval(connection.t2o_rpi)
        .map_err(|error| Error::Eds(error.context("T->O")))?;
    let t2o_size: u16 = eds
        .data_size(connection.t2o_size, connection.t2o_format)
        .map_err(|error| Error::Eds(error.context("T->O")))?;

    // Sizes on the wire: the data plus the sequence count and the real-time header. O->T in the
    // example: 32 + 2 + 4 = 38; T->O: 32 + 2 = 34
    let o2t_connection_size: u16 =
        connection_size(o2t_size, TransportClass::Class1, o2t_real_time_format);
    let t2o_connection_size: u16 =
        connection_size(t2o_size, TransportClass::Class1, t2o_real_time_format);
    let large: bool =
        large.unwrap_or(o2t_connection_size.max(t2o_connection_size) > u9::MAX.value());
    let o2t_network_connection_parameters = network_connection_parameters(
        "O->T",
        o2t_connection_size,
        connection_parameters.o2t_size_types().fixed(),
        priority,
        large,
    )?;
    let t2o_network_connection_parameters = network_connection_parameters(
        "T->O",
        t2o_connection_size,
        connection_parameters.t2o_size_types().fixed(),
        priority,
        large,
    )?;

    // "20 04 24 [Param2] 2C 96 2C 64": Param2's default in place of [Param2]
    let path: Vec<u8> = eds.path_bytes(&connection.path)?;
    let Some(connection_path) = cip_path(&path) else {
        return Err(Error::UnsupportedConnection(format!(
            "its path {path:02X?} is not made of logical segments only"
        )));
    };

    Ok(ForwardOpenRequest {
        priority_time_tick: PriorityTimeTick::from(0u8),
        timeout_ticks: 0,
        o2t_network_connection_id: 0,
        t2o_network_connection_id: 0,
        connection_triad: ConnectionTriad::default(),
        connection_timeout_multiplier: ConnectionTimeoutMultiplier::default(),
        o2t_requested_packet_interval,
        o2t_network_connection_parameters,
        t2o_requested_packet_interval,
        t2o_network_connection_parameters,
        transport_type_trigger: TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class1)
            .production_trigger(ProductionTrigger::Cyclic)
            .direction(Direction::Client)
            .build(),
        connection_path,
    })
}

/// A direction's point-to-point, exclusive-owner parameter word: 32-bit when `large`, else 16-bit,
/// which holds up to 511 bytes. The size is fixed when the direction supports it, else variable.
/// `("O->T", 38, true, Scheduled, false)` is the 16-bit word for 38 bytes, fixed, scheduled,
/// point-to-point, exclusive owner.
fn network_connection_parameters(
    direction: &str,
    connection_size: u16,
    fixed_size_supported: bool,
    priority: ConnectionPriority,
    large: bool,
) -> Result<NetworkConnectionParameters> {
    let connection_size_type = match fixed_size_supported {
        true => ConnectionSizeType::Fixed,
        false => ConnectionSizeType::Variable,
    };
    if large {
        return Ok(NetworkConnectionParameters::Large(
            LargeNetworkConnectionParameters::builder()
                .connection_size(connection_size)
                .connection_size_type(connection_size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        ));
    }
    let standard_connection_size = u9::try_new(connection_size).map_err(|_| {
        Error::UnsupportedConnection(format!(
            "{direction}: the connection size of {connection_size} bytes needs a Large_Forward_Open"
        ))
    })?;
    Ok(NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(standard_connection_size)
            .connection_size_type(connection_size_type)
            .priority(priority)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    ))
}

/// Path bytes as a `CipPath`; `None` unless they are whole 16-bit words of logical segments.
/// `20 04 24 97 2C 96 2C 64` is class 4, instance 0x97, connection points 0x96 and 0x64.
fn cip_path(bytes: &[u8]) -> Option<CipPath> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let words = u8::try_from(bytes.len() / 2).ok()?;
    CipPath::read_args(&mut Cursor::new(bytes), (words,)).ok()
}

/// How the outputs signal run/idle: `0x44440405` is `Header32Bit` (bits 8-10)
pub fn o2t_real_time_format(connection: &Connection) -> Result<RealTimeFormat> {
    real_time_format(
        "O->T",
        connection
            .connection_parameters
            .o2t_real_time_transfer_format(),
    )
}

/// How the inputs signal run/idle: `0x44440405` is `Modeless` (bits 12-14)
pub fn t2o_real_time_format(connection: &Connection) -> Result<RealTimeFormat> {
    real_time_format(
        "T->O",
        connection
            .connection_parameters
            .t2o_real_time_transfer_format(),
    )
}

/// The library's `RealTimeFormat` for a code this scanner supports
fn real_time_format(direction: &str, format: RealTimeTransferFormat) -> Result<RealTimeFormat> {
    match format {
        RealTimeTransferFormat::Modeless => Ok(RealTimeFormat::Modeless),
        RealTimeTransferFormat::ZeroLength => Ok(RealTimeFormat::ZeroLength),
        RealTimeTransferFormat::Heartbeat => Ok(RealTimeFormat::Heartbeat),
        RealTimeTransferFormat::Header32Bit => Ok(RealTimeFormat::Header32Bit),
        RealTimeTransferFormat::Reserved(code) => Err(Error::UnsupportedConnection(format!(
            "{direction}: real-time transfer format {code} is not supported"
        ))),
    }
}

/// The best priority both directions support: scheduled, else high, else low. `0x44440405`
/// (scheduled only, both ways) is `Scheduled`; `0x3775_0405` (T->O low and high) is `High`.
fn shared_priority(connection_parameters: ConnectionParameters) -> Option<ConnectionPriority> {
    let (o2t, t2o) = (
        connection_parameters.o2t_priorities(),
        connection_parameters.t2o_priorities(),
    );
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
