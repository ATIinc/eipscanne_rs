use clap::Parser;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{decode_reply, send_request};
use scanner::session::Session;

// The ClearLink assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../assemblies"]
mod assemblies {
    pub mod clearlink;
}
mod cli_config;

use assemblies::clearlink::config::{CONFIG_ASSEMBLY_INSTANCE, ConfigAssemblyObject};
use assemblies::clearlink::output::{OUTPUT_ASSEMBLY_INSTANCE, OutputAssemblyObject};
use cli_config::{CliArgs, set_io_data};

/// The ClearLink to talk to unless one is given on the command line
const DEFAULT_ADAPTER_IP: &str = "172.31.19.10";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();
    let adapter_ip = cli_args
        .host
        .clone()
        .unwrap_or_else(|| DEFAULT_ADAPTER_IP.to_string());

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register((adapter_ip.as_str(), ETHERNET_IP_TCP_PORT)).await?;
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    // ========= Write the ClearLink Config ============
    println!("REQUESTING - SET config");
    let _config_success_response = send_request(
        &mut session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            CONFIG_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
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
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            OUTPUT_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
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
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            OUTPUT_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
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
