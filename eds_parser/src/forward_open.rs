//! Step 4: from what the EDS says the device supports to the fields of the Forward_Open this
//! scanner sends.
//!
//! The masks of a connection list what the device *supports*; each method here picks the value of
//! one Forward_Open field, named after it, and refuses the connection when the device does not
//! support what this scanner does (class 1, cyclic, exclusive owner, point-to-point). The caller
//! writes the `ForwardOpenRequest` from them, with what an EDS does not describe: tick time,
//! timeouts, connection IDs and the connection triad.

use bilge::prelude::{u3, u9};

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, ProductionTrigger,
    RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters, TransportClass,
    TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;

use crate::connection::{Connection, DirectionSpec, read_path};
use crate::error::{Error, Result};

/// Bit of the transport classes word for class 1
const CLASS_1: u16 = 1 << 1;

// ======= Start of Connection impl ========

impl Connection {
    /// Class 1, cyclic, client. The client/server bit of the EDS is ignored: vendors disagree on
    /// it, and the originator of a Forward_Open is always the client.
    pub fn transport_type_trigger(&self) -> Result<TransportTypeTrigger> {
        let masks = &self.trigger_and_transport;
        if masks.transport_classes() & CLASS_1 == 0 {
            return Err(self.unsupported("transport class 1 is not supported"));
        }
        if !masks.cyclic() {
            return Err(self.unsupported("a cyclic trigger is not supported"));
        }
        if !masks.exclusive_owner() {
            return Err(self.unsupported(
                "not an exclusive-owner connection (input-only and listen-only are out of scope)",
            ));
        }
        Ok(TransportTypeTrigger::builder()
            .transport_class(TransportClass::Class1)
            .production_trigger(ProductionTrigger::Cyclic)
            .direction(Direction::Client)
            .build())
    }

    /// The connection path as the EDS gives it; only logical segments are supported
    pub fn connection_path(&self) -> Result<CipPath> {
        read_path(&self.path).ok_or_else(|| {
            let hex: Vec<String> = self.path.iter().map(|byte| format!("{byte:02X}")).collect();
            self.unsupported(format!(
                "path {} is not made of logical segments only",
                hex.join(" ")
            ))
        })
    }

    pub fn o2t_requested_packet_interval(&self) -> Result<CipUdint> {
        self.requested_packet_interval(&self.o2t, "O->T")
    }

    pub fn t2o_requested_packet_interval(&self) -> Result<CipUdint> {
        self.requested_packet_interval(&self.t2o, "T->O")
    }

    /// The O->T parameter word, 32-bit when `large` (a Large_Forward_Open)
    pub fn o2t_network_connection_parameters(
        &self,
        large: bool,
    ) -> Result<NetworkConnectionParameters> {
        let parameters = &self.connection_parameters;
        if !parameters.o2t_point_to_point() {
            return Err(self.unsupported("O->T point-to-point is not supported"));
        }
        self.network_connection_parameters(
            &self.o2t,
            "O->T",
            self.o2t_real_time_format()?,
            parameters.o2t_fixed_size(),
            large,
        )
    }

    /// The T->O parameter word, 32-bit when `large` (a Large_Forward_Open)
    pub fn t2o_network_connection_parameters(
        &self,
        large: bool,
    ) -> Result<NetworkConnectionParameters> {
        let parameters = &self.connection_parameters;
        if !parameters.t2o_point_to_point() {
            return Err(self.unsupported("T->O point-to-point is not supported"));
        }
        self.network_connection_parameters(
            &self.t2o,
            "T->O",
            self.t2o_real_time_format()?,
            parameters.t2o_fixed_size(),
            large,
        )
    }

    /// How the O->T direction signals run/idle; the Forward_Open does not carry it
    pub fn o2t_real_time_format(&self) -> Result<RealTimeFormat> {
        self.real_time_format(self.connection_parameters.o2t_real_time_format(), "O->T")
    }

    /// How the T->O direction signals run/idle; the Forward_Open does not carry it
    pub fn t2o_real_time_format(&self) -> Result<RealTimeFormat> {
        self.real_time_format(self.connection_parameters.t2o_real_time_format(), "T->O")
    }

