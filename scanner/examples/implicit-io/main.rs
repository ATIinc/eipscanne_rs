//! Opens a class 1 connection to an adapter, exchanges cyclic I/O with it for a number of cycles
//! and closes it again. `main` is the five stages of implicit messaging in order.
//!
//! The defaults match the OpENer sample application (`tests/integration`), which copies the
//! outputs it receives on assembly 150 into the inputs it sends from assembly 100:
//!
//! `cargo run --example implicit-io -- --host 172.28.0.10`

use std::time::Instant;

use clap::Parser;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ProductionTrigger,
    RealTimeFormat, TransportClass,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::implicit::{
    ConnectionConfig, Consumer, DirectionConfig, Producer, bind_io_socket, forward_close,
    forward_open, recv_io_packet, send_io_packet,
};
use scanner::session::Session;

/// Who this scanner says it is in the Forward_Open; the adapter matches the Forward_Close
/// against the same values
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
/// The connection ID the adapter puts on every input packet
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;

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
    /// Class 1 cyclic, point-to-point both ways, a run/idle header on the outputs only, with the
    /// connection timing out after four missed packets
    fn connection_config(&self) -> ConnectionConfig {
        let requested_packet_interval = self.rpi * 1000;
        ConnectionConfig {
            configuration_instance: self.configuration_instance,
            o2t: DirectionConfig {
                connection_point: self.output_instance,
                data_size: self.output_size,
                requested_packet_interval,
                real_time_format: RealTimeFormat::Header32Bit,
                connection_size_type: ConnectionSizeType::Fixed,
            },
            t2o: DirectionConfig {
                connection_point: self.input_instance,
                data_size: self.input_size,
                requested_packet_interval,
                real_time_format: RealTimeFormat::Modeless,
                connection_size_type: ConnectionSizeType::Fixed,
            },
            transport_class: TransportClass::Class1,
            production_trigger: ProductionTrigger::Cyclic,
            priority: ConnectionPriority::Scheduled,
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
            },
            large_forward_open: self.large,
        }
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
    let connection = forward_open(&mut session, args.connection_config()).await?;
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
    // A random starting number so a restarted scanner does not repeat what the adapter last saw
    let mut producer = Producer::new(&connection, rand::random());
    let mut consumer = Consumer::new(&connection, established_at);

    // One loop drives both directions. Production code may instead run the producer on a task of
    // its own, so a slow input handler can never delay an output packet.
    let mut send_timer = tokio::time::interval(producer.period());
    let mut outputs = vec![0u8; usize::from(args.output_size)];
    let mut cycle: u32 = 0;
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
                let packet = producer.next_packet(&outputs, true)?;
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
                println!("[{cycle:>3}] SENT     {}", hex(&outputs));
            }

            received = recv_io_packet(&socket) => {
                let (packet, from) = received?;
                match consumer.accept(&packet, from, Instant::now()) {
                    Ok(input) => println!(
                        "[{cycle:>3}] RECEIVED {}{}",
                        hex(&input.data),
                        if input.new_data { "" } else { " (unchanged)" }
                    ),
                    Err(discarded) => eprintln!("[{cycle:>3}] DISCARDED {discarded}"),
                }
            }

            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(consumer.deadline())) => {
                eprintln!("the connection timed out: no input packet in time");
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
