//! One encapsulation packet on a stream: the header, then as many bytes as its Length says, so
//! a packet split over several reads or sharing one with the next is read whole.

use std::io::Cursor;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::eip::command::CommandSpecificData;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};

use crate::error::Result;

/// Size of the encapsulation header on the wire
const ENCAPSULATION_HEADER_LEN: usize = 24;

/// Reads one encapsulation packet
pub async fn read_packet(stream: &mut (impl AsyncRead + Unpin)) -> Result<EnIpPacket> {
    let (_, bytes) = read_frame(stream).await?;
    Ok(EnIpPacket::read(&mut Cursor::new(&bytes))?)
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

/// The Message Router request carried by `packet` (Send RR Data or Send Unit Data), if it carries
/// one
pub fn request_of(packet: &EnIpPacket) -> Option<&MessageRouterRequest> {
    let cip_message = match &packet.command_specific_data {
        CommandSpecificData::SendRrData(rr_data) => &rr_data.cip_message,
        CommandSpecificData::SendUnitData(unit_data) => &unit_data.cip_message,
        _ => return None,
    };
    match cip_message {
        CipMessage::Request(request) => Some(request),
        CipMessage::Response(_) => None,
    }
}

/// Reads the bytes of one encapsulation packet, header included, and its parsed header. The
/// header alone parses for any command, so a packet whose command is not served can still be
/// answered.
pub(crate) async fn read_frame(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<(EncapsulationHeader, Vec<u8>)> {
    let mut bytes = vec![0u8; ENCAPSULATION_HEADER_LEN];
    stream.read_exact(&mut bytes).await?;

    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))?;
    let length = usize::from(header.length.unwrap_or(0));
    bytes.resize(ENCAPSULATION_HEADER_LEN + length, 0);
    stream
        .read_exact(&mut bytes[ENCAPSULATION_HEADER_LEN..])
        .await?;

    Ok((header, bytes))
}

/// Writes a header without command specific data, the reply that only carries a status
pub(crate) async fn write_header(
    stream: &mut (impl AsyncWrite + Unpin),
    header: &EncapsulationHeader,
) -> Result<()> {
    let mut bytes = Cursor::new(Vec::new());
    header.write_le_args(&mut bytes, (0,))?;
    stream.write_all(bytes.get_ref()).await?;
    Ok(())
}
