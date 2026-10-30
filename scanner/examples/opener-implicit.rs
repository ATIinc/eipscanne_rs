//! Opens a class 1 connection to the OpENer sample, exchanges cyclic I/O for a number of cycles
//! and closes it; `main` is the five stages in order. OpENer echoes the outputs of assembly 150 as
//! the inputs of assembly 100, idle ones too.
//!
//! Start OpENer (`tests/integration`) on the host, then run the example in the devcontainer:
//!
//! ```text
//! tests/integration/start-opener.sh
//! cargo run --example opener-implicit
//! ```

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bilge::prelude::{u4, u9};
use clap::Parser;
use tokio::net::UdpSocket;
use tokio::signal::ctrl_c;
use tokio::time::{interval, sleep_until};

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connected_data_header_len,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::io_data::RunOrIdle;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use eipscanne_rs::eip::io_packet::EnIpIoPacket;
use scanner::error::Error;
use scanner::implicit::connection::{ImplicitConnection, forward_close, forward_open_implicit};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{
    FIRST_PACKET_GRACE, accept_t2o_packet, bind_io_socket, input_timeout, recv_io_packet,
};
use scanner::session::{Session, SessionConfig};

/// Who this scanner is in the Forward_Open; the Forward_Close repeats it
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
/// The connection ID the adapter puts on every input packet
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;
/// How each direction signals run/idle, agreed off the wire (EDS file or device manual). The
/// OpENer sample takes a run/idle header on the outputs only.
const O2T_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Header32Bit;
const T2O_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Modeless;
/// The OpENer sample's assembly instances
const CONFIGURATION_INSTANCE: u8 = 151;
const OUTPUT_INSTANCE: u8 = 150;
const INPUT_INSTANCE: u8 = 100;
/// Data bytes per packet, each way
const DATA_SIZE: u16 = 32;

/// Exchanges cyclic I/O with the OpENer sample over a class 1 connection
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the OpENer adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,

    /// Requested packet interval of both directions, in milliseconds
    #[arg(long, default_value_t = 1000)]
    rpi: u32,

    /// Number of output packets to send before closing
    #[arg(long, default_value_t = 10)]
    cycles: u32,

    /// Open with a Large_Forward_Open (32-bit connection parameters)
    #[arg(long)]
    large: bool,

    /// Switch the outputs between run and idle every this many cycles, starting in run
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    toggle_every: Option<u32>,
}

