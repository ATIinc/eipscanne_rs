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

    use bilge::prelude::{u4, u9};

    use eipscanne_rs::cip::connection_manager::forward_open::{
        ForwardOpenRequest, ForwardOpenResponse,
    };
    use eipscanne_rs::cip::connection_manager::parameters::{
        ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType,
        Direction, NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger,
        RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters, TransportClass,
        TransportTypeTrigger,
    };
    use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
    use eipscanne_rs::cip::path::CipPath;
    use eipscanne_rs::cip::types::CipUdint;
    use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;

    use crate::implicit::open::OpenConnection;

    pub const TARGET_IP: Ipv4Addr = Ipv4Addr::new(172, 28, 0, 10);
    /// The connection IDs of the library's I/O packet tests
    pub const O2T_NETWORK_CONNECTION_ID: CipUdint = 0xa1b2_c3d4;
    pub const T2O_NETWORK_CONNECTION_ID: CipUdint = 0x1234_5678;

    /// The 16-bit parameter word of a point-to-point, exclusive-owner direction
    pub fn standard_parameters(
        connection_size: u16,
        connection_size_type: ConnectionSizeType,
    ) -> NetworkConnectionParameters {
        NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(connection_size))
                .connection_size_type(connection_size_type)
                .priority(ConnectionPriority::Scheduled)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        )
    }

    /// The Forward_Open of the library's Forward_Open test: configuration assembly 151, 32
    /// output bytes to assembly 150 behind a sequence count and a run/idle header (38), 32 input
    /// bytes from assembly 100 behind a sequence count (34), both every second, x4 timeout
    pub fn sample_request() -> ForwardOpenRequest {
        ForwardOpenRequest {
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            o2t_network_connection_id: 0,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: 0x0001,
                originator_vendor_id: 342,
                originator_serial_number: 0x0001_2345,
            },
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            o2t_requested_packet_interval: 1_000_000,
            o2t_network_connection_parameters: standard_parameters(38, ConnectionSizeType::Fixed),
            t2o_requested_packet_interval: 1_000_000,
            t2o_network_connection_parameters: standard_parameters(34, ConnectionSizeType::Fixed),
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class1)
                .production_trigger(ProductionTrigger::Cyclic)
                .direction(Direction::Client)
                .build(),
            connection_path: CipPath::new_assembly_connection(151, 150, 100),
        }
    }

    /// The connection `sample_request` opens, with the run/idle header on the outputs only
    pub fn sample_connection() -> OpenConnection {
        let request = sample_request();
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
            request,
            response,
            o2t_real_time_format: RealTimeFormat::Header32Bit,
            t2o_real_time_format: RealTimeFormat::Modeless,
            target_ip: TARGET_IP,
            o2t_endpoint: SocketAddrV4::new(TARGET_IP, ETHERNET_IP_IO_UDP_PORT),
        }
    }
}
