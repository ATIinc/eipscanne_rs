//! Watches a Teknic IO-HUB-4-E over a class 1 I/O connection, with the same assembly structs the
//! explicit examples read and write. The connection comes from the hub's EDS file, the structs
//! are checked against that file before the connection is opened, and every input packet is
//! decoded as an `InputAssemblyHub4E`. One motor's status is printed whenever it changes.
//!
//! The outputs sent are `OutputAssemblyHub4E::default()`: no motor enabled, every output off,
//! sent idle (run flag cleared) unless `--run` is given. `--host` has no default on purpose.
//!
//! `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host 172.31.19.18 --motor 0`

use std::io::Cursor;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use bilge::prelude::u4;
use binrw::BinRead;
use clap::Parser;

use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionTimeoutMultiplier, PriorityTimeTick,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::error::Error;
use scanner::implicit::connection::{forward_close, forward_open};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{
    FIRST_PACKET_GRACE, accept_t2o_packet, bind_io_socket, input_timeout, recv_io_packet,
};
use scanner::session::Session;

use eds_parser::Eds;
use eds_parser::check::check_assembly;
use eds_parser::to_forward_open::{OriginatorSettings, to_forward_open};

// The IO-HUB assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../scanner/assemblies"]
mod assemblies {
    pub mod io_hub;
}

use assemblies::io_hub::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyHub4E};
use assemblies::io_hub::output::{OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyHub4E};

/// Who this scanner says it is in the Forward_Open (the same values as `implicit-io`)
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;

/// Watches a Teknic IO-HUB-4-E over a class 1 I/O connection
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The hub's EDS file
    #[arg(long)]
    eds: std::path::PathBuf,

    /// IP address of the IO-HUB-4-E
    #[arg(long)]
    host: String,

    /// Motor port to watch (0 = M0 ... 3 = M3)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..4))]
    motor: u8,

    /// Number of output packets to send before closing
    #[arg(long, default_value_t = 1000)]
    cycles: u32,

    /// Send the outputs with the run flag set instead of idle
    #[arg(long)]
    run: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // ========= Read the EDS and derive the connection ============
    let text = std::fs::read_to_string(&args.eds)
        .with_context(|| format!("reading {}", args.eds.display()))?;
    let eds = Eds::parse(&text)?;
    let connection = eds
        .first_exclusive_owner_connection()
        .context("the EDS offers no exclusive-owner connection")?;
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
            large_forward_open: false,
        },
    )?;
    println!(
        "{} ({}): path {}",
        connection.name,
        connection.keyword,
        hex(&connection.path)
    );

    // ========= Check the structs against the EDS ============
    // The connection must carry the assemblies the structs are written for, with their layout.
    // The path ends with the two connection points: 2C <O->T> 2C <T->O>
    if !connection.path.ends_with(&[
        0x2C,
        OUTPUT_ASSEMBLY_INSTANCE,
        0x2C,
        INPUT_ASSEMBLY_INSTANCE,
    ]) {
        bail!(
            "the connection path {} does not end with the assemblies the structs are for ({OUTPUT_ASSEMBLY_INSTANCE} and {INPUT_ASSEMBLY_INSTANCE})",
            hex(&connection.path)
        );
    }
    let (Some(input_format), Some(output_format)) =
        (&connection.t2o.format, &connection.o2t.format)
    else {
        bail!("{} names no assembly for its data", connection.keyword);
    };
    let input_layout = eds
        .assembly(input_format)
        .with_context(|| format!("the EDS has no {input_format}"))?;
    let output_layout = eds
        .assembly(output_format)
        .with_context(|| format!("the EDS has no {output_format}"))?;
    println!("CHECKING the structs against {input_format} and {output_format}");
    for finding in check_assembly::<InputAssemblyHub4E>(input_layout)? {
        println!("  {finding}");
    }
    for finding in check_assembly::<OutputAssemblyHub4E>(output_layout)? {
        println!("  {finding}");
    }

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket = bind_io_socket().await?;
    println!("OPENING the connection");
    let connection = forward_open(
        &mut session,
        request,
        o2t_real_time_format,
        t2o_real_time_format,
    )
    .await?;
    let established_at = Instant::now();
    println!(
        "  outputs every {} us, inputs every {} us",
        connection.response.o2t_actual_packet_interval,
        connection.response.t2o_actual_packet_interval
    );

    // ========= 3. Exchange I/O ============
    // The outputs never change, so the CIP sequence count stays put after the first packet;
    // every packet gets the next encapsulation sequence number, starting at a random one so a
    // restarted scanner does not repeat what the adapter last saw
    let mut send_timer = tokio::time::interval(Duration::from_micros(u64::from(
        connection.response.o2t_actual_packet_interval,
    )));
    let mut encapsulation_sequence_number: u32 = rand::random();
    let cip_sequence_count: u16 = 1;

    let timeout = input_timeout(&connection);
    let mut last_sequence_number = None;
    let mut deadline = established_at + FIRST_PACKET_GRACE.max(timeout);

    let mut last_line: Option<String> = None;
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
                let packet = build_o2t_packet(
                    &connection,
                    encapsulation_sequence_number,
                    cip_sequence_count,
                    CipDataOpt::Typed(Box::new(OutputAssemblyHub4E::default())),
                    args.run,
                )?;
                encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
            }

            received = recv_io_packet(&socket) => {
                // A datagram that is not an I/O packet is logged and skipped, like a discarded one
                let (packet, from) = match received {
                    Ok(received) => received,
                    Err(Error::Parse(error)) => {
                        eprintln!("[{cycle:>4}] DISCARDED a datagram that does not parse: {error}");
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                let (address, inputs) = match accept_t2o_packet(&connection, last_sequence_number, &packet, from) {
                    Ok(accepted) => accepted,
                    Err(discarded) => {
                        eprintln!("[{cycle:>4}] DISCARDED {discarded}");
                        continue;
                    }
                };
                last_sequence_number = Some(address.encapsulation_sequence_number);
                deadline = Instant::now() + timeout;
                // Read from the wire, so the data is raw bytes; the same struct `io-hub-homing`
                // reads with Get_Attribute_Single
                let CipDataOpt::Raw(data) = &inputs.data else {
                    continue;
                };
                let inputs = match InputAssemblyHub4E::read_le(&mut Cursor::new(data)) {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        eprintln!("[{cycle:>4}] UNDECODED {error}");
                        continue;
                    }
                };
                // Printed when it changes: measured values such as torque move all the time
                let motor = inputs.motor_input(args.motor);
                let status = motor.statusword;
                let line = format!(
                    "M{} connected={} enabled={} ready={} has_homed={} shutdown={} position={}",
                    args.motor,
                    u8::from(status.motor_connected()),
                    u8::from(status.enabled()),
                    u8::from(status.ready_for_command()),
                    u8::from(status.has_homed()),
                    u8::from(status.motor_shutdown_present()),
                    motor.position_measured,
                );
                if last_line.as_ref() != Some(&line) {
                    println!("[{cycle:>4}] {line}");
                    last_line = Some(line);
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
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
