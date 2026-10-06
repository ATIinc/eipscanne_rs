//! Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E over explicit messaging: clear a
//! shutdown if one is present, enable the motor, command the homing move and poll the input
//! assembly until the motor reports it has homed. The motor is disabled again however the run
//! ends.
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

// The IO-HUB assemblies, shared with other examples that drive the hub
#[allow(dead_code)]
#[path = "../io_hub_assemblies.rs"]
mod io_hub_assemblies;

use io_hub_assemblies::input::{INPUT_ASSEMBLY_INSTANCE, InputAssemblyHub4E, MotorInputData};
use io_hub_assemblies::output::{
    MotorOutputData, MoveType, OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyHub4E,
};

type Error = Box<dyn std::error::Error>;

/// How long the shutdown-reset handshake may take in each direction
const SHUTDOWN_RESET_TIMEOUT: Duration = Duration::from_secs(1);

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

    /// How often to print the motor status while waiting for homing to finish, in milliseconds
    #[arg(long, default_value_t = 500)]
    poll_ms: u64,

    /// How long to wait for homing to finish after the last command, in seconds
    #[arg(long, default_value_t = 10)]
    timeout_s: u64,
}

/// The session plus the output assembly as last written
struct Hub {
    session: Session,
    motor: u8,
    output: OutputAssemblyHub4E,
}

impl Hub {
    /// Get_Attribute_Single on the input assembly
    async fn read_input(&mut self) -> Result<InputAssemblyHub4E, Error> {
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

    /// Changes the outputs of the motor being homed and writes the whole assembly
    async fn command(&mut self, change: impl FnOnce(&mut MotorOutputData)) -> Result<(), Error> {
        change(self.output.motor_output_mut(self.motor));
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

    /// Polls the input assembly until `condition` holds for the motor, or `timeout` passes
    async fn wait_until(
        &mut self,
        what: &str,
        timeout: Duration,
        condition: impl Fn(&MotorInputData) -> bool,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + timeout;
        loop {
            let input = self.read_input().await?;
            if condition(input.motor_input(self.motor)) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!("timed out waiting for {what} on motor {}", self.motor).into());
            }
        }
    }

    /// Shutdown Reset is a handshake: raise the bit until the hub acknowledges it, then lower
    /// it until the acknowledgement drops, so the next reset gets a clean rising edge
    async fn clear_shutdown_if_present(&mut self) -> Result<(), Error> {
        let input = self.read_input().await?;
        if !input
            .motor_input(self.motor)
            .statusword
            .motor_shutdown_present()
        {
            return Ok(());
        }

        println!("CLEARING shutdown");
        self.command(|m| m.controlword.set_shutdown_reset(true))
            .await?;
        let acknowledged = self
            .wait_until("shutdown reset ack", SHUTDOWN_RESET_TIMEOUT, |m| {
                m.statusword.shutdown_reset_ack()
            })
            .await;

        self.command(|m| m.controlword.set_shutdown_reset(false))
            .await?;
        self.wait_until("shutdown reset ack to drop", SHUTDOWN_RESET_TIMEOUT, |m| {
            !m.statusword.shutdown_reset_ack()
        })
        .await?;

        acknowledged
    }

    /// The homing commands, then the wait for the result; `shutdown` runs afterwards whatever
    /// happens here
    async fn run(&mut self, args: &Args) -> Result<(), Error> {
        self.clear_shutdown_if_present().await?;

        println!("ENABLING motor {}", self.motor);
        self.command(|m| m.controlword.set_enable(true)).await?;

        // The hub acts on a move when the move number changes, so each command gets the next one
        let input = self.read_input().await?;
        let mut move_number = input.motor_input(self.motor).move_number_ack;
        for i in 1..=args.repeat {
            move_number = move_number.wrapping_add(1);
            self.command(|m| m.command_move(MoveType::HomingMove, move_number))
                .await?;
            println!(
                "HOMING command {i}/{} sent as move {move_number}",
                args.repeat
            );
            tokio::time::sleep(Duration::from_millis(args.delay_ms)).await;
        }

        let deadline = Instant::now() + Duration::from_secs(args.timeout_s);
        loop {
            let input = self.read_input().await?;
            let motor = input.motor_input(self.motor);
            print_status(motor);
            if motor.statusword.has_homed() {
                println!("HOMED");
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "homing did not finish within {} s of the last command",
                    args.timeout_s
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(args.poll_ms)).await;
        }
    }

    /// Disables the motor and ends the session
    async fn shutdown(mut self) -> Result<(), Error> {
        println!("DISABLING motor {}", self.motor);
        self.command(|m| m.controlword.set_enable(false)).await?;

        println!("UNREGISTERING the session");
        self.session.unregister().await?;
        Ok(())
    }
}

/// One line of the status bits that tell how homing is going
fn print_status(motor: &MotorInputData) {
    let status = motor.statusword;
    println!(
        "  pos={} vel={} | homing={} has_homed={} in_home_sensor={} | enabled={} ready={} \
         complete={} shutdown={} warning={} cancelled={} | move {} ack {}{}",
        motor.position_measured,
        motor.velocity_measured,
        u8::from(status.homing()),
        u8::from(status.has_homed()),
        u8::from(status.in_home_sensor()),
        u8::from(status.enabled()),
        u8::from(status.ready_for_command()),
        u8::from(status.command_complete()),
        u8::from(status.motor_shutdown_present()),
        u8::from(status.motor_warning_present()),
        u8::from(status.move_cancelled()),
        motor.move_number_ack,
        motor.move_type_ack,
        match motor.rejection() {
            Some(code) => format!(" ({code:?})"),
            None => String::new(),
        }
    );
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    println!("REGISTERING a session with {}", args.host);
    let session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    let mut hub = Hub {
        session,
        motor: args.motor,
        output: OutputAssemblyHub4E::default(),
    };

    let outcome = tokio::select! {
        result = hub.run(&args) => result,
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C: stopping");
            Ok(())
        }
    };
    if let Err(error) = &outcome {
        eprintln!("homing failed: {error}");
    }

    // Always leave the motor disabled and the session closed
    hub.shutdown().await?;
    outcome
}
