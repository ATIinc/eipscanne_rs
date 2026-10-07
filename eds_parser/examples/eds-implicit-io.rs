//! Opens the class 1 connection an EDS file describes, exchanges cyclic I/O for a number of
//! cycles and closes it again. The stages are those of the scanner's `implicit-io` example; only
//! the connection settings come from the EDS instead of flags.
//!
//! The outputs sent are zeros, and the scanner stays idle (run flag cleared) unless `--run` is
//! given: an output assembly may drive real outputs.
//!
//! `cargo run --example eds-implicit-io -- --eds eds_parser/tests/fixtures/sample_adapter.eds --host 172.28.0.10 --run`

use std::time::{Duration, Instant};

use bilge::prelude::u4;
use clap::Parser;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, PriorityTimeTick, RealTimeFormat,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::implicit::{
    FIRST_PACKET_GRACE, accept_input, bind_io_socket, forward_close, forward_open, input_timeout,
    output_packet, recv_io_packet, send_io_packet,
};
use scanner::session::Session;

use eds_parser::{Eds, OriginatorSettings, to_forward_open};

/// Who this scanner says it is in the Forward_Open (the same values as `implicit-io`)
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;

/// Exchanges cyclic I/O with an adapter over the connection its EDS file describes
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The device's EDS file
    #[arg(long)]
    eds: std::path::PathBuf,

    /// The connection to open: its `ConnectionN` keyword or its name. Default: the first
    /// exclusive-owner connection
    #[arg(long)]
    connection: Option<String>,

    /// IP address of the adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,

    /// Number of output packets to send before closing
    #[arg(long, default_value_t = 10)]
    cycles: u32,

    /// Open with a Large_Forward_Open (32-bit connection parameters)
    #[arg(long)]
    large: bool,

    /// Send the outputs with the run flag set instead of idle
    #[arg(long)]
    run: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Read the EDS and pick the connection ============
    let text = std::fs::read_to_string(&args.eds)?;
    let eds = Eds::parse(&text)?;
    let connection = match &args.connection {
        Some(wanted) => eds
            .connection(wanted)
            .ok_or_else(|| format!("the EDS has no connection called {wanted}"))?,
        None => eds
            .first_exclusive_owner_connection()
            .ok_or("the EDS offers no exclusive-owner connection")?,
    };

    let (request, o2t_real_time_format, t2o_real_time_format) = to_forward_open(
        connection,
        OriginatorSettings {
            // 1024 ms per tick (1 ms shifted left by 10), 5 ticks until the request itself
            // times out
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
            },
            large_forward_open: args.large,
        },
    )?;

    println!(
        "{} ({}) from {}",
        connection.name,
        connection.keyword,
        args.eds.display()
    );
    println!("  path {}", hex(&connection.path));
    println!(
        "  O->T {} bytes every {} us, {o2t_real_time_format:?}",
        connection.o2t.size, request.o2t_requested_packet_interval
    );
    println!(
        "  T->O {} bytes every {} us, {t2o_real_time_format:?}",
        connection.t2o.size, request.t2o_requested_packet_interval
    );
    println!("  {request:#?}");

    // Without a run/idle header there is no way to tell the adapter the outputs are not meant
    if !args.run && o2t_real_time_format != RealTimeFormat::Header32Bit {
        return Err(
            "the O->T direction has no run/idle header, so idle cannot be signalled: pass --run to send the outputs as real"
                .into(),
        );
    }

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket = bind_io_socket().await?;
    println!("OPENING the connection");
    let outputs = vec![0u8; usize::from(connection.o2t.size)];
    let connection = forward_open(
        &mut session,
        request,
        o2t_real_time_format,
        t2o_real_time_format,
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
    // The same loop as implicit-io, kept here so the example reads top to bottom. The outputs
    // never change, so the CIP sequence count stays put after the first packet
    let mut send_timer = tokio::time::interval(Duration::from_micros(u64::from(
        connection.response.o2t_actual_packet_interval,
    )));
    let mut encapsulation_sequence_number: u32 = rand::random();
    let cip_sequence_count: u16 = 1;

    let timeout = input_timeout(&connection);
    let mut last_sequence_number = None;
    let mut last_cip_sequence_count = None;
    let mut deadline = established_at + FIRST_PACKET_GRACE.max(timeout);

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
                let packet = output_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    CipDataOpt::Raw(outputs.clone()),
                    args.run,
                )?;
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
                println!("[{cycle:>3}] SENT     {} ({})", hex(&outputs), if args.run { "run" } else { "idle" });
            }

            received = recv_io_packet(&socket) => {
                let (packet, from) = received?;
                match accept_input(&connection, last_sequence_number, &packet, from) {
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
