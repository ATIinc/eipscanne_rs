// NOTE: DUPLICATE OF examples/utils

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use binrw::{BinRead, BinWrite};

use std::io::BufReader;

use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

pub async fn write_object_assembly<T>(
    stream: &mut TcpStream,
    object_assembly: RequestObjectAssembly<T>,
) where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    // Write the object_assembly binary data to the buffer
    let mut byte_array_buffer: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut byte_array_buffer);

    object_assembly.write(&mut writer).unwrap();

    let _ = stream.write_all(&byte_array_buffer).await;
}

pub async fn read_object_assembly<T>(
    stream: &mut TcpStream,
) -> Result<ResponseObjectAssembly<T>, binrw::Error>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    // Write the object_assembly binary data to the buffer
    let mut response_buffer = vec![0; 1000];
    let response_bytes_read = stream.read(&mut response_buffer).await?;
    response_buffer.truncate(response_bytes_read);

    println!("  RESPONSE: {} bytes", response_bytes_read);

    let response_byte_cursor = std::io::Cursor::new(response_buffer);
    let mut response_reader = BufReader::new(response_byte_cursor);

    ResponseObjectAssembly::<T>::read(&mut response_reader)
}
