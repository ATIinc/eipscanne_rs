//! Step 4: from what the EDS says the device supports to the Forward_Open this scanner sends.
//!
//! The masks of a connection list what the device *supports*; the bridge picks one value for each
//! field of the request, and refuses the connection when the device does not support what this
//! scanner does (class 1, cyclic, exclusive owner, point-to-point). It returns exactly what
//! `scanner::implicit::forward_open` takes: the request and the real-time format of each
//! direction, which the EDS declares but the Forward_Open does not carry.

use bilge::prelude::{u3, u9};

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipUdint, CipUsint};

use crate::connection::{Connection, ConnectionParameters, DirectionSpec};
use crate::error::BridgeError;

/// What an EDS does not describe: who the originator is and how it wants the connection run
#[derive(Debug, Clone, PartialEq)]
pub struct OriginatorSettings {
    /// Tick length and priority of the Forward_Open itself
    pub priority_time_tick: PriorityTimeTick,
    /// Ticks until the Forward_Open (and the Forward_Close) times out
    pub timeout_ticks: CipUsint,
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,
    /// The T->O connection ID this scanner chooses
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    /// Send a Large_Forward_Open (32-bit connection parameters) instead of a Forward_Open
    pub large_forward_open: bool,
}

/// Bit of the transport classes word for class 1
const CLASS_1: u16 = 1 << 1;

/// Path segment bytes of the one path shape the bridge accepts:
/// `20 04 24 cc 2C oo 2C tt` (8-bit class, instance and two connection points)
const CLASS_SEGMENT_8BIT: u8 = 0x20;
const INSTANCE_SEGMENT_8BIT: u8 = 0x24;
const CONNECTION_POINT_SEGMENT_8BIT: u8 = 0x2C;

/// Turns one connection of an EDS into the Forward_Open of a class 1, cyclic, exclusive-owner,
/// point-to-point connection, followed by the O->T and T->O real-time formats
pub fn to_forward_open(
    connection: &Connection,
    originator: OriginatorSettings,
) -> Result<(ForwardOpenRequest, RealTimeFormat, RealTimeFormat), BridgeError> {
    let unsupported = |what: &str| BridgeError::Unsupported {
        connection: connection.keyword.clone(),
        what: what.to_string(),
    };

    let masks = &connection.trigger_and_transport;
    if masks.transport_classes() & CLASS_1 == 0 {
        return Err(unsupported("transport class 1 is not supported"));
    }
    if !masks.cyclic() {
        return Err(unsupported("a cyclic trigger is not supported"));
    }
    if !masks.exclusive_owner() {
        return Err(unsupported(
            "not an exclusive-owner connection (input-only and listen-only are out of scope)",
        ));
    }
    // The client/server bit is ignored on purpose: vendors disagree on it, and the originator
    // of a Forward_Open is always the client

    let parameters = &connection.connection_parameters;
    if !parameters.o2t_point_to_point() {
        return Err(unsupported("O->T point-to-point is not supported"));
    }
    if !parameters.t2o_point_to_point() {
        return Err(unsupported("T->O point-to-point is not supported"));
    }

    let priority = shared_priority(parameters).ok_or_else(|| {
        unsupported("no priority (scheduled, high or low) is supported by both directions")
    })?;

    let (configuration_instance, o2t_connection_point, t2o_connection_point) =
        assembly_instances(&connection.path).ok_or_else(|| BridgeError::UnsupportedPath {
            connection: connection.keyword.clone(),
            path: connection.path.clone(),
        })?;

    let o2t_real_time_format =
        real_time_format(connection, "O->T", parameters.o2t_real_time_format())?;
    let t2o_real_time_format =
        real_time_format(connection, "T->O", parameters.t2o_real_time_format())?;

    // Configuration data the EDS declares is ignored: configuration data segments are out of
    // scope, and adapters accept the Forward_Open without them

    let request = ForwardOpenRequest {
        priority_time_tick: originator.priority_time_tick,
        timeout_ticks: originator.timeout_ticks,
        // Point-to-point: the adapter chooses the O->T connection ID and returns it
        o2t_network_connection_id: 0,
        t2o_network_connection_id: originator.t2o_network_connection_id,
        connection_triad: originator.connection_triad,
        connection_timeout_multiplier: originator.connection_timeout_multiplier,
        o2t_requested_packet_interval: requested_packet_interval(
            connection,
            &connection.o2t,
            "O->T",
        )?,
        o2t_network_connection_parameters: network_connection_parameters(
            connection,
            &connection.o2t,
            "O->T",
            o2t_real_time_format,
            parameters.o2t_fixed_size(),
            priority,
            originator.large_forward_open,
        )?,
        t2o_requested_packet_interval: requested_packet_interval(
            connection,
            &connection.t2o,
            "T->O",
        )?,
        t2o_network_connection_parameters: network_connection_parameters(
            connection,
            &connection.t2o,
            "T->O",
            t2o_real_time_format,
            parameters.t2o_fixed_size(),
            priority,
            originator.large_forward_open,
        )?,
        transport_type_trigger: TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class1)
            .production_trigger(ProductionTrigger::Cyclic)
            .direction(Direction::Client)
            .build(),
        connection_path: CipPath::new_assembly_connection(
            configuration_instance,
            o2t_connection_point,
            t2o_connection_point,
        ),
    };

    Ok((request, o2t_real_time_format, t2o_real_time_format))
}

