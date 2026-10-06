//! The UDP socket of stage 3: I/O packets go out and come in on port 2222.

use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use binrw::{BinRead, BinResult, BinWrite};
use tokio::net::UdpSocket;

use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::io_packet::IoPacket;

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// Binds the I/O socket on every interface. Bind it before the Forward_Open: adapters start
/// sending as soon as they have replied.
pub async fn bind_io_socket() -> std::io::Result<UdpSocket> {
    UdpSocket::bind(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        ETHERNET_IP_IO_UDP_PORT,
    ))
    .await
}

/// Sends one I/O packet to `to`
pub async fn send_io_packet(
    socket: &UdpSocket,
    packet: &IoPacket,
    to: SocketAddrV4,
) -> BinResult<()> {
    let mut bytes = Cursor::new(Vec::new());
    packet.write(&mut bytes)?;
    socket.send_to(bytes.get_ref(), to).await?;
    Ok(())
}

/// Waits for one datagram and parses it as an I/O packet, returning it with its sender. Cancel
/// safe: a dropped call loses no datagram.
pub async fn recv_io_packet(socket: &UdpSocket) -> BinResult<(IoPacket, SocketAddr)> {
    let mut bytes = vec![0u8; MAX_UDP_PAYLOAD];
    let (len, from) = socket.recv_from(&mut bytes).await?;
    bytes.truncate(len);

    let packet = IoPacket::read(&mut Cursor::new(&bytes))?;
    Ok((packet, from))
}
