//! Homes one motor of a Teknic ClearLink over explicit messaging: register a session, clear
//! faults if the motor reports any, write the homing configuration, enable the motor, start the
//! homing move and read the inputs until the motor reports it has homed, then disable the motor
//! again. The motor is disabled however the motion ends: homed, timed out, failed or stopped with
//! Ctrl+C.
//!
//! This moves a real motor. `--host` has no default on purpose.
//!
//! `cargo run --example clearlink-homing -- --host 172.31.19.14 --motor 1 --home-sensor 6`

use std::time::{Duration, Instant};

use clap::Parser;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{decode_reply, send_request};
use scanner::session::Session;

// The ClearLink assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../assemblies"]
mod assemblies {
    pub mod clearlink;
}

use assemblies::clearlink::config::{CONFIG_ASSEMBLY_INSTANCE, ConfigAssemblyObject};
use assemblies::clearlink::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyObject};
use assemblies::clearlink::output::{OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyObject};

/// How long the ClearLink gets to act on a configuration or a fault clear before the next write
const SETTLE_TIME: Duration = Duration::from_millis(200);
/// How often the inputs are read while waiting for a status bit
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Homes one motor of a Teknic ClearLink controller
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the ClearLink
    #[arg(long)]
    host: String,

    /// Motor connector to home (0 = M0 ... 3 = M3)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..4))]
    motor: u8,

    /// I/O connector (0-12) of the home sensor, or -1 to home against a hard stop
    #[arg(long, default_value_t = -1, value_parser = clap::value_parser!(i8).range(-1..13))]
    home_sensor: i8,

    /// Homing velocity, in steps per second (the sign gives the direction)
    #[arg(long, default_value_t = 2000)]
    velocity: i32,

    /// Acceleration and deceleration limit of the homing move, in steps per second squared
    #[arg(long, default_value_t = 100)]
    acceleration: u32,

    /// How long the whole homing may take, in seconds
    #[arg(long, default_value_t = 30)]
    timeout_s: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // The three assemblies this example talks to
    let config_assembly = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        CONFIG_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );
    let input_assembly = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        INPUT_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );
    let output_assembly = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        OUTPUT_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= Read the outputs ============
    // Every write sends the whole output assembly, so it starts from what the device has and only
    // the motor being homed is changed
    println!("REQUESTING - GET outputs");
    let reply = send_request(
        &mut session,
        output_assembly.clone(),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;
    let mut outputs: OutputAssemblyObject = decode_reply(&reply)?;

    // ========= Clear faults, if there are any ============
    let reply = send_request(
        &mut session,
        input_assembly.clone(),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;
    let inputs: InputAssemblyObject = decode_reply(&reply)?;
    let status = inputs.motor_input(args.motor).motor_status;
    if status.shutdowns_present() || status.motor_in_fault() {
        println!("REQUESTING - SET clear alerts and motor fault");
        let motor = outputs.motor_output_mut(args.motor);
        motor.output_register.set_enable(false);
        motor.output_register.set_clear_alerts(true);
        motor.output_register.set_clear_motor_fault(true);
        send_request(
            &mut session,
            output_assembly.clone(),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(outputs.clone())),
        )
        .await?;
        tokio::time::sleep(SETTLE_TIME).await;

        // Both bits act on their rising edge, so they are lowered again for the next clear
        let motor = outputs.motor_output_mut(args.motor);
        motor.output_register.set_clear_alerts(false);
        motor.output_register.set_clear_motor_fault(false);
        send_request(
            &mut session,
            output_assembly.clone(),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(outputs.clone())),
        )
        .await?;
    }

    // ========= Write the homing configuration ============
    // The configuration assembly is written whole, so the other motors and the I/O get the
    // defaults of `ConfigAssemblyObject::default()`
    println!(
        "REQUESTING - SET config: homing enabled, home sensor connector {}",
        args.home_sensor
    );
    let mut config = ConfigAssemblyObject::default();
    let motor_config = config.motor_config_mut(args.motor);
    motor_config.config_register.set_homing_enable(true);
    motor_config.home_sensor_connector = args.home_sensor;
    send_request(
        &mut session,
        config_assembly,
        ServiceCode::SetAttributeSingle,
        Some(Box::new(config)),
    )
    .await?;
    tokio::time::sleep(SETTLE_TIME).await;

    // The motion steps run until they finish, fail or Ctrl+C is pressed; the motor is disabled
    // below in every case
    let homing = async {
        let deadline = Instant::now() + Duration::from_secs(args.timeout_s);

        // ========= Enable the motor ============
        println!("REQUESTING - SET enable motor {}", args.motor);
        outputs
            .motor_output_mut(args.motor)
            .output_register
            .set_enable(true);
        send_request(
            &mut session,
            output_assembly.clone(),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(outputs.clone())),
        )
        .await?;

        // ========= Wait until the motor is ready to home ============
        loop {
            let reply = send_request(
                &mut session,
                input_assembly.clone(),
                ServiceCode::GetAttributeSingle,
                None,
            )
            .await?;
            let inputs: InputAssemblyObject = decode_reply(&reply)?;
            let status = inputs.motor_input(args.motor).motor_status;
            if status.ready_to_home() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!("the motor never became ready to home: {status:?}").into());
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }

        // ========= Start the homing move ============
        println!(
            "REQUESTING - SET homing move at {} steps/s, {} steps/s^2",
            args.velocity, args.acceleration
        );
        let motor = outputs.motor_output_mut(args.motor);
        motor.output_register.set_homing_move(true);
        motor.output_register.set_load_velocity_move(true);
        motor.jog_velocity = args.velocity;
        motor.acceleration_limit = args.acceleration;
        motor.deceleration_limit = args.acceleration;
        send_request(
            &mut session,
            output_assembly.clone(),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(outputs.clone())),
        )
        .await?;

        // ========= Wait for the move to be acknowledged ============
        loop {
            let reply = send_request(
                &mut session,
                input_assembly.clone(),
                ServiceCode::GetAttributeSingle,
                None,
            )
            .await?;
            let inputs: InputAssemblyObject = decode_reply(&reply)?;
            let status = inputs.motor_input(args.motor).motor_status;
            if status.load_velocity_move_ack() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!("the homing move was never acknowledged: {status:?}").into());
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }

        // The move flags act on their rising edge, so they are cleared while the motor homes
        let motor = outputs.motor_output_mut(args.motor);
        motor.output_register.set_homing_move(false);
        motor.output_register.set_load_velocity_move(false);
        send_request(
            &mut session,
            output_assembly.clone(),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(outputs.clone())),
        )
        .await?;

        // ========= Wait for the motor to home ============
        loop {
            let reply = send_request(
                &mut session,
                input_assembly.clone(),
                ServiceCode::GetAttributeSingle,
                None,
            )
            .await?;
            let inputs: InputAssemblyObject = decode_reply(&reply)?;
            let status = inputs.motor_input(args.motor).motor_status;
            if status.has_homed() {
                println!("HOMED");
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!("the motor did not home in time: {status:?}").into());
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };
    let outcome = tokio::select! {
        result = homing => result,
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C: stopping");
            Ok(())
        }
    };
    if let Err(error) = &outcome {
        eprintln!("homing failed: {error}");
    }

    // ========= Disable the motor (always) ============
    println!("REQUESTING - SET disable motor {}", args.motor);
    let motor = outputs.motor_output_mut(args.motor);
    motor.output_register.set_enable(false);
    motor.output_register.set_homing_move(false);
    motor.output_register.set_load_velocity_move(false);
    send_request(
        &mut session,
        output_assembly,
        ServiceCode::SetAttributeSingle,
        Some(Box::new(outputs)),
    )
    .await?;

    // ========= UnRegister the session ============
    println!("REQUESTING - UN REGISTER session");
    session.unregister().await?;

    outcome
}