/// The real-time format the EDS declares for a direction
fn real_time_format(
    connection: &Connection,
    direction: &'static str,
    real_time_format: u3,
) -> Result<RealTimeFormat, BridgeError> {
    match u8::from(real_time_format) {
        0 => Ok(RealTimeFormat::Modeless),
        1 => Ok(RealTimeFormat::ZeroLength),
        3 => Ok(RealTimeFormat::Heartbeat),
        4 => Ok(RealTimeFormat::Header32Bit),
        other => Err(BridgeError::Unsupported {
            connection: connection.keyword.clone(),
            what: format!("{direction} real-time format {other} is not supported"),
        }),
    }
}

/// The resolved RPI of a direction, in microseconds
fn requested_packet_interval(
    connection: &Connection,
    spec: &DirectionSpec,
    direction: &'static str,
) -> Result<CipUdint, BridgeError> {
    spec.requested_packet_interval
        .ok_or_else(|| BridgeError::MissingRpi {
            connection: connection.keyword.clone(),
            direction,
        })
}

/// The parameter word of a direction: the EDS size plus the sequence count and real-time header
/// (EDS sizes exclude both), fixed size when supported (variable otherwise), in the width of the
/// service
fn network_connection_parameters(
    connection: &Connection,
    spec: &DirectionSpec,
    direction: &'static str,
    real_time_format: RealTimeFormat,
    fixed_size_supported: bool,
    priority: ConnectionPriority,
    large_forward_open: bool,
) -> Result<NetworkConnectionParameters, BridgeError> {
    let size = connection_size(spec.size, TransportClass::Class1, real_time_format);
    let size_type = if fixed_size_supported {
        ConnectionSizeType::Fixed
    } else {
        ConnectionSizeType::Variable
    };

    if large_forward_open {
        return Ok(NetworkConnectionParameters::Large(
            LargeNetworkConnectionParameters::builder()
                .connection_size(size)
                .connection_size_type(size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        ));
    }
    let size = u9::try_new(size).map_err(|_| BridgeError::ConnectionSizeTooLarge {
        connection: connection.keyword.clone(),
        direction,
        size,
    })?;
    Ok(NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(size)
            .connection_size_type(size_type)
            .priority(priority)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    ))
}

/// The best priority both directions support: scheduled, else high, else low
fn shared_priority(parameters: &ConnectionParameters) -> Option<ConnectionPriority> {
    if parameters.o2t_scheduled_priority() && parameters.t2o_scheduled_priority() {
        Some(ConnectionPriority::Scheduled)
    } else if parameters.o2t_high_priority() && parameters.t2o_high_priority() {
        Some(ConnectionPriority::High)
    } else if parameters.o2t_low_priority() && parameters.t2o_low_priority() {
        Some(ConnectionPriority::Low)
    } else {
        None
    }
}

