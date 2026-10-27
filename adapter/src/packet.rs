//! Encapsulation packets on a stream, read whole however the bytes arrive.

use std::io::Cursor;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use eipscanne_rs::cip::message::CipMessage;
use eipscanne_rs::cip::message::request::MessageRouterRequest;
use eipscanne_rs::eip::command::CommandSpecificData;
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

/// The Message Router request a Send RR Data or Send Unit Data packet carries
pub fn message_router_request(packet: &EnIpPacket) -> Option<&MessageRouterRequest> {
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