    fn requested_packet_interval(&self, spec: &DirectionSpec, direction: &str) -> Result<CipUdint> {
        spec.requested_packet_interval.ok_or_else(|| {
            self.unsupported(format!(
                "the EDS gives no {direction} requested packet interval"
            ))
        })
    }

    /// The EDS size plus the sequence count and real-time header (EDS sizes exclude both), fixed
    /// size when supported (variable otherwise), the best priority both directions support
    fn network_connection_parameters(
        &self,
        spec: &DirectionSpec,
        direction: &str,
        real_time_format: RealTimeFormat,
        fixed_size_supported: bool,
        large: bool,
    ) -> Result<NetworkConnectionParameters> {
        let size = connection_size(spec.size, TransportClass::Class1, real_time_format);
        let size_type = if fixed_size_supported {
            ConnectionSizeType::Fixed
        } else {
            ConnectionSizeType::Variable
        };
        let priority = self.priority()?;

        if large {
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
        let size = u9::try_new(size).map_err(|_| {
            self.unsupported(format!(
                "the {direction} connection size of {size} bytes needs a Large_Forward_Open"
            ))
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
    fn priority(&self) -> Result<ConnectionPriority> {
        let parameters = &self.connection_parameters;
        if parameters.o2t_scheduled_priority() && parameters.t2o_scheduled_priority() {
            Ok(ConnectionPriority::Scheduled)
        } else if parameters.o2t_high_priority() && parameters.t2o_high_priority() {
            Ok(ConnectionPriority::High)
        } else if parameters.o2t_low_priority() && parameters.t2o_low_priority() {
            Ok(ConnectionPriority::Low)
        } else {
            Err(self.unsupported(
                "no priority (scheduled, high or low) is supported by both directions",
            ))
        }
    }

    fn real_time_format(&self, code: u3, direction: &str) -> Result<RealTimeFormat> {
        match u8::from(code) {
            0 => Ok(RealTimeFormat::Modeless),
            1 => Ok(RealTimeFormat::ZeroLength),
            3 => Ok(RealTimeFormat::Heartbeat),
            4 => Ok(RealTimeFormat::Header32Bit),
            other => Err(self.unsupported(format!(
                "{direction} real-time format {other} is not supported"
            ))),
        }
    }

    fn unsupported(&self, message: impl Into<String>) -> Error {
        Error {
            entry: self.keyword.clone(),
            message: message.into(),
        }
    }
}

// ^^^^^^^^ End of Connection impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use crate::connection::{ConnectionParameters, TriggerAndTransport};

    use super::*;

    /// Class 1, cyclic, exclusive owner
    const TRIGGER_AND_TRANSPORT: u32 = 0x0401_0002;
    /// Fixed sizes both ways, O->T 32-bit header, T->O modeless, point-to-point both ways, every
    /// priority both ways
    const CONNECTION_PARAMETERS: u32 = 0x7775_0405;

    fn connection(trigger_and_transport: u32, connection_parameters: u32) -> Connection {
        Connection {
            keyword: "Connection1".to_string(),
            name: "Exclusive owner".to_string(),
            trigger_and_transport: TriggerAndTransport::from(trigger_and_transport),
            connection_parameters: ConnectionParameters::from(connection_parameters),
            o2t: DirectionSpec {
                requested_packet_interval: Some(1_000_000),
                size: 32,
                assembly: None,
            },
            t2o: DirectionSpec {
                requested_packet_interval: Some(1_000_000),
                size: 32,
                assembly: None,
            },
            path: vec![0x20, 0x04, 0x24, 0x97, 0x2C, 0x96, 0x2C, 0x64],
        }
    }

    fn unsupported<T: std::fmt::Debug>(message_contains: &str, result: Result<T>) {
        match result {
            Err(error) => {
                assert_eq!(error.entry, "Connection1");
                assert!(error.message.contains(message_contains), "{error}");
            }
            other => panic!("expected an error about {message_contains}, got {other:?}"),
        }
    }

    /// A 16-bit parameter word, as `o2t_network_connection_parameters` builds it
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
    fn transport_class_trigger_and_application_type_must_match() {
        let trigger = |word: u32| connection(word, CONNECTION_PARAMETERS).transport_type_trigger();

        unsupported("class 1", trigger(0x0401_0001)); // class 0 only
        unsupported("cyclic", trigger(0x0402_0002)); // change of state only
        unsupported("exclusive-owner", trigger(0x0201_0002)); // input only
    }

    #[test]
    fn the_client_server_bit_is_ignored() {
        assert!(
            connection(TRIGGER_AND_TRANSPORT | 0x8000_0000, CONNECTION_PARAMETERS)
                .transport_type_trigger()
                .is_ok()
        );
    }

    #[test]
    fn both_directions_must_support_point_to_point() {
        unsupported(
            "O->T point-to-point",
            connection(TRIGGER_AND_TRANSPORT, 0x7771_0405).o2t_network_connection_parameters(false),
        ); // bit 18 off
        unsupported(
            "T->O point-to-point",
            connection(TRIGGER_AND_TRANSPORT, 0x7735_0405).t2o_network_connection_parameters(false),
        ); // bit 22 off
    }

    #[test]
    fn priority_is_the_best_one_both_directions_support() {
        let parameters = |word: u32| {
            connection(TRIGGER_AND_TRANSPORT, word).o2t_network_connection_parameters(false)
        };
        let priority = |word: u32| {
            let Ok(NetworkConnectionParameters::Standard(o2t)) = parameters(word) else {
                panic!("a Forward_Open has 16-bit parameters");
            };
            o2t.priority()
        };

        assert_eq!(priority(0x7775_0405), ConnectionPriority::Scheduled);
        assert_eq!(priority(0x3775_0405), ConnectionPriority::High); // T->O: low + high
        assert_eq!(priority(0x1375_0405), ConnectionPriority::Low); // T->O low, O->T low + high
        unsupported("priority", parameters(0x1275_0405)); // T->O low, O->T high
    }

    #[test]
    fn size_type_is_fixed_when_supported_else_variable() {
        let connection = connection(TRIGGER_AND_TRANSPORT, 0x7775_040A); // variable only
        assert_eq!(
            connection.o2t_network_connection_parameters(false),
            Ok(standard(
                38,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            ))
        );
        assert_eq!(
            connection.t2o_network_connection_parameters(false),
            Ok(standard(
                34,
                ConnectionSizeType::Variable,
                ConnectionPriority::Scheduled
            ))
        );
    }

    #[test]
    fn real_time_formats_map_or_are_refused() {
        let formats = |word: u32| {
            let connection = connection(TRIGGER_AND_TRANSPORT, word);
            (
                connection.o2t_real_time_format().unwrap(),
                connection.t2o_real_time_format().unwrap(),
            )
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
            connection(TRIGGER_AND_TRANSPORT, 0x7775_0205).o2t_real_time_format(),
        );
        unsupported(
            "T->O real-time format 5",
            connection(TRIGGER_AND_TRANSPORT, 0x7775_5405).t2o_real_time_format(),
        );
    }

    #[test]
    fn a_direction_without_rpi_is_refused() {
        let mut connection = connection(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS);
        connection.t2o.requested_packet_interval = None;

        unsupported(
            "T->O requested packet interval",
            connection.t2o_requested_packet_interval(),
        );
    }

    #[test]
    fn a_connection_size_over_nine_bits_needs_a_large_forward_open() {
        let mut connection = connection(TRIGGER_AND_TRANSPORT, CONNECTION_PARAMETERS);
        connection.t2o.size = 510; // 512 with the sequence count

        unsupported(
            "512 bytes needs a Large_Forward_Open",
            connection.t2o_network_connection_parameters(false),
        );
        assert!(matches!(
            connection.t2o_network_connection_parameters(true),
            Ok(NetworkConnectionParameters::Large(parameters)) if parameters.connection_size() == 512
        ));
    }
}
