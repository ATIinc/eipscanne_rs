//! Everything the caller decides about a connection before opening it, and how that becomes a
//! Forward_Open request.

use std::fmt;

use bilge::prelude::{u4, u9};

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipUdint, CipUsint};

/// Tick time of the unconnected request timeout: 1 ms shifted left by 10, i.e. 1024 ms per tick
const PRIORITY_TICK_TIME: u8 = 10;
/// Ticks until the Forward_Open or Forward_Close itself times out: 5 x 1024 ms
const TIMEOUT_TICKS: CipUsint = 5;
/// The target chooses the O->T connection ID of a point-to-point connection, so the request asks
/// for none
const O2T_CONNECTION_ID_CHOSEN_BY_TARGET: CipUdint = 0;

/// The settings of one direction of the connection
#[derive(Debug, Clone, PartialEq)]
pub struct DirectionConfig {
    /// The assembly instance that produces (T->O) or consumes (O->T) the data
    pub connection_point: u8,
    /// Bytes of application data per packet, without the sequence count and real-time header
    pub data_size: u16,
    /// Requested packet interval (RPI), in microseconds
    pub requested_packet_interval: CipUdint,
    /// How the packets of this direction signal run/idle. Not part of the Forward_Open: both
    /// ends must agree on it beforehand (an EDS file or the device manual says which one)
    pub real_time_format: RealTimeFormat,
    /// Whether every packet carries exactly `data_size` bytes or at most that many
    pub connection_size_type: ConnectionSizeType,
}

/// Everything the caller decides before opening a connection. Phase 4 derives it from an EDS file;
/// until then it is typed by hand (see the `implicit-io` example).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionConfig {
    /// The assembly instance that holds the configuration data
    pub configuration_instance: u8,
    /// O->T: the outputs this scanner sends
    pub o2t: DirectionConfig,
    /// T->O: the inputs the adapter sends
    pub t2o: DirectionConfig,
    pub transport_class: TransportClass,
    pub production_trigger: ProductionTrigger,
    /// Priority of both directions
    pub priority: ConnectionPriority,
    pub connection_timeout_multiplier: ConnectionTimeoutMultiplier,
    /// The T->O connection ID: chosen by this scanner, echoed by the adapter on every input
    /// packet
    pub t2o_network_connection_id: CipUdint,
    pub connection_triad: ConnectionTriad,
    /// Send a Large_Forward_Open (32-bit connection parameters) instead of a Forward_Open
    pub large_forward_open: bool,
}

/// A configuration that cannot be sent
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// The connection size of a direction does not fit the 9 bits of a Forward_Open; a
    /// Large_Forward_Open carries up to 65535 bytes
    ConnectionSizeTooLarge { direction: &'static str, size: u16 },
}

// ======= Start of ConnectionConfig impl ========

impl ConnectionConfig {
    /// The Forward_Open (or Large_Forward_Open) request that opens this connection: class 1
    /// point-to-point in both directions, exclusive owner, this scanner as the client
    pub fn to_forward_open_request(&self) -> Result<ForwardOpenRequest, ConfigError> {
        Ok(ForwardOpenRequest {
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(PRIORITY_TICK_TIME))
                .priority(false)
                .build(),
            timeout_ticks: TIMEOUT_TICKS,
            o2t_network_connection_id: O2T_CONNECTION_ID_CHOSEN_BY_TARGET,
            t2o_network_connection_id: self.t2o_network_connection_id,
            connection_triad: self.connection_triad,
            connection_timeout_multiplier: self.connection_timeout_multiplier,
            o2t_requested_packet_interval: self.o2t.requested_packet_interval,
            o2t_network_connection_parameters: self
                .network_connection_parameters("O->T", &self.o2t)?,
            t2o_requested_packet_interval: self.t2o.requested_packet_interval,
            t2o_network_connection_parameters: self
                .network_connection_parameters("T->O", &self.t2o)?,
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(self.transport_class)
                .production_trigger(self.production_trigger)
                .direction(Direction::Client)
                .build(),
            connection_path: CipPath::new_assembly_connection(
                self.configuration_instance,
                self.o2t.connection_point,
                self.t2o.connection_point,
            ),
        })
    }

    /// The parameter word of one direction, in the width of the service
    fn network_connection_parameters(
        &self,
        direction_name: &'static str,
        direction: &DirectionConfig,
    ) -> Result<NetworkConnectionParameters, ConfigError> {
        let size = connection_size(
            direction.data_size,
            self.transport_class,
            direction.real_time_format,
        );

        if self.large_forward_open {
            Ok(NetworkConnectionParameters::Large(
                LargeNetworkConnectionParameters::builder()
                    .connection_size(size)
                    .connection_size_type(direction.connection_size_type)
                    .priority(self.priority)
                    .connection_type(ConnectionType::PointToPoint)
                    .redundant_owner(RedundantOwner::Exclusive)
                    .build(),
            ))
        } else {
            let size = u9::try_new(size).map_err(|_| ConfigError::ConnectionSizeTooLarge {
                direction: direction_name,
                size,
            })?;
            Ok(NetworkConnectionParameters::Standard(
                StandardNetworkConnectionParameters::builder()
                    .connection_size(size)
                    .connection_size_type(direction.connection_size_type)
                    .priority(self.priority)
                    .connection_type(ConnectionType::PointToPoint)
                    .redundant_owner(RedundantOwner::Exclusive)
                    .build(),
            ))
        }
    }
}

