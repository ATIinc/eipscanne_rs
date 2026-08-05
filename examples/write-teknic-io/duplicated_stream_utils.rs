use eipscanne_rs::cip::message::data::{CipData, CipDataOpt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use binrw::{BinRead, BinWrite};

use std::io::BufReader;

use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

pub async fn write_object_assembly(stream: &mut TcpStream, object_assembly: RequestObjectAssembly) {
    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    object_assembly.write(&mut writer).unwrap();

    let _ = stream.write_all(&byte_array_buffer).await;
}

pub async fn read_object_assembly(
    stream: &mut TcpStream,
) -> Result<ResponseObjectAssembly, binrw::Error>
{
    // Write the object_assembly binary data to the buffer
    let mut response_buffer = vec![0; 500];
    let response_bytes_read = stream.read(&mut response_buffer).await?;
    response_buffer.truncate(response_bytes_read);

    println!("  RESPONSE: {} bytes", response_bytes_read);

    let response_byte_cursor = std::io::Cursor::new(response_buffer);
    let mut response_reader = BufReader::new(response_byte_cursor);

    ResponseObjectAssembly::read(&mut response_reader)
}

pub async fn read_typed_object_assembly<T>(
    stream: &mut TcpStream,
) -> Result<(ResponseObjectAssembly, T), binrw::Error>
where
    T: for<'a> BinRead<Args<'a> = ()> + CipData + 'static
{
    let raw_assembly_response = read_object_assembly(stream).await?;

    // Make sure there is actually a response
    if let Some(ref router_response) = raw_assembly_response.cip_message {
        // Confirm that the read data is a Raw type (Vec<u8>)
        if let CipDataOpt::Raw(ref raw_data) = router_response.response_data.data {
            // Deserialize the raw data into the expected type
            let byte_cursor = std::io::Cursor::new(raw_data);
            let mut buf_reader = std::io::BufReader::new(byte_cursor);

            // Read the typed object from the buffer
            let typed_object = T::read_le(&mut buf_reader)?;

            return Ok((raw_assembly_response, typed_object));

        }
    }

    Err(binrw::Error::Custom {
        err: Box::new("No valid response data found".to_string()),
        pos: 0,
    })
}

#[allow(dead_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
