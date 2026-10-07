//! Opens a class 1 connection to an adapter, exchanges cyclic I/O with it for a number of cycles
//! and closes it again. `main` is the five stages of implicit messaging in order.
//!
//! The defaults match the OpENer sample application (`tests/integration`), which copies the
//! outputs it receives on assembly 150 into the inputs it sends from assembly 100:
//!
//! `cargo run --example implicit-io -- --host 172.28.0.10`

use std::time::{Duration, Instant};

use bilge::prelude::{u4, u9};
use clap::Parser;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    LargeNetworkConnectionParameters, NetworkConnectionParameters, PriorityTimeTick,
    ProductionTrigger, RealTimeFormat, RedundantOwner, StandardNetworkConnectionParameters,
    TransportClass, TransportTypeTrigger, connection_size,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::implicit::connection::{forward_close, forward_open};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{
    FIRST_PACKET_GRACE, accept_t2o_packet, bind_io_socket, input_timeout, recv_io_packet,
};
use scanner::session::Session;

/// Who this scanner says it is in the Forward_Open; the adapter matches the Forward_Close
/// against the same values
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
/// The connection ID the adapter puts on every input packet
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;
/// How each direction signals run/idle. Not part of the Forward_Open: both ends agree on it
/// beforehand (an EDS file or the device manual says which one). The OpENer sample takes a
/// run/idle header on the outputs and sends none on the inputs.
const O2T_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Header32Bit;
const T2O_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Modeless;

/// Exchanges cyclic I/O with an EtherNet/IP adapter over a class 1 connection
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,

    /// Assembly instance holding the configuration
    #[arg(long, default_value_t = 151)]
    configuration_instance: u8,

    /// Assembly instance that consumes the outputs (O->T)
    #[arg(long, default_value_t = 150)]
    output_instance: u8,

    /// Assembly instance that produces the inputs (T->O)
    #[arg(long, default_value_t = 100)]
    input_instance: u8,

    /// Output bytes per packet
    #[arg(long, default_value_t = 32)]
    output_size: u16,

    /// Input bytes per packet
    #[arg(long, default_value_t = 32)]
    input_size: u16,

    /// Requested packet interval of both directions, in milliseconds
    #[arg(long, default_value_t = 1000)]
    rpi: u32,

    /// Number of output packets to send before closing
    #[arg(long, default_value_t = 10)]
    cycles: u32,

    /// Open with a Large_Forward_Open (32-bit connection parameters)
    #[arg(long)]
    large: bool,
}

impl Args {
    /// The Forward_Open for a class 1 cyclic connection, point-to-point both ways, timing out
    /// after four missed packets
    fn forward_open_request(&self) -> Result<ForwardOpenRequest, Box<dyn std::error::Error>> {
        let requested_packet_interval = self.rpi * 1000;
        // Each connection size counts the 16-bit sequence count and the real-time header too
        let o2t_size = connection_size(
            self.output_size,
            TransportClass::Class1,
            O2T_REAL_TIME_FORMAT,
        );
        let t2o_size = connection_size(
            self.input_size,
            TransportClass::Class1,
            T2O_REAL_TIME_FORMAT,
        );

        Ok(ForwardOpenRequest {
            // 1024 ms per tick (1 ms shifted left by 10), 5 ticks until the request itself
            // times out
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
                self.configuration_instance,
                self.output_instance,
                self.input_instance,
            ),
        })
    }

    /// The parameter word of one direction: 32 bits for a Large_Forward_Open, otherwise 16 bits
    /// with only 9 for the size
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
    let args = Args::parse();

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket = bind_io_socket().await?;
    println!("OPENING the connection");
    let connection = forward_open(
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
    // The outputs: every packet gets the next encapsulation sequence number, starting at a
    // random one so a restarted scanner does not repeat what the adapter last saw; the CIP
    // sequence count moves whenever the outputs change, which here is every cycle
    let mut send_timer = tokio::time::interval(Duration::from_micros(u64::from(
        connection.response.o2t_actual_packet_interval,
    )));
    let mut encapsulation_sequence_number: u32 = rand::random();
    let mut cip_sequence_count: u16 = 0;
    let mut outputs = vec![0u8; usize::from(args.output_size)];

    // The inputs: the last accepted numbers, and the deadline every accepted packet moves
    let timeout = input_timeout(&connection);
    let mut last_sequence_number = None;
    let mut last_cip_sequence_count = None;
    let mut deadline = established_at + FIRST_PACKET_GRACE.max(timeout);

    // One loop drives both directions. Production code may instead send the outputs from a task
    // of its own, so a slow input handler can never delay an output packet.
    let mut cycle: u32 = 0;
    // One Ctrl+C future for the whole loop, so a press is not lost between two iterations;
    // Ctrl+C ends the exchange and the connection is still closed below
    let ctrl_c = tokio::signal::ctrl_c();
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
                let packet = build_o2t_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    CipDataOpt::Raw(outputs.clone()),
                    true,
                )?;
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
                println!("[{cycle:>3}] SENT     {}", hex(&outputs));
            }

            received = recv_io_packet(&socket) => {
                let (packet, from) = received?;
                match accept_t2o_packet(&connection, last_sequence_number, &packet, from) {
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

            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
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
    forward_close(&mut session, &connection).await?;

    // ========= 5. End the session ============
    println!("UNREGISTERING the session");
    session.unregister().await?;

    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
