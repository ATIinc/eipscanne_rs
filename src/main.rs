use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use binrw::{
    // BinRead,  // trait for reading
    BinWrite, // trait for writing
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

#[allow(dead_code)]
fn deserialize_packet_from<T: std::io::Read>(
    _reader: BufReader<T>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(Vec::new())
}

// #[tokio::main]
// async fn main() -> io::Result<()> {
//     // Connect to the server at IP address and port
//     let address = "127.0.0.1:8080"; // Change this to the correct IP and port
//     let mut stream = TcpStream::connect(address).await?;

//     // Create a message to send
//     let registration_request_bytes = get_registration_object_bytes().unwrap();
//     stream.write_all(&registration_request_bytes).await?;

//     // Wait for a response
//     let mut reader = BufReader::new(&mut stream);
//     let mut response = Vec::new();
//     reader.read_to_end(&mut response).await?;

//     let mut byte_reader = BufReader::new(response.as_slice());

//     let registration_response = deserialize_packet_from(&byte_reader).unwrap();

//     Ok(())
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    let (mut socket, _) = listener.accept().await?;

    tokio::spawn(async move {
        let write_buf = get_registration_object_bytes().unwrap();

        // Write the data back
        if let Err(e) = socket.write_all(&write_buf).await {
            eprintln!("failed to write to socket; err = {:?}", e);
            return;
        }

        let mut read_buf = vec![0; 1024];

        let _n = match socket.read(&mut read_buf).await {
            // socket closed
            Ok(n) if n == 0 => return,
            Ok(n) => n,
            Err(e) => {
                eprintln!("failed to read from socket; err = {:?}", e);
                return;
            }
        };

        let byte_cursor = std::io::Cursor::new(read_buf);
        let buf_reader = BufReader::new(byte_cursor);

        let _identity_response = deserialize_packet_from(buf_reader);
    });

    Ok(())
}
