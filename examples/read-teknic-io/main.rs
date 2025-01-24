use clearlink_config::ConfigAssemblyObject;
use clearlink_output::OutputAssemblyObject;
use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

// Assert dependency on the different modules in this directory
mod clearlink_config;
mod clearlink_output;
mod duplicated_stream_utils;

// Make sure the code itself looks the same
use duplicated_stream_utils as stream_utils;

const ETHERNET_IP_PORT: u16 = 0xAF12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server at IP address and port
    // let address = format!("172.28.0.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port
    let address = format!("172.31.19.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port

    let mut stream = TcpStream::connect(address).await?;

    // ========= Register the session ============
    println!("REQUESTING registration");
    stream_utils::write_object_assembly(&mut stream, RequestObjectAssembly::new_registration())
        .await;
    let registration_response = stream_utils::read_object_assembly::<u8>(&mut stream).await?;

    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", registration_response);
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response
        .packet_description
        .header
        .session_handle;

    // ========= Write the ClearLink Config ============
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_single_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x96, 0x3),
            Some(ConfigAssemblyObject::default()),
        ),
    )
    .await;
    // ^^^^^^^^^ Write the ClearLink Config ^^^^^^^^^^^^

    // ========= Request the digital output ============
    println!("REQUESTING digital output");

    // TODO: Create the request for the SetDigitalIO message in the teknic_cip
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::<u8>::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x70, 0x3),
            ServiceCode::GetAttributeSingle,
            None,
        ),
    )
    .await;

    // TODO: Create the response for the SetDigitalIO message in the teknic_cip
    let set_digital_io_response_object =
        stream_utils::read_object_assembly::<OutputAssemblyObject>(&mut stream).await?;

    // println!("{:#?}\n", set_digital_io_response_object);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", set_digital_io_response_object);
    // ^^^^^^^^^ Request the digital output ^^^^^^^^^^^^

    // ========= Write the Digital Output ============
    let mut output_assembly_data = set_digital_io_response_object
        .cip_message
        .unwrap()
        .response_data
        .data
        .unwrap();

    output_assembly_data
        .io_output_data
        .dop_value
        .set_output1(true);

    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            provided_session_handle,
            CipPath::new_full(0x4, 0x70, 0x3),
            ServiceCode::SetAttributeSingle,
            Some(output_assembly_data),
        ),
    )
    .await;

    // ^^^^^^^^^ Write the Digital Output ^^^^^^^^^^^^

    // ========= UnRegister the sesion ============
    println!("REQUESTING un-registration");
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(provided_session_handle),
    )
    .await;

    println!("UN Registered the CIP session");
    // ^^^^^^^^^ UnRegister the session ^^^^^^^^^^^^

    Ok(())
}
