//! Homes one motor of a Teknic ClearLink over explicit messaging: clear faults, write the homing
//! configuration, enable the motor, start the homing move and poll the input assembly until the
//! motor reports it has homed. The motor is disabled again however the run ends.
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

// The ClearLink assemblies, shared with the write-teknic-io example; each example uses a
// different part of them
#[allow(dead_code)]
#[path = "../clearlink_assemblies.rs"]
mod clearlink_assemblies;

use clearlink_assemblies::config::{CONFIG_ASSEMBLY_INSTANCE, ConfigAssemblyObject};
use clearlink_assemblies::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyObject, MotorStatus};
use clearlink_assemblies::output::{OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyObject};

type Error = Box<dyn std::error::Error>;

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

    /// How often to read the input assembly while waiting for a status bit, in milliseconds
    #[arg(long, default_value_t = 10)]
    poll_ms: u64,

    /// How long to wait for any one status bit before giving up, in seconds
    #[arg(long, default_value_t = 30)]
    timeout_s: u64,
}

/// The session plus the output assembly as last written, so each step changes one thing and
/// writes the whole assembly back
struct Homing {
    session: Session,
    motor: u8,
    output: OutputAssemblyObject,
    poll: Duration,
    timeout: Duration,
}

impl Homing {
    /// Get_Attribute_Single on the input assembly
    async fn read_input(&mut self) -> Result<InputAssemblyObject, Error> {
        let reply = send_request(
            &mut self.session,
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

    /// Set_Attribute_Single on the output assembly with the outputs as they stand
    async fn write_output(&mut self) -> Result<(), Error> {
        send_request(
            &mut self.session,
            CipPath::new_full(
                ASSEMBLY_CLASS_ID,
                OUTPUT_ASSEMBLY_INSTANCE,
                ASSEMBLY_DATA_ATTRIBUTE_ID,
            ),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(self.output.clone())),
        )
        .await?;
        Ok(())
    }

    /// Set_Attribute_Single on the configuration assembly
    async fn write_config(&mut self, config: ConfigAssemblyObject) -> Result<(), Error> {
        send_request(
            &mut self.session,
            CipPath::new_full(
                ASSEMBLY_CLASS_ID,
                CONFIG_ASSEMBLY_INSTANCE,
                ASSEMBLY_DATA_ATTRIBUTE_ID,
            ),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(config)),
        )
        .await?;
        Ok(())
    }

    /// Changes the outputs of the motor being homed and writes the assembly
    async fn command(
        &mut self,
        change: impl FnOnce(&mut clearlink_assemblies::output::MotorOutputData),
    ) -> Result<(), Error> {
        change(self.output.motor_output_mut(self.motor));
        self.write_output().await
    }

