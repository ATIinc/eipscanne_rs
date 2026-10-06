//! Step 4: from what the EDS says the device supports to what this scanner will ask for.
//!
//! The masks of a connection list what the device *supports*; the bridge picks one value for each
//! `ConnectionConfig` field, and refuses the connection when the device does not support what
//! this scanner does (class 1, cyclic, exclusive owner, point-to-point).

use bilge::prelude::u3;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ProductionTrigger,
    RealTimeFormat, TransportClass,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::types::CipUdint;
use scanner::implicit::{ConnectionConfig, DirectionConfig};

use crate::connection::{Connection, ConnectionParameters, DirectionSpec};
use crate::error::BridgeError;

/// What an EDS does not describe: who the originator is and how it wants the connection run
#[derive(Debug, Clone, PartialEq)]
pub struct OriginatorSettings {
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,
    /// The T->O connection ID this scanner chooses
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    pub large_forward_open: bool,
}

/// Bit of the transport classes word for class 1
const CLASS_1: u16 = 1 << 1;

/// Path segment bytes of the one path shape the bridge accepts:
/// `20 04 24 cc 2C oo 2C tt` (8-bit class, instance and two connection points)
const CLASS_SEGMENT_8BIT: u8 = 0x20;
const INSTANCE_SEGMENT_8BIT: u8 = 0x24;
const CONNECTION_POINT_SEGMENT_8BIT: u8 = 0x2C;

/// Turns one connection of an EDS into the configuration of a class 1, cyclic, exclusive-owner,
/// point-to-point connection
pub fn to_connection_config(
    connection: &Connection,
    originator: OriginatorSettings,
) -> Result<ConnectionConfig, BridgeError> {
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

    // Configuration data the EDS declares is ignored: configuration data segments are out of
    // scope, and adapters accept the Forward_Open without them

    Ok(ConnectionConfig {
        configuration_instance,
        o2t: direction_config(
            connection,
            &connection.o2t,
            "O->T",
            o2t_connection_point,
            parameters.o2t_fixed_size(),
            parameters.o2t_real_time_format(),
        )?,
        t2o: direction_config(
            connection,
            &connection.t2o,
            "T->O",
            t2o_connection_point,
            parameters.t2o_fixed_size(),
            parameters.t2o_real_time_format(),
        )?,
        transport_class: TransportClass::Class1,
        production_trigger: ProductionTrigger::Cyclic,
        priority,
        connection_timeout_multiplier: originator.connection_timeout_multiplier,
        t2o_network_connection_id: originator.t2o_network_connection_id,
        connection_triad: originator.connection_triad,
        large_forward_open: originator.large_forward_open,
    })
}

/// One direction: fixed size when supported (variable otherwise), the real-time format the
/// device declares, the resolved size and interval
fn direction_config(
    connection: &Connection,
    spec: &DirectionSpec,
    direction: &'static str,
    connection_point: u8,
    fixed_size_supported: bool,
    real_time_format: u3,
) -> Result<DirectionConfig, BridgeError> {
    let real_time_format = match u8::from(real_time_format) {
        0 => RealTimeFormat::Modeless,
        1 => RealTimeFormat::ZeroLength,
        3 => RealTimeFormat::Heartbeat,
        4 => RealTimeFormat::Header32Bit,
        other => {
            return Err(BridgeError::Unsupported {
                connection: connection.keyword.clone(),
                what: format!("{direction} real-time format {other} is not supported"),
            });
        }
    };
    let requested_packet_interval =
        spec.requested_packet_interval
            .ok_or_else(|| BridgeError::MissingRpi {
                connection: connection.keyword.clone(),
                direction,
            })?;

    Ok(DirectionConfig {
        connection_point,
        data_size: spec.size,
        requested_packet_interval,
        real_time_format,
        connection_size_type: if fixed_size_supported {
            ConnectionSizeType::Fixed
        } else {
            ConnectionSizeType::Variable
        },
    })
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

    fn bridge(
        trigger_and_transport: u32,
        connection_parameters: u32,
    ) -> Result<ConnectionConfig, BridgeError> {
        to_connection_config(
            &connection(trigger_and_transport, connection_parameters),
            originator(),
        )
    }

    fn unsupported(what_contains: &str, result: Result<ConnectionConfig, BridgeError>) {
        match result {
            Err(BridgeError::Unsupported { connection, what }) => {
                assert_eq!(connection, "Connection1");
                assert!(what.contains(what_contains), "{what}");
            }
            other => panic!("expected Unsupported({what_contains}), got {other:?}"),
        }
    }

    #[test]
    fn the_opener_connection_maps_field_by_field() {
        let config = bridge(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS).unwrap();

        assert_eq!(
            config,
            ConnectionConfig {
                configuration_instance: 0x97,
                o2t: DirectionConfig {
                    connection_point: 0x96,
                    data_size: 32,
                    requested_packet_interval: 1_000_000,
                    real_time_format: RealTimeFormat::Header32Bit,
                    connection_size_type: ConnectionSizeType::Fixed,
                },
                t2o: DirectionConfig {
                    connection_point: 0x64,
                    data_size: 32,
                    requested_packet_interval: 1_000_000,
                    real_time_format: RealTimeFormat::Modeless,
                    connection_size_type: ConnectionSizeType::Fixed,
                },
                transport_class: TransportClass::Class1,
                production_trigger: ProductionTrigger::Cyclic,
                priority: ConnectionPriority::Scheduled,
                connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
                t2o_network_connection_id: 0x1234_5678,
                connection_triad: originator().connection_triad,
                large_forward_open: false,
            }
        );
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
        let priority = |word: u32| bridge(TRIGGER_AND_TRANSPORT, word).unwrap().priority;

        assert_eq!(priority(0x7775_0405), ConnectionPriority::Scheduled);
        assert_eq!(priority(0x3775_0405), ConnectionPriority::High); // T->O: low + high
        assert_eq!(priority(0x1375_0405), ConnectionPriority::Low); // T->O low, O->T low + high
        unsupported("priority", bridge(TRIGGER_AND_TRANSPORT, 0x1275_0405)); // T->O low, O->T high
    }

    #[test]
    fn size_type_is_fixed_when_supported_else_variable() {
        let config = bridge(TRIGGER_AND_TRANSPORT, 0x7775_040A).unwrap(); // variable only
        assert_eq!(
            config.o2t.connection_size_type,
            ConnectionSizeType::Variable
        );
        assert_eq!(
            config.t2o.connection_size_type,
            ConnectionSizeType::Variable
        );
    }

    #[test]
    fn real_time_formats_map_or_are_refused() {
        let formats = |word: u32| {
            let config = bridge(TRIGGER_AND_TRANSPORT, word).unwrap();
            (config.o2t.real_time_format, config.t2o.real_time_format)
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
            to_connection_config(&connection, originator()),
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
            to_connection_config(&connection, originator())
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
}
