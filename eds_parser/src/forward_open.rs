//! A `ConnectionN` entry as the fields of the Forward_Open that opens it, or the reason this
//! scanner cannot open it: it opens class 1, cyclic, exclusive-owner connections, point-to-point
//! both ways.

use std::io::Cursor;

use anyhow::{Context, Result, bail};
use bilge::prelude::{u4, u9};
use binrw::BinRead;

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

use crate::Eds;
use crate::connection_manager::Connection;

/// The largest connection size the 16-bit parameters of a Forward_Open hold
const MAX_STANDARD_CONNECTION_SIZE: u16 = 511;

/// The Forward_Open fields a connection decides, every other field zero; `large` as for
/// `Eds::forward_open_request`. For this entry, with `Param1` defaulting to 1000000, `Param2` to
/// 0x97 and `Assem100` 32 bytes:
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
pub(crate) fn request(
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
        bail!(
            "this scanner opens class 1, cyclic, exclusive-owner connections; this one offers {trigger_and_transport:?}"
        );
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
        bail!(
            "this scanner opens point-to-point connections both ways; this one offers {connection_parameters:?}"
        );
    }
    let priority: ConnectionPriority =
        connection_parameters.shared_priority().with_context(|| {
            format!("no priority is supported both ways; this one offers {connection_parameters:?}")
        })?;
    let o2t_real_time_format: RealTimeFormat = connection.o2t_real_time_format()?;
    let t2o_real_time_format: RealTimeFormat = connection.t2o_real_time_format()?;

    // Param1,32,Assem150, (fields 3-5): O->T every Param1's default microseconds, 32 bytes
    let o2t_requested_packet_interval: CipUdint = eds
        .requested_packet_interval(connection.o2t_rpi)
        .context("O->T")?;
    let o2t_size: u16 = eds
        .data_size(connection.o2t_size, connection.o2t_format)
        .context("O->T")?;
    // Param1,,Assem100, (fields 6-8): T->O every Param1's default microseconds, as many bytes as
    // Assem100
    let t2o_requested_packet_interval: CipUdint = eds
        .requested_packet_interval(connection.t2o_rpi)
        .context("T->O")?;
    let t2o_size: u16 = eds
        .data_size(connection.t2o_size, connection.t2o_format)
        .context("T->O")?;

    // Sizes on the wire: the data plus the sequence count and the real-time header. O->T in the
    // example: 32 + 2 + 4 = 38; T->O: 32 + 2 = 34
    let o2t_connection_size: u16 =
        connection_size(o2t_size, TransportClass::Class1, o2t_real_time_format);
    let t2o_connection_size: u16 =
        connection_size(t2o_size, TransportClass::Class1, t2o_real_time_format);
    let large: bool = large
        .unwrap_or(o2t_connection_size.max(t2o_connection_size) > MAX_STANDARD_CONNECTION_SIZE);
    let o2t_network_connection_parameters = network_connection_parameters(
        o2t_connection_size,
        connection_parameters.o2t_size_types().fixed(),
        priority,
        large,
    )
    .with_context(|| {
        format!(
            "the O->T connection size of {o2t_connection_size} bytes needs a Large_Forward_Open"
        )
    })?;
    let t2o_network_connection_parameters = network_connection_parameters(
        t2o_connection_size,
        connection_parameters.t2o_size_types().fixed(),
        priority,
        large,
    )
    .with_context(|| {
        format!(
            "the T->O connection size of {t2o_connection_size} bytes needs a Large_Forward_Open"
        )
    })?;

    // "20 04 24 [Param2] 2C 96 2C 64": Param2's default in place of [Param2]
    let path: Vec<u8> = eds.path(&connection.path)?;
    let connection_path: CipPath = cip_path(&path)
        .with_context(|| format!("its path {path:02X?} is not made of logical segments only"))?;

    Ok(ForwardOpenRequest {
        priority_time_tick: PriorityTimeTick::builder()
            .tick_time(u4::new(0))
            .priority(false)
            .build(),
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

/// A point-to-point, exclusive-owner parameter word: 32-bit when `large`, else 16-bit, which
/// `None` means `connection_size` does not fit. The size is fixed when the direction supports it,
/// else variable. `(38, true, Scheduled, false)` is the 16-bit word for 38 bytes, fixed,
/// scheduled, point-to-point, exclusive owner.
fn network_connection_parameters(
    connection_size: u16,
    fixed_size_supported: bool,
    priority: ConnectionPriority,
    large: bool,
) -> Option<NetworkConnectionParameters> {
    let connection_size_type = match fixed_size_supported {
        true => ConnectionSizeType::Fixed,
        false => ConnectionSizeType::Variable,
    };
    if large {
        return Some(NetworkConnectionParameters::Large(
            LargeNetworkConnectionParameters::builder()
                .connection_size(connection_size)
                .connection_size_type(connection_size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        ));
    }
    Some(NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::try_new(connection_size).ok()?)
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
