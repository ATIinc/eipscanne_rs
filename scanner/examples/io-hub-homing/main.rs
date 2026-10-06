//! Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E over explicit messaging: register a
//! session, clear a shutdown if one is present, enable the motor, send the homing command, read
//! the inputs until the motor reports it has homed, then disable the motor again.
//!
//! `--repeat N` sends N homing commands back to back, each before the previous one has finished,
//! which reproduces a firmware bug: depending on the version the motor cancels the active homing
//! move, never asserts `has_homed`, or faults.
//!
//! This moves a real motor. `--host` has no default on purpose.
//!
//! `cargo run --example io-hub-homing -- --host 172.31.19.18 --motor 0`
//! `cargo run --example io-hub-homing -- --host 172.31.19.18 --motor 0 --repeat 4`

use std::time::{Duration, Instant};

use clap::Parser;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{decode_reply, send_request};
use scanner::session::Session;

// The IO-HUB assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../assemblies"]
mod assemblies {
    pub mod io_hub;
}

use assemblies::io_hub::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyHub4E, MotorInputData};
use assemblies::io_hub::output::{MoveType, OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyHub4E};

/// How long the hub gets to act on a Shutdown Reset before the bit is lowered again
const SHUTDOWN_RESET_TIME: Duration = Duration::from_millis(200);
/// How often the inputs are read while waiting for the motor to home
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the IO-HUB-4-E
    #[arg(long)]
    host: String,

    /// Motor port to home (0 = M0 ... 3 = M3)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..4))]
    motor: u8,

    /// Number of homing commands to send back to back; more than 1 reproduces the firmware bug
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    repeat: u32,

    /// Pause between repeated homing commands, in milliseconds
    #[arg(long, default_value_t = 10)]
    delay_ms: u64,

    /// How long to wait for the motor to home after the last command, in seconds
    #[arg(long, default_value_t = 10)]
    timeout_s: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // Every write sends the whole output assembly, so it is kept here and changed in place
    let mut outputs = OutputAssemblyHub4E::default();

    // ========= Clear a shutdown, if one is present ============
    let inputs = read_inputs(&mut session).await?;
    if inputs
        .motor_input(args.motor)
        .statusword
        .motor_shutdown_present()
    {
        println!("REQUESTING - SET shutdown reset");
        let motor = outputs.motor_output_mut(args.motor);
        motor.controlword.set_shutdown_reset(true);
        write_outputs(&mut session, &outputs).await?;
        tokio::time::sleep(SHUTDOWN_RESET_TIME).await;

        // Lowered again so the next reset has a rising edge
        let motor = outputs.motor_output_mut(args.motor);
        motor.controlword.set_shutdown_reset(false);
        write_outputs(&mut session, &outputs).await?;
    }

    // ========= Enable the motor ============
    println!("REQUESTING - SET enable motor {}", args.motor);
    outputs
        .motor_output_mut(args.motor)
        .controlword
        .set_enable(true);
    write_outputs(&mut session, &outputs).await?;

    // ========= Send the homing command(s) ============
    // The hub acts on a move when its move number changes, so each command takes the next one
    let inputs = read_inputs(&mut session).await?;
    let mut move_number = inputs.motor_input(args.motor).move_number_ack;
    for i in 1..=args.repeat {
        move_number = move_number.wrapping_add(1);
        outputs
            .motor_output_mut(args.motor)
            .command_move(MoveType::HomingMove, move_number);
        write_outputs(&mut session, &outputs).await?;
        println!(
            "REQUESTING - HOMING move {move_number} ({i}/{})",
            args.repeat
        );
        tokio::time::sleep(Duration::from_millis(args.delay_ms)).await;
    }

    // ========= Wait for the motor to home ============
    let deadline = Instant::now() + Duration::from_secs(args.timeout_s);
    loop {
        let inputs = read_inputs(&mut session).await?;
        let motor = inputs.motor_input(args.motor);
        print_status(motor);

        if motor.statusword.has_homed() {
            println!("HOMED");
            break;
        }
        if Instant::now() >= deadline {
            println!("NOT HOMED within {} s of the last command", args.timeout_s);
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    // ========= Disable the motor ============
    println!("REQUESTING - SET disable motor {}", args.motor);
    outputs
        .motor_output_mut(args.motor)
        .controlword
        .set_enable(false);
    write_outputs(&mut session, &outputs).await?;

    // ========= UnRegister the session ============
    println!("REQUESTING - UN REGISTER session");
    session.unregister().await?;

    Ok(())
}

/// Get_Attribute_Single on the input assembly
async fn read_inputs(
    session: &mut Session,
) -> Result<InputAssemblyHub4E, Box<dyn std::error::Error>> {
    let reply = send_request(
        session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            INPUT_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;
    Ok(decode_reply(&reply)?)
}

/// Set_Attribute_Single on the output assembly
async fn write_outputs(
    session: &mut Session,
    outputs: &OutputAssemblyHub4E,
) -> Result<(), Box<dyn std::error::Error>> {
    send_request(
        session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            OUTPUT_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(outputs.clone())),
    )
    .await?;
    Ok(())
}

/// The status bits that tell how homing is going, on one line
fn print_status(motor: &MotorInputData) {
    let status = motor.statusword;
    let rejection = match motor.rejection() {
        Some(code) => format!(" rejected: {code:?}"),
        None => String::new(),
    };
    println!(
        "  homing={} has_homed={} in_home_sensor={} enabled={} shutdown={} cancelled={} | move {} ack {}{rejection}",
        u8::from(status.homing()),
        u8::from(status.has_homed()),
        u8::from(status.in_home_sensor()),
        u8::from(status.enabled()),
        u8::from(status.motor_shutdown_present()),
        u8::from(status.move_cancelled()),
        motor.move_number_ack,
        motor.move_type_ack,
    );
}
