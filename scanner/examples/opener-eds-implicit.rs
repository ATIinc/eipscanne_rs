//! The stages of `opener-implicit`, with the connection read from this crate's
//! `sample_adapter.eds` fixture, the EDS of the OpENer sample.
//!
//! Start OpENer (`tests/integration`) on the host, then run the example in the devcontainer:
//!
//! ```text
//! tests/integration/start-opener.sh
//! cargo run --example opener-eds-implicit
//! ```
//!
//! Without OpENer, run it against the adapter crate's sample:
//!
//! ```text
//! cargo run --example sample-implicit-adapter
//! cargo run --example opener-eds-implicit -- --host 127.0.0.1 --run
//! ```
//!
//! Each cycle prints a `SENT` line, and OpENer answers with a `RECEIVED` line echoing the outputs.
//! The outputs are zeros, sent idle (run flag cleared) unless `--run` is given.

use std::fs::read_to_string;
use std::time::{Duration, Instant};

use bilge::prelude::u4;
use clap::Parser;
use clap::builder::{BoolishValueParser, TypedValueParser};
use tokio::signal::ctrl_c;
use tokio::time::{interval, sleep_until};

use eds_parser::Eds;
use eds_parser::connection_manager::ConnectionEntry;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, PriorityTimeTick, RealTimeFormat,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::io_data::RunOrIdle;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::error::Error;
use scanner::implicit::connection::{forward_close, forward_open_implicit};
use scanner::implicit::eds::{
    first_openable_connection, forward_open_request, o2t_real_time_format, t2o_real_time_format,
};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{
    FIRST_PACKET_GRACE, accept_t2o_packet, bind_io_socket, input_timeout, recv_io_packet,
};
use scanner::session::{Session, SessionConfig};

/// Who this scanner says it is in the Forward_Open (the same values as `opener-implicit`)
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;

/// The fixture describing the OpENer sample application
const SAMPLE_ADAPTER_EDS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sample_adapter.eds"
);

/// Exchanges cyclic I/O with the OpENer sample over a connection its EDS file describes
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The N of the `ConnectionN` to open. Default: the first connection this scanner can open
    #[arg(long)]
    connection: Option<u32>,

    /// IP address of the OpENer adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,

    /// Number of output packets to send before closing
    #[arg(long, default_value_t = 10)]
    cycles: u32,

    /// `true`: open with a Large_Forward_Open (32-bit connection parameters); `false`: with a
    /// Forward_Open. Default: a Large_Forward_Open only when a direction's size needs it
    #[arg(long)]
    large: Option<bool>,

    /// Send the outputs with the run flag set instead of idle
    #[arg(
        long = "run",
        action = clap::ArgAction::SetTrue,
        value_parser = BoolishValueParser::new()
            .map(|run| if run { RunOrIdle::Run } else { RunOrIdle::Idle }),
    )]
    run_or_idle: RunOrIdle,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // ========= Read the EDS and pick the connection ============
    let text = read_to_string(SAMPLE_ADAPTER_EDS)?;
    let eds = Eds::parse(&text)?;
    let number: u32 = match args.connection {
        Some(number) => number,
        None => first_openable_connection(&eds, args.large)?,
    };
    let connection: &ConnectionEntry = eds
        .connections
        .get(&number)
        .ok_or_else(|| format!("Connection{number}: the EDS has no such connection"))?;

    let request = ForwardOpenRequest {
        // 1024 ms per tick (1 ms shifted left by 10), 5 ticks until the request itself times out
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
        // The RPIs, network connection parameters, transport class and trigger, and path
        ..forward_open_request(&eds, connection, args.large)?
    };
    let o2t_real_time_format: RealTimeFormat = o2t_real_time_format(connection)?;
    let t2o_real_time_format: RealTimeFormat = t2o_real_time_format(connection)?;
    let outputs =
        vec![0u8; usize::from(eds.data_size(connection.o2t_size, connection.o2t_format)?)];

    println!(
        "Connection{number} ({}) from {}",
        connection.name, SAMPLE_ADAPTER_EDS
    );
    println!("{request:#?}");

    // Without a run/idle header there is no way to tell the adapter the outputs are not meant
    if args.run_or_idle == RunOrIdle::Idle && o2t_real_time_format != RealTimeFormat::Header32Bit {
        return Err(
            "the O->T direction has no run/idle header, so idle cannot be signalled: pass --run to send the outputs as real"
                .into(),
        );
    }

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register(
        (args.host.as_str(), ETHERNET_IP_TCP_PORT),
        SessionConfig::default(),
    )
    .await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket = bind_io_socket().await?;
    println!("OPENING the connection");
    let connection = forward_open_implicit(
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
    // The same loop as opener-implicit, kept here so the example reads top to bottom. The outputs
    // never change, so the CIP sequence count stays put after the first packet
    let mut send_timer = interval(Duration::from_micros(u64::from(
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
    let ctrl_c = ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = send_timer.tick() => {
                if cycle == args.cycles {
                    break;
                }
                cycle += 1;
                let packet = build_o2t_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    CipDataOpt::Raw(outputs.clone()),
                    args.run_or_idle,
                )?;
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
                println!("[{cycle:>3}] SENT     {} ({:?})", hex(&outputs), args.run_or_idle);
            }

            received = recv_io_packet(&socket) => {
                // A datagram that is not an I/O packet is logged and skipped, like a discarded one
                let (packet, from) = match received {
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
