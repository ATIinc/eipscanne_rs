//! O->T: the outputs, always as bytes. The I/O task passes every packet to [`accept_o2t_packet`]
//! and keeps the deadline.

use std::io::Cursor;
use std::net::SocketAddr;
use std::time::Duration;

use binrw::BinRead;
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::{TransportClass, connection_size};
use eipscanne_rs::cip::io_data::CipIoData;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::io_packet::{EnIpIoPacket, SequencedAddress};

use crate::error::{Error, Result};
use crate::implicit::connection::{ImplicitConnection, data_len_matches_connection, data_size};

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// The least time the scanner gets for its first packet
pub(crate) const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// The smallest allowed sequence number gap
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Receives one I/O packet and its sender. Cancel safe.
pub(crate) async fn recv_io_packet(socket: &UdpSocket) -> Result<(EnIpIoPacket, SocketAddr)> {
    let mut bytes = vec![0u8; MAX_UDP_PAYLOAD];
    let (len, from): (usize, SocketAddr) = socket.recv_from(&mut bytes).await?;
    bytes.truncate(len);

    let packet = EnIpIoPacket::read(&mut Cursor::new(&bytes))?;
    Ok((packet, from))
}

/// Screens a packet of `connection` from `from`, given the last accepted encapsulation sequence
/// number (`None` before the first). Returns its Sequenced Address and Connected Data Item, or
/// `Error::UnexpectedPacket` saying why it was discarded.
pub(crate) fn accept_o2t_packet(
    connection: &ImplicitConnection,
    last_sequence_number: Option<CipUdint>,
    packet: &EnIpIoPacket,
    from: SocketAddr,
) -> Result<(SequencedAddress, CipIoData)> {
    // 1. It is for this connection
    let address: SequencedAddress = packet.sequenced_address;
    if address.connection_id != connection.response.o2t_network_connection_id {
        return Err(Error::UnexpectedPacket(format!(
            "packet for another connection ({:#010x})",
            address.connection_id
        )));
    }

    // 2. It comes from the scanner (whatever port it chose)
    if from.ip() != connection.originator_ip {
        return Err(Error::UnexpectedPacket(format!(
            "packet from {from}, not the scanner"
        )));
    }

    // 3. It is newer than the last accepted one (distance modulo 2^32 in 1..2^31), by at most
    //    the timeout multiplier plus one (never under 16). Any first number is accepted
    if let Some(last) = last_sequence_number {
        let received: CipUdint = address.encapsulation_sequence_number;
        let distance: u32 = received.wrapping_sub(last);
        if distance == 0 || distance >= 1 << 31 {
            return Err(Error::UnexpectedPacket(format!(
                "stale sequence number {received} (last accepted {last})"
            )));
        }
        // The timeout multiplier as a factor: 4, 8, ... 512
        let multiplier: u32 = connection
            .request
            .connection_timeout_multiplier
            .multiplier();
        let allowed: u32 = MIN_ALLOWED_SEQUENCE_GAP.max(multiplier.saturating_add(1));
        if distance > allowed {
            return Err(Error::UnexpectedPacket(format!(
                "sequence number {received} is more than {allowed} ahead of the last accepted {last}"
            )));
        }
    }

    // 4. Its data has the agreed size and decodes in this direction's shape
    let CipDataOpt::Raw(bytes) = &packet.connected_data else {
        return Err(Error::UnexpectedPacket(
            "packet whose Connected Data Item is not raw".to_string(),
        ));
    };
    let real_time_format = connection.o2t_real_time_format;
    let (expected, size_type) = data_size(
        &connection.request.o2t_network_connection_parameters,
        real_time_format,
    );
    // The sequence count and header precede the data
    let overhead = usize::from(connection_size(0, TransportClass::Class1, real_time_format));
    let data_len = bytes.len().saturating_sub(overhead);
    if bytes.len() < overhead || !data_len_matches_connection(data_len, expected, size_type) {
        return Err(Error::UnexpectedPacket(format!(
            "{data_len} output bytes, the connection carries {expected}"
        )));
    }
    // A Connected Data Item's length is a 16-bit field on the wire
    let outputs = CipIoData::read_le_args(
        &mut Cursor::new(bytes),
        (bytes.len() as u16, TransportClass::Class1, real_time_format),
    )?;

    Ok((address, outputs))
}
