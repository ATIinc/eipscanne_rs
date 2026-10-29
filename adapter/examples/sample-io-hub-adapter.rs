//! A sample Teknic IO-HUB-4-E with an Identity object and the hub's output and input assemblies,
//! served on 127.0.0.1 until Ctrl-C. Its four motors enable when told, home about 2 s after a
//! homing move, and turn in short spurts while a class 1 connection runs. It accepts a class 3
//! connection.
//!
//! The scanner binds UDP 2222, so on the same host the adapter takes its outputs on another port
//! and names it in the Forward_Open reply:
//!
//! ```text
//! cargo run --example sample-io-hub-adapter
//! cargo run --example read-identity -- --host 127.0.0.1
//! cargo run --example read-identity-connected -- --host 127.0.0.1
//! cargo run --example io-hub-explicit-homing -- --host 127.0.0.1 --motor 0
//! cargo run --example io-hub-eds-implicit -- --host 127.0.0.1 --motor 0
//! ```

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::object_ids::{
    ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID, IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID,
};
use eipscanne_rs::cip::types::{CipDint, CipInt, CipShortString, CipUsint};
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;

use adapter::implicit::ImplicitDevice;
use adapter::message_router::MessageRouter;
use adapter::packet::decode_data;
use adapter::server::serve_implicit;

// The IO-HUB assemblies the scanner's examples use, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../scanner/assemblies"]
mod assemblies {
    pub mod io_hub;
}

use assemblies::io_hub::input::{
    INPUT_ASSEMBLY_INSTANCE, InputAssemblyHub4E, MotorInputData, MotorStatusword,
};
use assemblies::io_hub::output::{MoveType, OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyHub4E};

const ASSEMBLY_CLASS: u16 = ASSEMBLY_CLASS_ID as u16;
const DATA_ATTRIBUTE: u16 = ASSEMBLY_DATA_ATTRIBUTE_ID as u16;
const OUTPUT_INSTANCE: u16 = OUTPUT_ASSEMBLY_INSTANCE as u16;
const INPUT_INSTANCE: u16 = INPUT_ASSEMBLY_INSTANCE as u16;
/// Data bytes of the output assembly
const OUTPUT_SIZE: u16 = 148;
/// Data bytes of the input assembly
const INPUT_SIZE: u16 = 228;

/// How long a homing move takes
const HOMING_TIME: Duration = Duration::from_secs(2);
/// A spurt starts this often while a class 1 connection runs
const SPURT_PERIOD: Duration = Duration::from_secs(4);
/// How long a spurt lasts
const SPURT_TIME: Duration = Duration::from_secs(1);
/// A spurt moves a motor once per step time
const SPURT_STEP_TIME: Duration = Duration::from_millis(100);
/// Counts a motor moves per step
const SPURT_STEP: CipDint = 100;
/// Velocity reported during a spurt
const SPURT_VELOCITY: CipDint = 1000;

/// Serves a sample IO-HUB-4-E
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address to listen on; 0.0.0.0 also serves other hosts
    #[arg(long, default_value_t = Ipv4Addr::LOCALHOST)]
    host: Ipv4Addr,

    /// UDP port of the I/O; 2222 is taken by a scanner on the same host
    #[arg(long, default_value_t = 2223)]
    io_port: u16,
}

/// One motor port
#[derive(Default)]
struct Motor {
    enabled: bool,
    move_number_ack: CipInt,
    move_type_ack: CipUsint,
    homing_since: Option<Instant>,
    has_homed: bool,
    position: CipDint,
}

/// The last outputs, the four motors and the last spurt step
#[derive(Default)]
struct Hub {
    outputs: OutputAssemblyHub4E,
    motors: [Motor; 4],
    spurt_step: u128,
}

/// An Identity object and the IO-HUB's assemblies; the inputs follow the motors
struct SampleIoHubAdapter {
    io_port: u16,
    started: Instant,
    hub: Mutex<Hub>,
}

impl SampleIoHubAdapter {
    /// Enables and disables the motors and acks each new move
    fn apply_outputs(&self, mut outputs: OutputAssemblyHub4E) {
        let mut hub = self
            .hub
            .lock()
            .expect("nothing panics while holding the hub lock");
        for (index, motor) in (0..).zip(hub.motors.iter_mut()) {
            let output = outputs.motor_output_mut(index);
            let enabled = output.controlword.enable();
            if enabled != motor.enabled {
                tracing::info!(motor = index, enabled, "motor enable changed");
                motor.enabled = enabled;
            }
            if !enabled {
                motor.homing_since = None;
            } else if output.move_number != motor.move_number_ack {
                tracing::info!(
                    motor = index,
                    move_number = output.move_number,
                    move_type = output.move_type,
                    "move commanded"
                );
                motor.move_number_ack = output.move_number;
                motor.move_type_ack = output.move_type;
                if output.move_type == MoveType::HomingMove as CipUsint {
                    motor.homing_since = Some(Instant::now());
                }
            }
        }
        hub.outputs = outputs;
    }

