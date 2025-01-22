use tokio::net::TcpStream;

use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::packet::EnIpPacketDescription;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

// Assert dependency on the different modules in this directory
mod clearlink_config;
mod clearlink_output;
mod stream_utils;

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

    // ========= Request the digital output ============
    println!("REQUESTING digital output");

    // TODO: Create the request for the SetDigitalIO message in the teknic_cip
    let set_digital_output_message = MessageRouterRequest::new(
        ServiceCode::SetAttributeSingle,
        CipPath::new_full(0x4, 0x70, 0x3),
    );

    let set_digital_output_object = RequestObjectAssembly {
        packet_description: EnIpPacketDescription::new_cip_description(provided_session_handle, 0),
        cip_message: Some(set_digital_output_message),
    };

    stream_utils::write_object_assembly(&mut stream, set_digital_output_object).await;

    // TODO: Create the response for the SetDigitalIO message in the teknic_cip
    let set_digital_io_response_object =
        stream_utils::read_object_assembly::<u8>(&mut stream).await?;

    // println!("{:#?}\n", set_digital_io_response_object);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", set_digital_io_response_object);
    // ^^^^^^^^^ Request the digital output ^^^^^^^^^^^^

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