    /// Polls the input assembly until `condition` holds for the motor being homed, or the
    /// timeout passes
    async fn wait_until(
        &mut self,
        what: &str,
        condition: impl Fn(&MotorStatus) -> bool,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + self.timeout;
        loop {
            let input = self.read_input().await?;
            let status = input.motor_input(self.motor).motor_status;
            if condition(&status) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for {what} on motor {}: {status:?}",
                    self.motor
                )
                .into());
            }
            tokio::time::sleep(self.poll).await;
        }
    }

    /// Clears shutdowns and a motor fault if the motor reports any
    async fn clear_faults(&mut self) -> Result<(), Error> {
        let input = self.read_input().await?;
        let status = input.motor_input(self.motor).motor_status;

        if status.shutdowns_present() {
            println!("CLEARING shutdowns");
            self.command(|m| {
                m.output_register.set_enable(false);
                m.output_register.set_clear_alerts(true);
            })
            .await?;
            self.wait_until("shutdowns to clear", |s| !s.shutdowns_present())
                .await?;
            self.command(|m| m.output_register.set_clear_alerts(false))
                .await?;
        }

        if status.motor_in_fault() {
            // Clear Motor Fault is edge triggered: low, high, then low again, each acknowledged
            println!("CLEARING motor fault");
            self.command(|m| m.output_register.set_clear_motor_fault(false))
                .await?;
            self.wait_until("fault clear ack low", |s| !s.clear_motor_fault_ack())
                .await?;
            self.command(|m| m.output_register.set_clear_motor_fault(true))
                .await?;
            self.wait_until("fault clear ack high", |s| s.clear_motor_fault_ack())
                .await?;
            self.command(|m| m.output_register.set_clear_motor_fault(false))
                .await?;
            self.wait_until("fault clear ack low", |s| !s.clear_motor_fault_ack())
                .await?;

            // Then the enable has to go off, on and off again for the fault bit to drop
            self.command(|m| m.output_register.set_enable(false))
                .await?;
            self.wait_until("motor disabled", |s| !s.enabled()).await?;
            self.command(|m| m.output_register.set_enable(true)).await?;
            self.wait_until("motor fault to clear", |s| !s.motor_in_fault())
                .await?;
            self.command(|m| m.output_register.set_enable(false))
                .await?;
            self.wait_until("motor disabled", |s| !s.enabled()).await?;
        }

        Ok(())
    }

    /// The whole sequence; `shutdown` runs afterwards whatever happens here
    async fn run(&mut self, args: &Args) -> Result<(), Error> {
        self.clear_faults().await?;

        // The configuration assembly is written whole, so the other motors and the I/O get the
        // defaults of `ConfigAssemblyObject::default()`
        println!(
            "WRITING config: homing enabled, home sensor connector {}",
            args.home_sensor
        );
        let mut config = ConfigAssemblyObject::default();
        let motor_config = config.motor_config_mut(self.motor);
        motor_config.config_register.set_homing_enable(true);
        motor_config.home_sensor_connector = args.home_sensor;
        self.write_config(config).await?;
        // The device needs a moment to apply the configuration before motion commands
        tokio::time::sleep(Duration::from_millis(200)).await;

        println!("ENABLING motor {}", self.motor);
        self.command(|m| m.output_register.set_enable(true)).await?;
        self.wait_until("motor enabled", |s| s.enabled()).await?;
        self.wait_until("ready to home", |s| s.ready_to_home())
            .await?;

        println!(
            "HOMING at {} steps/s, {} steps/s^2",
            args.velocity, args.acceleration
        );
        let (velocity, acceleration) = (args.velocity, args.acceleration);
        self.command(|m| {
            m.output_register.set_homing_move(true);
            m.output_register.set_load_velocity_move(true);
            m.jog_velocity = velocity;
            m.acceleration_limit = acceleration;
            m.deceleration_limit = acceleration;
        })
        .await?;
        self.wait_until("velocity move ack", |s| s.load_velocity_move_ack())
            .await?;

        // The move flags are edge triggered: clear them while the motor homes
        self.command(|m| {
            m.output_register.set_homing_move(false);
            m.output_register.set_load_velocity_move(false);
        })
        .await?;
        self.wait_until("homed", |s| s.has_homed()).await?;
        println!("HOMED");

        Ok(())
    }

    /// Disables the motor and ends the session
    async fn shutdown(mut self) -> Result<(), Error> {
        println!("DISABLING motor {}", self.motor);
        self.command(|m| m.output_register.set_enable(false))
            .await?;
        self.wait_until("motor disabled", |s| !s.enabled()).await?;

        println!("UNREGISTERING the session");
        self.session.unregister().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // Start from the outputs as the device has them, so nothing else changes when the assembly
    // is written back
    let reply = send_request(
        &mut session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            OUTPUT_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;
    let output: OutputAssemblyObject = decode_reply(&reply)?;

    let mut homing = Homing {
        session,
        motor: args.motor,
        output,
        poll: Duration::from_millis(args.poll_ms),
        timeout: Duration::from_secs(args.timeout_s),
    };

    let outcome = tokio::select! {
        result = homing.run(&args) => result,
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C: stopping");
            Ok(())
        }
    };
    if let Err(error) = &outcome {
        eprintln!("homing failed: {error}");
    }

    // Always leave the motor disabled and the session closed
    homing.shutdown().await?;
    outcome
}