impl Args {
    /// A class 1 cyclic, point-to-point Forward_Open that times out after four missed packets
    fn forward_open_request(&self) -> Result<ForwardOpenRequest, Box<dyn std::error::Error>> {
        let requested_packet_interval = self.rpi * 1000;
        // Each connection size counts the 16-bit sequence count and the real-time header too
        let o2t_size: u16 =
            DATA_SIZE + connected_data_header_len(TransportClass::Class1, O2T_REAL_TIME_FORMAT);
        let t2o_size: u16 =
            DATA_SIZE + connected_data_header_len(TransportClass::Class1, T2O_REAL_TIME_FORMAT);

        Ok(ForwardOpenRequest {
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
            o2t_network_connection_parameters: self.network_connection_parameters(o2t_size)?,
            t2o_requested_packet_interval: requested_packet_interval,
            t2o_network_connection_parameters: self.network_connection_parameters(t2o_size)?,
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class1)
                .production_trigger(ProductionTrigger::Cyclic)
                .direction(Direction::Client)
                .build(),
            connection_path: CipPath::new_assembly_connection(
                CONFIGURATION_INSTANCE,
                OUTPUT_INSTANCE,
                INPUT_INSTANCE,
            ),
        })
    }

    /// One direction's parameter word: 32 bits for a Large_Forward_Open, otherwise 16
    fn network_connection_parameters(
        &self,
        size: u16,
    ) -> Result<NetworkConnectionParameters, Box<dyn std::error::Error>> {
        Ok(if self.large {
            NetworkConnectionParameters::Large(
                LargeNetworkConnectionParameters::builder()
                    .connection_size(size)
                    .connection_size_type(ConnectionSizeType::Fixed)
                    .priority(ConnectionPriority::Scheduled)
                    .connection_type(ConnectionType::PointToPoint)
                    .redundant_owner(RedundantOwner::Exclusive)
                    .build(),
            )
        } else {
            let size =
                u9::try_new(size).map_err(|_| format!("a {size}-byte connection needs --large"))?;
            NetworkConnectionParameters::Standard(
                StandardNetworkConnectionParameters::builder()
                    .connection_size(size)
                    .connection_size_type(ConnectionSizeType::Fixed)
                    .priority(ConnectionPriority::Scheduled)
                    .connection_type(ConnectionType::PointToPoint)
                    .redundant_owner(RedundantOwner::Exclusive)
                    .build(),
            )
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register(
        (args.host.as_str(), ETHERNET_IP_TCP_PORT),
        SessionConfig::default(),
    )
    .await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket: UdpSocket = bind_io_socket().await?;
    println!("OPENING the connection");
    let connection: ImplicitConnection = forward_open_implicit(
        &mut session,
        args.forward_open_request()?,
        O2T_REAL_TIME_FORMAT,
        T2O_REAL_TIME_FORMAT,
    )
    .await?;
    let established_at = Instant::now();
    println!(
        "  O->T connection {:#010x} every {} us to {}",
        connection.response.o2t_network_connection_id,
        connection.response.o2t_actual_packet_interval,
        connection.o2t_endpoint
    );
    println!(
        "  T->O connection {:#010x} every {} us from {}",
        connection.response.t2o_network_connection_id,
        connection.response.t2o_actual_packet_interval,
        connection.target_ip
    );

    // ========= 3. Exchange I/O ============
    // The outputs: the encapsulation sequence number starts at random and advances every
    // packet; the CIP sequence count advances when the outputs change, here every cycle
    let mut send_timer = interval(Duration::from_micros(u64::from(
        connection.response.o2t_actual_packet_interval,
    )));
    let mut encapsulation_sequence_number: u32 = rand::random();
    let mut cip_sequence_count: u16 = 0;
    let mut outputs = vec![0u8; usize::from(DATA_SIZE)];

    // The inputs: the last accepted numbers, and the deadline every accepted packet moves
    let timeout: Duration = input_timeout(&connection);
    let mut last_sequence_number: Option<u32> = None;
    let mut last_cip_sequence_count: Option<u16> = None;
    let mut deadline = established_at + FIRST_PACKET_GRACE.max(timeout);

    // One loop drives both directions; production code may send outputs from their own task
    let mut cycle: u32 = 0;
    // One Ctrl+C future for the whole loop, so no press is lost; the connection is still closed
    let ctrl_c = ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = send_timer.tick() => {
                if cycle == args.cycles {
                    break;
                }
                cycle += 1;
                // Something that changes every cycle, so the echo can be told apart
                for (i, byte) in outputs.iter_mut().enumerate() {
                    *byte = (cycle as u8).wrapping_add(i as u8);
                }
                cip_sequence_count = cip_sequence_count.wrapping_add(1);
                let run_or_idle = if args
                    .toggle_every
                    .is_none_or(|every| ((cycle - 1) / every).is_multiple_of(2))
                {
                    RunOrIdle::Run
                } else {
                    RunOrIdle::Idle
                };
                let packet: EnIpIoPacket = build_o2t_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    CipDataOpt::Raw(outputs.clone()),
                    run_or_idle,
                )?;
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
                println!("[{cycle:>3}] SENT     {} ({run_or_idle:?})", hex(&outputs));
            }

            received = recv_io_packet(&socket) => {
                // A datagram that is not an I/O packet is logged and skipped, like a discarded one
                let (packet, from): (EnIpIoPacket, SocketAddr) = match received {
                    Ok(received) => received,
                    Err(Error::Parse(error)) => {
                        eprintln!("[{cycle:>3}] DISCARDED a datagram that does not parse: {error}");
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                match accept_t2o_packet(&connection, &packet, from, last_sequence_number) {
                    Ok((address, inputs)) => {
                        last_sequence_number = Some(address.encapsulation_sequence_number);
                        deadline = Instant::now() + timeout;
                        // An unchanged CIP sequence count means the adapter resent old inputs
                        let unchanged = last_cip_sequence_count.is_some()
                            && inputs.cip_sequence_count == last_cip_sequence_count;
                        last_cip_sequence_count = inputs.cip_sequence_count;
                        // Read from the wire, so the data is raw bytes
                        if let CipDataOpt::Raw(data) = &inputs.data {
                            println!(
                                "[{cycle:>3}] RECEIVED {}{}",
                                hex(data),
                                if unchanged { " (unchanged)" } else { "" }
                            );
                        }
                    }
                    Err(discarded) => eprintln!("[{cycle:>3}] DISCARDED {discarded}"),
                }
            }

            _ = sleep_until(tokio::time::Instant::from_std(deadline)) => {
                eprintln!("the connection timed out: no input packet in time");
                break;
            }

            _ = &mut ctrl_c => {
                println!("Ctrl+C: stopping");
                break;
            }
        }
    }

    // ========= 4. Close the connection ============
    println!("CLOSING the connection");
    forward_close(&mut session, &connection.request).await?;

    // ========= 5. End the session ============
    println!("UNREGISTERING the session");
    session.unregister().await?;

    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
