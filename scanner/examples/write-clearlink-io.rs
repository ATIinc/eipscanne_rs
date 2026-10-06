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
use assemblies::clearlink::output::{IOOutputData, OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyObject};

#[derive(Parser)]
struct OutputValue {
    /// Turns the output on
    #[arg(
        long,
        required = true,
        conflicts_with = "off",
        conflicts_with = "pwm_value"
    )]
    on: bool,

    /// Turns the output off
    #[arg(
        long,
        required = true,
        conflicts_with = "on",
        conflicts_with = "pwm_value"
    )]
    off: bool,

    /// Sets the output value to the specified number between 0 and 255 (inclusive)
    #[arg(
        long = "pwm",
        required = true,
        conflicts_with = "on",
        conflicts_with = "off"
    )]
    pwm_value: Option<u8>,
}

/// Sets the value of a digital output on a Teknic ClearLink controller
#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "Used to set the value of a digital output on a Teknic ClearLink controller"
)]
struct CliArgs {
    /// IP address of the ClearLink
    #[arg(long, default_value = "172.31.19.10")]
    host: String,

    /// The digital output to set
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..5))]
    index: u8,

    #[command(flatten)]
    output_value: OutputValue,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    // The two assemblies this example talks to
    let config_assembly = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        CONFIG_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );
    let output_assembly = CipPath::new_full(
        ASSEMBLY_CLASS_ID,
        OUTPUT_ASSEMBLY_INSTANCE,
        ASSEMBLY_DATA_ATTRIBUTE_ID,
    );

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register((cli_args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    // ========= Write the ClearLink Config ============
    println!("REQUESTING - SET config");
    let _config_success_response = send_request(
        &mut session,
        config_assembly,
        ServiceCode::SetAttributeSingle,
        Some(Box::new(ConfigAssemblyObject::default())),
    )
    .await?;

    // println!("{:#?}\n", _config_success_response);      // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", _config_success_response);
    // ^^^^^^^^^ Write the ClearLink Config ^^^^^^^^^^^^

    // ========= Request the digital output ============
    println!("REQUESTING - GET digital output");

    let output_assembly_reply = send_request(
        &mut session,
        output_assembly.clone(),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;

    let mut output_assembly_object: OutputAssemblyObject = decode_reply(&output_assembly_reply)?;
    // ^^^^^^^^^ Request the digital output ^^^^^^^^^^^^

    // ========= Write the Digital Output ============

    // |||||||||||||||||||||||||||||||||
    // |||| Actually set the output ||||
    // |||||||||||||||||||||||||||||||||
    set_io_data(
        &mut output_assembly_object.io_output_data,
        cli_args.index as usize,
        cli_args.output_value,
    );

    println!("REQUESTING - SET digital output");

    let _set_digital_io_success_response = send_request(
        &mut session,
        output_assembly,
        ServiceCode::SetAttributeSingle,
        Some(Box::new(output_assembly_object)),
    )
    .await?;

    // ^^^^^^^^^ Write the Digital Output ^^^^^^^^^^^^

    // ========= UnRegister the session ============
    println!("REQUESTING - UN REGISTER session");
    session.unregister().await?;

    println!("UN Registered the CIP session");
    // ^^^^^^^^^ UnRegister the session ^^^^^^^^^^^^

    Ok(())
}

/// Turns the chosen output on or off, or sets its PWM duty cycle
fn set_io_data(io_output_data: &mut IOOutputData, index: usize, output_value: OutputValue) {
    match output_value {
        // On and Off are mutually exclusive so only one needs to be checked
        OutputValue {
            on: is_on,
            off: _,
            pwm_value: None,
        } => {
            io_output_data.set_digital_output(index, is_on);
        }
        OutputValue {
            on: _,
            off: _,
            pwm_value: Some(pwm),
        } => {
            io_output_data.set_digital_pwm(index, pwm);
        }
    }
}
