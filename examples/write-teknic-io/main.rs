use clap::Parser;
use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

// Assert dependency on the different modules in this directory
mod clearlink_config;
mod clearlink_output;
mod cli_config;
mod duplicated_stream_utils;

// Make sure the code itself looks the same
use clearlink_config::ConfigAssemblyObject;
use clearlink_output::OutputAssemblyObject;
use cli_config::{set_io_data, CliArgs};
use duplicated_stream_utils as stream_utils;

const ETHERNET_IP_PORT: u16 = 0xAF12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    // Connect to the server at IP address and port
    // let address = format!("172.28.0.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port
    let address = format!("172.31.19.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port

    let mut stream = TcpStream::connect(address).await?;

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    stream_utils::write_object_assembly(&mut stream, RequestObjectAssembly::new_registration())
        .await;
    let registration_response = stream_utils::read_object_assembly(&mut stream).await?;

    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", registration_response);
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response
        .packet_description
        .header
        .session_handle;

    // ========= Write the ClearLink Config ============
    println!("REQUESTING - SET config");
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x96, 0x3),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(ConfigAssemblyObject::default())),
        ),
    )
    .await;

    let _config_success_response = stream_utils::read_object_assembly(&mut stream).await?;

    // println!("{:#?}\n", _config_success_response);      // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", _config_success_response);
    // ^^^^^^^^^ Write the ClearLink Config ^^^^^^^^^^^^

    // ========= Request the digital output ============
    println!("REQUESTING - GET digital output");

    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x70, 0x3),
            ServiceCode::GetAttributeSingle,
            None,
        ),
    )
    .await;

    // TODO: Create the response for the SetDigitalIO message in the teknic_cip
    let (_set_digital_io_response_object, mut output_assembly_object) =
        stream_utils::read_typed_object_assembly::<OutputAssemblyObject>(&mut stream).await?;

    // println!("{:#?}\n", _set_digital_io_response_object);      // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", _set_digital_io_response_object);
    // ^^^^^^^^^ Request the digital output ^^^^^^^^^^^^

    // ========= Write the Digital Output ============

    // let mut output_assembly_data = OutputAssemblyObject::test_default();

    // |||||||||||||||||||||||||||||||||
    // |||| Actually set the output ||||
    // |||||||||||||||||||||||||||||||||
    set_io_data(
        &mut output_assembly_object.io_output_data,
        cli_args.index as usize,
        cli_args.output_value,
    );

    println!("REQUESTING - SET digital output");

    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x70, 0x3),
            ServiceCode::SetAttributeSingle,
            Some(Box::new(output_assembly_object)),
        ),
    )
    .await;

    let _set_digital_io_success_response =
        stream_utils::read_object_assembly(&mut stream).await?;

    // ^^^^^^^^^ Write the Digital Output ^^^^^^^^^^^^

    // ========= UnRegister the sesion ============
    println!("REQUESTING - UN REGISTER session");
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(provided_session_handle),
    )
    .await;

    println!("UN Registered the CIP session");
    // ^^^^^^^^^ UnRegister the session ^^^^^^^^^^^^

    Ok(())
}