// ^^^^^^^^ End of ConnectionConfig impl ^^^^^^^^

// ======= Start of ConfigError impl ========

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ConnectionSizeTooLarge { direction, size } => write!(
                f,
                "the {direction} connection size of {size} bytes needs a Large_Forward_Open"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

// ^^^^^^^^ End of ConfigError impl ^^^^^^^^

#[cfg(test)]
mod tests {
    use binrw::BinWrite;
    use hex_test_macros::prelude::*;

    use eipscanne_rs::cip::types::CipByte;
    use eipscanne_rs::object_assembly::RequestObjectAssembly;

    use super::*;

    /// Session handle of the capture the expected bytes come from
    const SESSION_HANDLE: CipUdint = 0x03;

    /// The connection the OpENer sample application accepts: configuration assembly 151,
    /// 32 output bytes to assembly 150 with a run/idle header, 32 input bytes from assembly 100
    /// without one, both every second
    fn opener_sample_config() -> ConnectionConfig {
        ConnectionConfig {
            configuration_instance: 151,
            o2t: DirectionConfig {
                connection_point: 150,
                data_size: 32,
                requested_packet_interval: 1_000_000,
                real_time_format: RealTimeFormat::Header32Bit,
                connection_size_type: ConnectionSizeType::Fixed,
            },
            t2o: DirectionConfig {
                connection_point: 100,
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
            connection_triad: ConnectionTriad {
                connection_serial_number: 0x0001,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            large_forward_open: false,
        }
    }

    #[test]
    fn opener_sample_config_builds_the_forward_open_of_the_captures() {
        // The same bytes as the Forward_Open request test of the library, where Wireshark's
        // dissection of them is documented
        let expected_byte_array: Vec<CipByte> = vec![
            0x6f, 0x00, 0x46, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x36, 0x00, 0x54, 0x04,
            0x21, 0x00, 0x06, 0x00, 0x25, 0x00, 0x01, 0x00, 0x0a, 0x05, 0x00, 0x00, 0x00, 0x00,
            0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x56, 0x01, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x40, 0x42, 0x0f, 0x00, 0x26, 0x48, 0x40, 0x42, 0x0f, 0x00, 0x22, 0x48,
            0x01, 0x04, 0x20, 0x04, 0x24, 0x97, 0x2c, 0x96, 0x2c, 0x64,
        ];

        let request = opener_sample_config().to_forward_open_request().unwrap();
        let packet = RequestObjectAssembly::new_forward_open(SESSION_HANDLE, request);

        let mut bytes = std::io::Cursor::new(Vec::new());
        packet.write(&mut bytes).unwrap();
        let bytes = bytes.into_inner();

        assert_eq_hex!(expected_byte_array, bytes);
    }

    #[test]
    fn large_forward_open_widens_the_connection_parameters() {
        let mut config = opener_sample_config();
        config.large_forward_open = true;

        let request = config.to_forward_open_request().unwrap();

        assert!(matches!(
            request.o2t_network_connection_parameters,
            NetworkConnectionParameters::Large(parameters) if parameters.connection_size() == 38
        ));
        assert!(matches!(
            request.t2o_network_connection_parameters,
            NetworkConnectionParameters::Large(parameters) if parameters.connection_size() == 34
        ));
    }

    #[test]
    fn connection_size_over_nine_bits_needs_a_large_forward_open() {
        let mut config = opener_sample_config();
        config.t2o.data_size = 510; // 512 with the sequence count

        assert_eq!(
            config.to_forward_open_request(),
            Err(ConfigError::ConnectionSizeTooLarge {
                direction: "T->O",
                size: 512
            })
        );

        config.large_forward_open = true;
        assert!(config.to_forward_open_request().is_ok());
    }
}
