//! Opens a class 3 connection to an adapter's Message Router, reads its Identity object over the
//! connection a number of times and closes it again.
//!
//! Start OpENer (`tests/integration`) on the host, then run the example in the devcontainer:
//!
//! ```text
//! tests/integration/start-opener.sh
//! cargo run --example read-identity-connected
//! ```
//!
//! `-- --host <adapter IP>` reads another adapter.

use std::time::Duration;

use bilge::prelude::{u4, u9};
use clap::Parser;
use tokio::signal::ctrl_c;
use tokio::time::interval;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RedundantOwner,
    StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{
    IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID, MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID,
};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use eipscanne_rs::eip::packet::EnIpPacket;
use scanner::explicit::connected::{ExplicitConnection, forward_open_explicit, send_request};
use scanner::explicit::decode_reply;
use scanner::implicit::connection::forward_close;
use scanner::session::{Session, SessionConfig};

/// Who this scanner is in the Forward_Open; the Forward_Close repeats it
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 2;
/// The connection ID the adapter puts on every reply
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5679;
/// Bytes per message, each way, including the sequence count (a 16-bit connection parameter word
/// allows at most 511)
const CONNECTION_SIZE: u16 = 504;

/// Reads the Identity object of an EtherNet/IP adapter over a class 3 connection
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,

    /// Number of reads before closing
    #[arg(long, default_value_t = 5)]
    reads: u32,

    /// Time between reads in milliseconds; also the connection's requested packet interval, so
    /// the adapter drops the connection after four missed reads
    #[arg(long, default_value_t = 1000)]
    interval: u32,
}

impl Args {
    /// A class 3 Forward_Open to the Message Router that times out after four missed requests
    fn forward_open_request(&self) -> ForwardOpenRequest {
        let requested_packet_interval = self.interval * 1000;
        let parameters = NetworkConnectionParameters::Standard(
            StandardNetworkConnectionParameters::builder()
                .connection_size(u9::new(CONNECTION_SIZE))
                .connection_size_type(ConnectionSizeType::Variable)
                .priority(ConnectionPriority::Low)
                .connection_type(ConnectionType::PointToPoint)
                .redundant_owner(RedundantOwner::Exclusive)
                .build(),
        );

        ForwardOpenRequest {
            // 5 ticks of 1024 ms until the request itself times out
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            // Point-to-point: the adapter chooses the O->T connection ID and returns it
            o2t_network_connection_id: 0,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
            },
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            o2t_requested_packet_interval: requested_packet_interval,
            o2t_network_connection_parameters: parameters,
            t2o_requested_packet_interval: requested_packet_interval,
            t2o_network_connection_parameters: parameters,
            // The adapter's Message Router answers each request it receives
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class3)
                .production_trigger(ProductionTrigger::ApplicationObject)
                .direction(Direction::Server)
                .build(),
            connection_path: CipPath::new_u8(MESSAGE_ROUTER_CLASS_ID, MESSAGE_ROUTER_INSTANCE_ID),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register(
        (args.host.as_str(), ETHERNET_IP_TCP_PORT),
        SessionConfig::default(),
    )
    .await?;

    // ========= Open the connection ============
    println!("OPENING the connection");
    let connection: ExplicitConnection =
        forward_open_explicit(&mut session, args.forward_open_request()).await?;
    println!(
        "  O->T connection {:#010x}, T->O connection {:#010x}, every {} us",
        connection.response.o2t_network_connection_id,
        connection.response.t2o_network_connection_id,
        connection.response.o2t_actual_packet_interval
    );

    // ========= Read the identity over the connection ============
    // A new request advances the CIP sequence count; the reply repeats it
    let mut read_timer = interval(Duration::from_millis(u64::from(args.interval)));
    let mut cip_sequence_count: u16 = 0;
    // One Ctrl+C future for the whole loop, so no press is lost; the connection is still closed
    let ctrl_c = ctrl_c();
    tokio::pin!(ctrl_c);
    for read in 1..=args.reads {
        tokio::select! {
            _ = read_timer.tick() => {}
            _ = &mut ctrl_c => {
                println!("Ctrl+C: stopping");
                break;
            }
        }

        cip_sequence_count = cip_sequence_count.wrapping_add(1);
        let reply: EnIpPacket = send_request(
            &mut session,
            &connection,
            cip_sequence_count,
            CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
            ServiceCode::GetAttributeAll,
            None,
        )
        .await?;
        let identity_response: IdentityResponse = decode_reply(&reply)?;
        println!(
            "[{read:>3}] sequence count {cip_sequence_count}: {:?}, serial {:#010x}",
            String::from(identity_response.product_name),
            identity_response.serial_number
        );
    }

    // ========= Close the connection ============
    println!("CLOSING the connection");
    forward_close(&mut session, &connection.request).await?;

    // ========= End the session ============
    println!("UNREGISTERING the session");
    session.unregister().await?;

    Ok(())
}