/// The configuration instance and the two connection points of a path to the Assembly object
/// made of 8-bit segments, in that order; `None` for any other path
fn assembly_instances(path: &[u8]) -> Option<(u8, u8, u8)> {
    match *path {
        [
            CLASS_SEGMENT_8BIT,
            ASSEMBLY_CLASS_ID,
            INSTANCE_SEGMENT_8BIT,
            configuration,
            CONNECTION_POINT_SEGMENT_8BIT,
            o2t,
            CONNECTION_POINT_SEGMENT_8BIT,
            t2o,
        ] => Some((configuration, o2t, t2o)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::{ConfigurationData, TriggerAndTransport};

    use super::*;

    /// Class 1, cyclic, exclusive owner
    const TRIGGER_AND_TRANSPORT: u32 = 0x0401_0002;
    /// Fixed sizes both ways, O->T 32-bit header, T->O modeless, point-to-point both ways, every
    /// priority both ways
    const CONNECTION_PARAMETERS: u32 = 0x7775_0405;
    const PATH: [u8; 8] = [0x20, 0x04, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64];

    fn connection(trigger_and_transport: u32, connection_parameters: u32) -> Connection {
        Connection {
            keyword: "Connection1".to_string(),
            name: "Exclusive owner".to_string(),
            trigger_and_transport: TriggerAndTransport::from(trigger_and_transport),
            connection_parameters: ConnectionParameters::from(connection_parameters),
            o2t: DirectionSpec {
                requested_packet_interval: Some(1_000_000),
                size: 32,
                format: Some("Assem150".to_string()),
            },
            t2o: DirectionSpec {
                requested_packet_interval: Some(1_000_000),
                size: 32,
                format: Some("Assem100".to_string()),
            },
            configuration: ConfigurationData::default(),
            path: PATH.to_vec(),
        }
    }

    fn originator() -> OriginatorSettings {
        OriginatorSettings {
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(bilge::prelude::u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            t2o_network_connection_id: 0x1234_5678,
            connection_triad: ConnectionTriad {
                connection_serial_number: 1,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            large_forward_open: false,
        }
    }

    type Bridged = Result<(ForwardOpenRequest, RealTimeFormat, RealTimeFormat), BridgeError>;

    fn bridge(trigger_and_transport: u32, connection_parameters: u32) -> Bridged {
        to_forward_open(
            &connection(trigger_and_transport, connection_parameters),
            originator(),
        )
    }

    fn unsupported(what_contains: &str, result: Bridged) {
        match result {
            Err(BridgeError::Unsupported { connection, what }) => {
                assert_eq!(connection, "Connection1");
                assert!(what.contains(what_contains), "{what}");
            }
            other => panic!("expected Unsupported({what_contains}), got {other:?}"),
        }
    }

    /// A 16-bit parameter word, as the bridge builds it
    fn standard(
        connection_size: u16,
        connection_size_type: ConnectionSizeType,
        priority: ConnectionPriority,
    ) -> NetworkConnectionParameters {
        NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(connection_size))
                .connection_size_type(connection_size_type)
                .priority(priority)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        )
    }

    #[test]
    fn the_opener_connection_maps_field_by_field() {
        let (request, o2t_real_time_format, t2o_real_time_format) =
            bridge(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS).unwrap();

        assert_eq!(
            request,
            ForwardOpenRequest {
                priority_time_tick: originator().priority_time_tick,
                timeout_ticks: 5,
                o2t_network_connection_id: 0,
                t2o_network_connection_id: 0x1234_5678,
                connection_triad: originator().connection_triad,
                connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
                o2t_requested_packet_interval: 1_000_000,
                // 32 bytes, the sequence count and the 32-bit header
                o2t_network_connection_parameters: standard(
                    38,
                    ConnectionSizeType::Fixed,
                    ConnectionPriority::Scheduled
                ),
                t2o_requested_packet_interval: 1_000_000,
                // 32 bytes and the sequence count
                t2o_network_connection_parameters: standard(
                    34,
                    ConnectionSizeType::Fixed,
                    ConnectionPriority::Scheduled
                ),
                transport_type_trigger: TransportTypeTrigger::builder()
                    .transport_class(TransportClass::Class1)
                    .production_trigger(ProductionTrigger::Cyclic)
                    .direction(Direction::Client)
                    .build(),
                connection_path: CipPath::new_assembly_connection(0x97, 0x96, 0x64),
            }
        );
        assert_eq!(o2t_real_time_format, RealTimeFormat::Header32Bit);
        assert_eq!(t2o_real_time_format, RealTimeFormat::Modeless);
    }

    #[test]
    fn transport_class_trigger_and_application_type_must_match() {
        unsupported("class 1", bridge(0x0401_0001, CONNECTION_PARAMETERS)); // class 0 only
        unsupported("cyclic", bridge(0x0402_0002, CONNECTION_PARAMETERS)); // change of state only
        unsupported(
            "exclusive-owner",
            bridge(0x0201_0002, CONNECTION_PARAMETERS),
        ); // input only
    }

    #[test]
    fn the_client_server_bit_is_ignored() {
        assert!(bridge(TRIGGER_AND_TRANSPORT | 0x8000_0000, CONNECTION_PARAMETERS).is_ok());
    }

    #[test]
    fn both_directions_must_support_point_to_point() {
        unsupported(
            "O->T point-to-point",
            bridge(TRIGGER_AND_TRANSPORT, 0x7771_0405),
        ); // bit 18 off
        unsupported(
            "T->O point-to-point",
            bridge(TRIGGER_AND_TRANSPORT, 0x7735_0405),
        ); // bit 22 off
    }

    #[test]
    fn priority_is_the_best_one_both_directions_support() {
        let priority = |word: u32| {
            let (request, _, _) = bridge(TRIGGER_AND_TRANSPORT, word).unwrap();
            let NetworkConnectionParameters::Standard(o2t) =
                request.o2t_network_connection_parameters
            else {
                panic!("a Forward_Open has 16-bit parameters");
            };
            o2t.priority()
        };

        assert_eq!(priority(0x7775_0405), ConnectionPriority::Scheduled);
        assert_eq!(priority(0x3775_0405), ConnectionPriority::High); // T->O: low + high
        assert_eq!(priority(0x1375_0405), ConnectionPriority::Low); // T->O low, O->T low + high
        unsupported("priority", bridge(TRIGGER_AND_TRANSPORT, 0x1275_0405)); // T->O low, O->T high
    }

    #[test]
    fn size_type_is_fixed_when_supported_else_variable() {
        let (request, _, _) = bridge(TRIGGER_AND_TRANSPORT, 0x7775_040A).unwrap(); // variable only
        assert_eq!(
            request.o2t_network_connection_parameters,
            standard(
                38,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            )
        );
        assert_eq!(
            request.t2o_network_connection_parameters,
            standard(
                34,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            )
        );
    }

    #[test]
    fn real_time_formats_map_or_are_refused() {
        let formats = |word: u32| {
            let (_, o2t, t2o) = bridge(TRIGGER_AND_TRANSPORT, word).unwrap();
            (o2t, t2o)
        };

        assert_eq!(
            formats(0x7775_0005),
            (RealTimeFormat::Modeless, RealTimeFormat::Modeless)
        );
        assert_eq!(
            formats(0x7775_1105),
            (RealTimeFormat::ZeroLength, RealTimeFormat::ZeroLength)
        );
        assert_eq!(
            formats(0x7775_3305),
            (RealTimeFormat::Heartbeat, RealTimeFormat::Heartbeat)
        );
        assert_eq!(
            formats(0x7775_4405),
            (RealTimeFormat::Header32Bit, RealTimeFormat::Header32Bit)
        );
        unsupported(
            "O->T real-time format 2",
            bridge(TRIGGER_AND_TRANSPORT, 0x7775_0205),
        );
        unsupported(
            "T->O real-time format 5",
            bridge(TRIGGER_AND_TRANSPORT, 0x7775_5405),
        );
    }

    #[test]
    fn a_direction_without_rpi_is_missing_rpi() {
        let mut connection = connection(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS);
        connection.t2o.requested_packet_interval = None;

        assert_eq!(
            to_forward_open(&connection, originator()),
            Err(BridgeError::MissingRpi {
                connection: "Connection1".to_string(),
                direction: "T->O",
            })
        );
    }

    #[test]
    fn only_eight_bit_assembly_paths_are_accepted() {
        let with_path = |path: &[u8]| {
            let mut connection = connection(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS);
            connection.path = path.to_vec();
            to_forward_open(&connection, originator())
        };
        let rejected = |path: &[u8]| {
            assert_eq!(
                with_path(path),
                Err(BridgeError::UnsupportedPath {
                    connection: "Connection1".to_string(),
                    path: path.to_vec(),
                })
            );
        };

        assert!(with_path(&PATH).is_ok());
        rejected(&[0x21, 0x00, 0x04, 0x00, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64]); // 16-bit class
        rejected(&[0x20, 0x05, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64]); // another class
        rejected(&[
            0x20, 0x04, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64, 0x80, 0x01, 0x00, 0x00,
        ]); // extra segment
        rejected(&[0x20, 0x04, 0x24, 0x97, 0x2C, 0x96]); // no T->O point
    }

    #[test]
    fn a_connection_size_over_nine_bits_needs_a_large_forward_open() {
        let mut connection = connection(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS);
        connection.t2o.size = 510; // 512 with the sequence count

        assert_eq!(
            to_forward_open(&connection, originator()),
            Err(BridgeError::ConnectionSizeTooLarge {
                connection: "Connection1".to_string(),
                direction: "T->O",
                size: 512,
            })
        );

        let mut originator = originator();
        originator.large_forward_open = true;
        let (request, _, _) = to_forward_open(&connection, originator).unwrap();
        assert!(matches!(
            request.t2o_network_connection_parameters,
            NetworkConnectionParameters::Large(parameters) if parameters.connection_size() == 512
        ));
    }
}
