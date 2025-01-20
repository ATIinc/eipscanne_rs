use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use binrw::{BinRead, BinWrite};

use std::io::BufReader;

extern crate eipscanne_rs;

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

async fn write_object_assembly<T>(stream: &mut TcpStream, object_assembly: RequestObjectAssembly<T>)
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    object_assembly.write(&mut writer).unwrap();

    let _ = stream.write_all(&byte_array_buffer).await;
}

async fn read_object_assembly<T>(
    stream: &mut TcpStream,
) -> Result<ResponseObjectAssembly<T>, binrw::Error>
where
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
{
    // Write the object_assembly binary data to the buffer
    let mut response_buffer = vec![0; 100];
    let response_bytes_read = stream.read(&mut response_buffer).await?;
    response_buffer.truncate(response_bytes_read);

    println!("  RESPONSE: {} bytes", response_bytes_read);

    let response_byte_cursor = std::io::Cursor::new(response_buffer);
    let mut response_reader = BufReader::new(response_byte_cursor);

    ResponseObjectAssembly::<T>::read(&mut response_reader)
}

const ETHERNET_IP_PORT: u16 = 0xAF12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server at IP address and port
    // let address = format!("172.28.0.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port
    let address = format!("172.31.19.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port

    let mut stream = TcpStream::connect(address).await?;

    // ========= Register the session ============
    println!("REQUESTING registration");
    write_object_assembly(&mut stream, RequestObjectAssembly::new_registration()).await;
    let registration_response = read_object_assembly::<u8>(&mut stream).await?;

    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", registration_response);
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response
        .packet_description
        .header
        .session_handle;

    // ========= Request the identity object ============
    println!("REQUESTING identity");
    write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_identity(provided_session_handle),
    )
    .await;
    let identity_response_object = read_object_assembly::<IdentityResponse>(&mut stream).await?;

    // println!("{:#?}\n", identity_response_object);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", identity_response_object);

    let message_router_response = identity_response_object.cip_message.unwrap();
    println!(
        "  --> Product Name: {:?}\n",
        String::from(message_router_response.router_data.data.product_name)
    );
    // ^^^^^^^^^ Request the identity object ^^^^^^^^^^^^

    // ========= UnRegister the sesion ============
    println!("REQUESTING un-registration");
    write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(provided_session_handle),
    )
    .await;

    println!("UN Registered the CIP session");
    // ^^^^^^^^^ UnRegister the session ^^^^^^^^^^^^

    Ok(())
}