    /// The input assembly, built anew from the motors; `moving` reports the spurt velocity
    fn inputs(hub: &mut Hub, moving: bool) -> InputAssemblyHub4E {
        let [motor0, motor1, motor2, motor3] = &mut hub.motors;
        InputAssemblyHub4E {
            motor0_input_data: Self::motor_input(motor0, moving),
            motor1_input_data: Self::motor_input(motor1, moving),
            motor2_input_data: Self::motor_input(motor2, moving),
            motor3_input_data: Self::motor_input(motor3, moving),
            ..Default::default()
        }
    }

    /// The inputs of one motor; a homing move finishes after `HOMING_TIME`
    fn motor_input(motor: &mut Motor, moving: bool) -> MotorInputData {
        if motor
            .homing_since
            .is_some_and(|since| since.elapsed() >= HOMING_TIME)
        {
            motor.homing_since = None;
            motor.has_homed = true;
        }
        let homing = motor.homing_since.is_some();
        let mut statusword = MotorStatusword::default();
        statusword.set_motor_connected(true);
        statusword.set_enabled(motor.enabled);
        statusword.set_ready_for_command(motor.enabled);
        statusword.set_homing(homing);
        statusword.set_has_homed(motor.has_homed);
        statusword.set_command_complete(motor.enabled && !homing && !moving);
        MotorInputData {
            statusword,
            position_measured: motor.position,
            velocity_measured: if moving { SPURT_VELOCITY } else { 0 },
            position_target: motor.position,
            move_type_ack: motor.move_type_ack,
            move_number_ack: motor.move_number_ack,
            ..Default::default()
        }
    }
}

impl MessageRouter for SampleIoHubAdapter {
    fn handle_get_attributes_all(
        &self,
        class: u16,
        instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        match (class, instance) {
            (IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID) => {
                Ok(CipDataOpt::Typed(Box::new(IdentityResponse {
                    vendor_id: VendorId::TeknicInc,
                    device_type: DeviceType::GenericDevice,
                    product_code: 1,
                    revision: Revision { major: 1, minor: 0 },
                    status: IdentityStatusBits::default().into(),
                    serial_number: 0x0102_0304,
                    product_name: CipShortString::from("Sample IO-HUB".to_string()),
                })))
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn handle_get_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        let mut hub = self
            .hub
            .lock()
            .expect("nothing panics while holding the hub lock");
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => {
                Ok(CipDataOpt::Typed(Box::new(hub.outputs.clone())))
            }
            (ASSEMBLY_CLASS, INPUT_INSTANCE, DATA_ATTRIBUTE) => {
                Ok(CipDataOpt::Typed(Box::new(Self::inputs(&mut hub, false))))
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn handle_set_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => {
                self.apply_outputs(decode_data(data)?);
                Ok(())
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn accepts_explicit_connections(&self) -> bool {
        true
    }
}

impl ImplicitDevice for SampleIoHubAdapter {
    type Outputs = OutputAssemblyHub4E;
    type Inputs = InputAssemblyHub4E;

    fn io_port(&self) -> u16 {
        self.io_port
    }

    fn assembly_layout(&self, instance: u16) -> Option<(u16, RealTimeFormat)> {
        match instance {
            // The formats of the hub's EDS: a run/idle header on the outputs only
            OUTPUT_INSTANCE => Some((OUTPUT_SIZE, RealTimeFormat::Header32Bit)),
            INPUT_INSTANCE => Some((INPUT_SIZE, RealTimeFormat::Modeless)),
            _ => None,
        }
    }

    fn consume_outputs(&self, _instance: u16, outputs: OutputAssemblyHub4E) {
        self.apply_outputs(outputs);
    }

    fn produce_inputs(&self, _instance: u16) -> InputAssemblyHub4E {
        let mut hub = self
            .hub
            .lock()
            .expect("nothing panics while holding the hub lock");
        let elapsed = self.started.elapsed().as_millis();
        let moving = elapsed % SPURT_PERIOD.as_millis() < SPURT_TIME.as_millis();
        let step = elapsed / SPURT_STEP_TIME.as_millis();
        if moving && step != hub.spurt_step {
            hub.spurt_step = step;
            for motor in &mut hub.motors {
                motor.position = motor.position.wrapping_add(SPURT_STEP);
            }
        }
        Self::inputs(&mut hub, moving)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `RUST_LOG` when set, otherwise info
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();

    let listener = TcpListener::bind((args.host, ETHERNET_IP_TCP_PORT)).await?;
    tracing::info!("serving on {}", listener.local_addr()?);
    tracing::info!(
        "try: cargo run --example read-identity -- --host {}",
        args.host
    );
    tracing::info!(
        "try: cargo run --example read-identity-connected -- --host {}",
        args.host
    );
    tracing::info!(
        "try: cargo run --example io-hub-explicit-homing -- --host {} --motor 0",
        args.host
    );
    tracing::info!(
        "try: cargo run --example io-hub-eds-implicit -- --host {} --motor 0",
        args.host
    );
    let adapter = SampleIoHubAdapter {
        io_port: args.io_port,
        started: Instant::now(),
        hub: Mutex::new(Hub::default()),
    };

    tokio::select! {
        served = serve_implicit(Arc::new(adapter), listener) => served?,
        stopped = tokio::signal::ctrl_c() => stopped?,
    }
    tracing::info!("stopped");

    Ok(())
}
