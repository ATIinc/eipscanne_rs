use clap::Parser;
use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

// Assert dependency on the different modules in this directory
mod cli_config;
mod duplicated_stream_utils;
mod nitra;

// Make sure the code itself looks the same
use cli_config::CliArgs;
use duplicated_stream_utils as stream_utils;
use nitra::{SolenoidValves, StatusByte};

const ETHERNET_IP_PORT: u16 = 0xAF12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_args = CliArgs::parse();

    // Connect to the server at IP address and port
    // let address = format!("172.28.0.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port
    let address = format!("{}:{}", cli_args.address, ETHERNET_IP_PORT);

    let mut stream = TcpStream::connect(address).await?;

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    stream_utils::write_object_assembly(&mut stream, RequestObjectAssembly::new_registration())
        .await;
    let registration_response = stream_utils::read_object_assembly::<u8>(&mut stream).await?;

    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", registration_response);
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response
        .packet_description
        .header
        .session_handle;

    // ========= Request the Nitra Status ============
    println!("REQUESTING - GET Nitra Status");

    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::<u8>::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 101, 3),
            ServiceCode::GetAttributeSingle,
            None,
        ),
    )
    .await;

    let _status_byte = stream_utils::read_object_assembly::<StatusByte>(&mut stream).await?;

    // println!("{:#?}\n", _status_byte);      // NOTE: the :#? triggers a pretty-print
    // println!("{:?}\n", _status_byte);
    // ^^^^^^^^^ Request the digital output ^^^^^^^^^^^^

    // ========= Write the Solenoid Valve Output ============

    // |||||||||||||||||||||||||||||||||
    // |||| Actually set the output ||||
    // |||||||||||||||||||||||||||||||||
    let mut output_valve_data = SolenoidValves::default();
    output_valve_data.set_valve_index(cli_args.index as usize, cli_args.output_value.on);

    println!("REQUESTING - SET Solenoid Valve Output");

    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 100, 3),
            ServiceCode::SetAttributeSingle,
            Some(output_valve_data),
        ),
    )
    .await;

    let _set_valve_success = stream_utils::read_object_assembly::<u8>(&mut stream).await?;

    // ^^^^^^^^^ Write the Solenoid Valve Output ^^^^^^^^^^^^

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
