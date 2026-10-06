//! Reference scaffolding around `eipscanne_rs`: how to open a class 1 (implicit messaging)
//! connection to an adapter, exchange cyclic I/O with it and close it again. The library stays
//! packet (de)serialization only; everything with a socket, a timer or state lives here, one
//! module per stage of the protocol, so production code can read it top to bottom and reuse the
//! parts it needs.
//!
//! ```text
//! Stage              What happens on the wire                                 Module
//! -----------------  -------------------------------------------------------  ---------
//! 1. Session         TCP 44818: RegisterSession                      -> Session   session
//! 2. Open            SendRRData(Forward_Open), read the reply -> OpenConnection   open
//! 3. Exchange        UDP 2222, two independent directions:                       udp
//!      3a. Produce     O->T: one packet every O->T interval (the outputs)         produce
//!      3b. Consume     T->O: screen, decode, watch the timeout (the inputs)       consume
//! 4. Close           SendRRData(Forward_Close), read the reply                   close
//! 5. End session     UnregisterSession                                           session
//! ```
//!
//! What the caller decides before stage 2 is a [`config::ConnectionConfig`]. The sending and the
//! receiving direction of stage 3 share no state, so they are two types: a [`produce::Producer`]
//! and a [`consume::Consumer`]. Neither touches the network; only `session`, `open`, `close`,
//! `udp` and the caller's loop do (see the `implicit-io` example).

pub mod close;
pub mod config;
pub mod consume;
pub mod open;
pub mod produce;
pub mod session;
pub mod udp;

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

    use crate::config::{ConnectionConfig, DirectionConfig};
    use crate::open::OpenConnection;

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
