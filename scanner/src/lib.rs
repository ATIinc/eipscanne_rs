//! The scanner side of EtherNet/IP, built on `eipscanne_rs`: how to talk to an adapter, in code
//! a person can read top to bottom. The library stays packet (de)serialization only; everything
//! with a socket, a timer or state lives here, so production code can use it as a reference and
//! reuse the parts it needs.
//!
//! ```text
//! session     The encapsulation session over TCP 44818 (RegisterSession ... UnregisterSession),
//!             shared by both kinds of messaging
//! explicit    Explicit (unconnected) messaging: one request, one reply, e.g. reading the
//!             Identity object or an assembly
//! implicit    Implicit messaging: a class 1 I/O connection (Forward_Open, cyclic I/O over
//!             UDP 2222, Forward_Close), one submodule per stage
//! ```
//!
//! The `read-identity` and `write-clearlink-io` examples use `session` and `explicit`; the
//! `implicit-io` example uses `session` and `implicit`. Every fallible call returns the one
//! [`Error`].

mod error;
pub mod explicit;
pub mod implicit;
pub mod session;

pub use error::Error;

/// One open connection to build the producer and consumer tests on
#[cfg(test)]
pub(crate) mod test_support {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenResponse;
    use eipscanne_rs::cip::connection_manager::parameters::{
        ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ProductionTrigger,
        RealTimeFormat, TransportClass,
    };
    use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
    use eipscanne_rs::cip::types::CipUdint;
    use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;

    use crate::implicit::config::{ConnectionConfig, DirectionConfig};
    use crate::implicit::open::OpenConnection;

    pub const TARGET_IP: Ipv4Addr = Ipv4Addr::new(172, 28, 0, 10);
    /// The connection IDs of the library's I/O packet tests
    pub const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
    pub const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;

    /// The connection of the library's Forward_Open and I/O packet tests: 32 bytes each way every
    /// second, a run/idle header on the outputs only, a x4 timeout multiplier
    pub fn sample_connection() -> OpenConnection {
        let config = ConnectionConfig {
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
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: 0x0001,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            large_forward_open: false,
        };
        let request = config.to_forward_open_request().unwrap();
        let response = ForwardOpenResponse {
            o2t_network_connection_id: O2T_NETWORK_CONNECTION_ID,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: request.connection_triad,
            o2t_actual_packet_interval: 1_000_000,
            t2o_actual_packet_interval: 1_000_000,
            application_reply_size: 0,
            application_reply: vec![],
        };

        OpenConnection {
            config,
            request,
            response,
            target_ip: TARGET_IP,
            o2t_endpoint: SocketAddrV4::new(TARGET_IP, ETHERNET_IP_IO_UDP_PORT),
        }
    }
}
