//! A sample Teknic ClearLink with an Identity object and the configuration, output and input
//! assemblies, served on 127.0.0.1 until Ctrl-C. Its inputs report an IO-n as driven while output n
//! is on or has a PWM duty cycle, never overloaded.
//!
//! Start the adapter, then talk to it with the scanner's examples:
//!
//! ```text
//! cargo run --example sample-clearlink-adapter
//! cargo run --example read-identity -- --host 127.0.0.1
//! cargo run --example clearlink-explicit-outputs -- --host 127.0.0.1 --index 4 --on
//! ```
//!
//! The adapter serves unconnected messaging only, so `read-identity-connected` is refused.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::object_ids::{
    ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID, IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID,
};
use eipscanne_rs::cip::types::CipShortString;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;

use adapter::device::{Device, decode_request};
use adapter::server::serve;

// The ClearLink assemblies the scanner's examples use, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../scanner/assemblies"]
mod assemblies {
    pub mod clearlink;
}

use assemblies::clearlink::config::CONFIG_ASSEMBLY_INSTANCE;
use assemblies::clearlink::input::{DigitalInputs, INPUT_ASSEMBLY_INSTANCE, InputAssemblyObject};
use assemblies::clearlink::output::{
    DigitalOutputs, IOOutputData, MotorOutputData, OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyObject,
    SerialAsciiOutputData,
};

const ASSEMBLY_CLASS: u16 = ASSEMBLY_CLASS_ID as u16;
const DATA_ATTRIBUTE: u16 = ASSEMBLY_DATA_ATTRIBUTE_ID as u16;
const CONFIG_INSTANCE: u16 = CONFIG_ASSEMBLY_INSTANCE as u16;
const OUTPUT_INSTANCE: u16 = OUTPUT_ASSEMBLY_INSTANCE as u16;
const INPUT_INSTANCE: u16 = INPUT_ASSEMBLY_INSTANCE as u16;

/// Serves a sample ClearLink
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address to listen on; 0.0.0.0 also serves other hosts
    #[arg(long, default_value_t = Ipv4Addr::LOCALHOST)]
    host: Ipv4Addr,
}

/// An Identity object and the ClearLink's output assembly; the inputs follow the outputs
struct SampleClearlinkAdapter {
    outputs: Mutex<OutputAssemblyObject>,
}

impl SampleClearlinkAdapter {
    /// The input assembly: DIP Status set for every output that is on or has a PWM duty cycle
    fn inputs(&self) -> InputAssemblyObject {
        let outputs = self
            .outputs
            .lock()
            .expect("nothing panics while holding the outputs lock");
        let dop_value = outputs.io_output_data.dop_value;
        let dop_pwm: [u8; 6] = outputs.io_output_data.dop_pwm;

        let mut inputs = InputAssemblyObject::default();
        inputs.io_input_data.dip_status = DigitalInputs::builder()
            .input0(dop_value.output0() || dop_pwm[0] != 0)
            .input1(dop_value.output1() || dop_pwm[1] != 0)
            .input2(dop_value.output2() || dop_pwm[2] != 0)
            .input3(dop_value.output3() || dop_pwm[3] != 0)
            .input4(dop_value.output4() || dop_pwm[4] != 0)
            .input5(dop_value.output5() || dop_pwm[5] != 0)
            .input6(false)
            .input7(false)
            .input8(false)
            .input9(false)
            .input10(false)
            .input11(false)
            .input12(false)
            .build();
        inputs
    }
}

impl Device for SampleClearlinkAdapter {
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
                    revision: Revision { major: 1, minor: 2 },
                    status: IdentityStatusBits::default().into(),
                    serial_number: 0x0102_0304,
                    product_name: CipShortString::from("Sample Clearlink".to_string()),
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
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => Ok(CipDataOpt::Typed(Box::new(
                self.outputs
                    .lock()
                    .expect("nothing panics while holding the outputs lock")
                    .clone(),
            ))),
            (ASSEMBLY_CLASS, INPUT_INSTANCE, DATA_ATTRIBUTE) => {
                Ok(CipDataOpt::Typed(Box::new(self.inputs())))
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
            // The configuration changes nothing here
            (ASSEMBLY_CLASS, CONFIG_INSTANCE, DATA_ATTRIBUTE) => Ok(()),
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => {
                let outputs: OutputAssemblyObject = decode_request(data)?;
                tracing::info!(
                    dop_value = ?outputs.io_output_data.dop_value,
                    dop_pwm = ?outputs.io_output_data.dop_pwm,
                    "outputs set"
                );
                *self
                    .outputs
                    .lock()
                    .expect("nothing panics while holding the outputs lock") = outputs;
                Ok(())
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
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
        "try: cargo run --example clearlink-explicit-outputs -- --host {} --index 4 --on",
        args.host
    );
    let adapter = SampleClearlinkAdapter {
        outputs: Mutex::new(OutputAssemblyObject {
            io_output_data: IOOutputData::new_digital_outputs(DigitalOutputs::default()),
            motor0_output_data: MotorOutputData::new(),
            motor1_output_data: MotorOutputData::new(),
            motor2_output_data: MotorOutputData::new(),
            motor3_output_data: MotorOutputData::new(),
            serial_ascii_output_data: SerialAsciiOutputData::new(),
        }),
    };

    tokio::select! {
        served = serve(listener, Arc::new(adapter)) => served?,
        stopped = tokio::signal::ctrl_c() => stopped?,
    }
    tracing::info!("stopped");

    Ok(())
}
