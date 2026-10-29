//! Bytes on the wire: encapsulation packets on a stream, read whole however the bytes arrive, and
//! the data a request or an I/O packet carries.

use std::io::Cursor;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::{Error, Result};

/// Size of the encapsulation header on the wire
const ENCAPSULATION_HEADER_LEN: usize = 24;

/// Reads one encapsulation packet whole; [`Error::Undecodable`] when only its header decodes
pub async fn read_packet(stream: &mut (impl AsyncRead + Unpin)) -> Result<EnIpPacket> {
    let mut bytes = vec![0u8; ENCAPSULATION_HEADER_LEN];
    stream.read_exact(&mut bytes).await?;

    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))?;
    let length = usize::from(header.length.unwrap_or(0));
    bytes.resize(ENCAPSULATION_HEADER_LEN + length, 0);
    stream
        .read_exact(&mut bytes[ENCAPSULATION_HEADER_LEN..])
        .await?;

    EnIpPacket::read(&mut Cursor::new(&bytes)).map_err(|error| Error::Undecodable { header, error })
}

/// Writes one encapsulation packet
pub async fn write_packet(
    stream: &mut (impl AsyncWrite + Unpin),
    packet: &EnIpPacket,
) -> Result<()> {
    let mut bytes = Cursor::new(Vec::new());
    packet.write(&mut bytes)?;
    stream.write_all(bytes.get_ref()).await?;
    Ok(())
}

/// The data of a Set, or a connection's outputs, as `T`, or the General Status refusing it; mirrors
/// the scanner's `decode_reply`
pub fn decode_data<T>(data: &[u8]) -> Result<T, ResponseStatusCode>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut reader = Cursor::new(data);
    let value = T::read_le(&mut reader).map_err(|error| {
        if error.is_eof() {
            ResponseStatusCode::NotEnoughData
        } else {
            ResponseStatusCode::InvalidAttributeValue
        }
    })?;
    if reader.position() < data.len() as u64 {
        return Err(ResponseStatusCode::TooMuchData);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_data_refuses_short_and_long_data() {
        assert_eq!(
            decode_data::<u32>(&[0xef, 0xbe]),
            Err(ResponseStatusCode::NotEnoughData)
        );
        assert_eq!(
            decode_data::<u32>(&[0xef, 0xbe, 0xad, 0xde, 0x00]),
            Err(ResponseStatusCode::TooMuchData)
        );
    }
}
