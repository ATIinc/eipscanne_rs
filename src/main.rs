use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::cip::message::{MessageRouter, RouterData};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use binrw::{
    // BinRead,  // trait for reading
    BinRead, BinWrite // trait for writing
};

use std::io::BufReader;

use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::object_assembly::ObjectAssembly;

use std::error::Error;

fn get_registration_object_bytes() -> Result<Vec<u8>, Box<dyn Error>> {
    // create an empty packet
    let registration_eip_packet_description = ObjectAssembly::new_registration();

    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    registration_eip_packet_description
        .write(&mut writer)
        .unwrap();

    Ok(byte_array_buffer.clone())
}

#[allow(dead_code)]
fn get_identity_object_bytes(session_handle: CipUdint) -> Result<Vec<u8>, Box<dyn Error>> {
    let identity_object = ObjectAssembly::new_identity(session_handle);

    // Write the identity_object data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    identity_object.write(&mut writer).unwrap();

    Ok(byte_array_buffer.clone())
}

const ETHERNET_IP_PORT: u16 = 0xAF12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Connect to the server at IP address and port
    let address = format!("172.28.0.10:{}", ETHERNET_IP_PORT); // Change this to the correct IP and port
    let mut stream = TcpStream::connect(address).await?;

    
    // ========= Register the session ============
    let registration_request_bytes = get_registration_object_bytes().unwrap();
    stream.write_all(&registration_request_bytes).await?;

    // Wait for a response
    let mut registration_response_buffer = vec![0; 100];
    let registration_bytes_read = stream.read(&mut registration_response_buffer).await?;

    registration_response_buffer.truncate(registration_bytes_read);

    let registration_response_byte_cursor = std::io::Cursor::new(registration_response_buffer);
    let mut registration_response_reader = BufReader::new(registration_response_byte_cursor);

    // there is no full object assembly for an identity response
    let registration_response = ObjectAssembly::<u8>::read(&mut registration_response_reader).unwrap();

    println!("REGISTRATION RESPONSE: {} bytes", registration_bytes_read);
    // println!("{:#?}\n", registration_response);     // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", registration_response); 
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    let provided_session_handle = registration_response.packet_description.header.session_handle;


    // ========= Request the identity object ============
    let identity_request_bytes = get_identity_object_bytes(provided_session_handle).unwrap();
    stream.write_all(&identity_request_bytes).await?;
 
    // Wait for a response
    let mut identity_response_buffer = vec![0; 100];
    let identity_bytes_read = stream.read(&mut identity_response_buffer).await?;

    identity_response_buffer.truncate(identity_bytes_read);

    let identity_response_byte_cursor = std::io::Cursor::new(identity_response_buffer);
    let mut identity_response_reader = BufReader::new(identity_response_byte_cursor);

    let identity_response = ObjectAssembly::<IdentityResponse>::read(&mut identity_response_reader).unwrap();
 
    println!("IDENTITY RESPONSE: {} bytes", identity_bytes_read);
    // println!("{:#?}\n", identity_response);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", identity_response);

    let message_router_response: MessageRouter<IdentityResponse> = identity_response.cip_message.unwrap();
    let identity_response = match message_router_response.router_data {
        RouterData::Request(_request) => Err(()),
        RouterData::Response(response) => Ok(response)
    }.unwrap();

    println!("Product Name: {:?}", String::from(identity_response.data.product_name));
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    Ok(())
}