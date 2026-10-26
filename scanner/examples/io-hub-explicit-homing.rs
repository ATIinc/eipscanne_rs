//! Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E over explicit messaging: register a
//! session, clear a shutdown if one is present, enable the motor, send the homing command, read
//! the inputs until the motor reports it has homed, then disable the motor again. The motor is
//! disabled however the motion ends: homed, timed out, failed or stopped with Ctrl+C.
//!
//! This moves a real motor. `--host` has no default on purpose.
//!
//! `cargo run --example io-hub-explicit-homing -- --host 172.31.19.18 --motor 0`

use std::time::{Duration, Instant};

use clap::Parser;
use tokio::signal::ctrl_c;
use tokio::time::sleep;

use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{get_attribute, set_attribute};
use scanner::session::{Session, SessionConfig};

// The IO-HUB assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../assemblies"]
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

    /// How long to wait for the motor to home after the command, in seconds
    #[arg(long, default_value_t = 10)]
    timeout_s: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // The two assemblies this example talks to
    let input_path = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        INPUT_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );
    let output_path = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        OUTPUT_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register(
        (args.host.as_str(), ETHERNET_IP_TCP_PORT),
        SessionConfig::default(),
    )
    .await?;

    // Every write sends the whole output assembly, so it is kept here and changed in place
    let mut outputs = OutputAssemblyHub4E::default();

    // ========= Clear a shutdown, if one is present ============
    let inputs: InputAssemblyHub4E = get_attribute(&mut session, &input_path).await?;
    if inputs
        .motor_input(args.motor)
        .statusword
        .motor_shutdown_present()
    {
        println!("REQUESTING - SET shutdown reset");
        let motor = outputs.motor_output_mut(args.motor);
        motor.controlword.set_shutdown_reset(true);
        set_attribute(&mut session, &output_path, outputs.clone()).await?;
        sleep(SHUTDOWN_RESET_TIME).await;

        // Lowered again so the next reset has a rising edge
        let motor = outputs.motor_output_mut(args.motor);
        motor.controlword.set_shutdown_reset(false);
        set_attribute(&mut session, &output_path, outputs.clone()).await?;
    }

    // The motion steps run until they finish, fail or Ctrl+C is pressed; the motor is disabled
    // below in every case
    let homing = async {
        // ========= Enable the motor ============
        println!("REQUESTING - SET enable motor {}", args.motor);
        outputs
            .motor_output_mut(args.motor)
            .controlword
            .set_enable(true);
        set_attribute(&mut session, &output_path, outputs.clone()).await?;

        // ========= Send the homing command ============
        // The hub acts on a move when its move number changes, so the command takes the next one
        let inputs: InputAssemblyHub4E = get_attribute(&mut session, &input_path).await?;
        let move_number = inputs
            .motor_input(args.motor)
            .move_number_ack
            .wrapping_add(1);
        println!("REQUESTING - HOMING move {move_number}");
        outputs
            .motor_output_mut(args.motor)
            .command_move(MoveType::HomingMove, move_number);
        set_attribute(&mut session, &output_path, outputs.clone()).await?;

        // ========= Wait for the motor to home ============
        let deadline = Instant::now() + Duration::from_secs(args.timeout_s);
        loop {
            let inputs: InputAssemblyHub4E = get_attribute(&mut session, &input_path).await?;
            let motor = inputs.motor_input(args.motor);
            print_status(motor);

            if motor.statusword.has_homed() {
                println!("HOMED");
                break;
            }
            if Instant::now() >= deadline {
                println!("NOT HOMED within {} s of the command", args.timeout_s);
                break;
            }
            sleep(POLL_INTERVAL).await;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };
    let outcome = tokio::select! {
        result = homing => result,
        _ = ctrl_c() => {
            println!("Ctrl+C: stopping");
            Ok(())
        }
    };
    if let Err(error) = &outcome {
        eprintln!("homing failed: {error}");
    }

    // ========= Disable the motor (always) ============
    println!("REQUESTING - SET disable motor {}", args.motor);
    outputs
        .motor_output_mut(args.motor)
        .controlword
        .set_enable(false);
    set_attribute(&mut session, &output_path, outputs).await?;

    // ========= UnRegister the session ============
    println!("REQUESTING - UN REGISTER session");
    session.unregister().await?;

    outcome
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
