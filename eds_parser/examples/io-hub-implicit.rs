//! Watches a Teknic IO-HUB-4-E over a class 1 I/O connection, with the same assembly structs the
//! explicit examples read and write. The connection comes from the hub's EDS file, the structs
//! are checked against that file before the connection is opened, and every input packet is
//! decoded as an `InputAssemblyHub4E`. One motor's status is printed whenever it changes.
//!
//! The outputs sent are `OutputAssemblyHub4E::default()`: no motor enabled, every output off,
//! sent idle (run flag cleared) unless `--run` is given. `--host` has no default on purpose.
//!
//! `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host 172.31.19.18 --motor 0`

use std::time::Instant;

use clap::Parser;

use eipscanne_rs::cip::connection_manager::parameters::ConnectionTimeoutMultiplier;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::implicit::{
    Consumer, Producer, bind_io_socket, forward_close, forward_open, recv_io_packet, send_io_packet,
};
use scanner::session::Session;

use eds_parser::{Eds, OriginatorSettings, check_assembly, to_connection_config};

// The IO-HUB assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../scanner/assemblies"]
mod assemblies {
    pub mod io_hub;
}

use assemblies::io_hub::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyHub4E, MotorInputData};
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Read the EDS and derive the connection ============
    let text = std::fs::read_to_string(&args.eds)?;
    let eds = Eds::parse(&text)?;
    let connection = eds
        .first_exclusive_owner_connection()
        .ok_or("the EDS offers no exclusive-owner connection")?;
    let config = to_connection_config(
        connection,
        OriginatorSettings {
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
        "{} ({}): O->T assembly {}, T->O assembly {}",
        connection.name,
        connection.keyword,
        config.o2t.connection_point,
        config.t2o.connection_point
    );

    // ========= Check the structs against the EDS ============
    // The connection must carry the assemblies the structs are written for, with their layout
    if config.t2o.connection_point != INPUT_ASSEMBLY_INSTANCE
        || config.o2t.connection_point != OUTPUT_ASSEMBLY_INSTANCE
    {
        return Err(format!(
            "the connection carries assemblies {} and {}, the structs are for {INPUT_ASSEMBLY_INSTANCE} and {OUTPUT_ASSEMBLY_INSTANCE}",
            config.t2o.connection_point, config.o2t.connection_point
        )
        .into());
    }
    let (Some(input_format), Some(output_format)) =
        (&connection.t2o.format, &connection.o2t.format)
    else {
        return Err(format!("{} names no assembly for its data", connection.keyword).into());
    };
    let input_layout = eds
        .assembly(input_format)
        .ok_or_else(|| format!("the EDS has no {input_format}"))?;
    let output_layout = eds
        .assembly(output_format)
        .ok_or_else(|| format!("the EDS has no {output_format}"))?;
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
    let connection = forward_open(&mut session, config).await?;
    let established_at = Instant::now();
    println!(
        "  outputs every {} us, inputs every {} us",
        connection.response.o2t_actual_packet_interval,
        connection.response.t2o_actual_packet_interval
    );

    // ========= 3. Exchange I/O ============
    // A random starting number so a restarted scanner does not repeat what the adapter last saw
    let mut producer = Producer::new(&connection, rand::random());
    let mut consumer = Consumer::new(&connection, established_at);

    let mut send_timer = tokio::time::interval(producer.period());
    let outputs = OutputAssemblyHub4E::default();
    let mut last_motor: Option<MotorInputData> = None;
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
                let packet = producer.next_packet_from(&outputs, args.run)?;
                send_io_packet(&socket, &packet, connection.o2t_endpoint).await?;
            }

            received = recv_io_packet(&socket) => {
                let (packet, from) = received?;
                let input = match consumer.accept(&packet, from, Instant::now()) {
                    Ok(input) => input,
                    Err(discarded) => {
                        eprintln!("[{cycle:>4}] DISCARDED {discarded}");
                        continue;
                    }
                };
                // The same struct `io-hub-homing` reads with Get_Attribute_Single
                let inputs = match input.decode::<InputAssemblyHub4E>() {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        eprintln!("[{cycle:>4}] UNDECODED {error}");
                        continue;
                    }
                };
                let motor = inputs.motor_input(args.motor);
                if last_motor.as_ref() != Some(motor) {
                    let status = motor.statusword;
                    println!(
                        "[{cycle:>4}] M{} connected={} enabled={} ready={} has_homed={} shutdown={} position={}",
                        args.motor,
                        u8::from(status.motor_connected()),
                        u8::from(status.enabled()),
                        u8::from(status.ready_for_command()),
                        u8::from(status.has_homed()),
                        u8::from(status.motor_shutdown_present()),
                        motor.position_measured,
                    );
                    last_motor = Some(motor.clone());
                }
            }

            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(consumer.deadline())) => {
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
