use tokio::net::TcpStream;

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

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
    let registration_response = stream_utils::read_object_assembly(&mut stream).await?;

    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", registration_response);
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response
        .packet_description
        .header
        .session_handle;

    // ========= Request the identity object ============
    println!("REQUESTING identity");
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_identity(provided_session_handle),
    )
    .await;
    let (identity_response_object, identity_response) =
        stream_utils::read_typed_object_assembly::<IdentityResponse>(&mut stream).await?;

    // println!("{:#?}\n", identity_response_object);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", identity_response_object);

    println!(
        "  --> Product Name: {:?}\n",
        String::from(identity_response.product_name)
    );
    // ^^^^^^^^^ Request the identity object ^^^^^^^^^^^^

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
