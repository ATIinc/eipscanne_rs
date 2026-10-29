//! O->T: the outputs, always as bytes. [`receive_outputs`] checks every packet and keeps the
//! deadline.

use std::io::Cursor;
use std::net::SocketAddr;
use std::time::Duration;

use binrw::BinRead;
use tokio::net::UdpSocket;
use tokio::time::{Instant, timeout_at};

use eipscanne_rs::cip::connection_manager::parameters::{TransportClass, connection_size};
use eipscanne_rs::cip::io_data::CipIoData;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::{EnIpIoPacket, SequencedAddress};

use crate::error::{Error, Result};
use crate::implicit::ImplicitDevice;
use crate::implicit::connection::ImplicitConnection;
use crate::packet::decode_data;

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// The least time the scanner gets for its first packet
const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// The smallest allowed sequence number gap
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Hands the device the new run mode outputs of `connection` until no packet comes in time
pub(crate) async fn receive_outputs(
    device: &impl ImplicitDevice,
    connection: &ImplicitConnection,
    io_socket: &UdpSocket,
) {
    // The timeout multiplier times the O->T actual packet interval; x512 on a 10 s interval
    // overflows 32-bit microseconds
    let timeout = Duration::from_micros(
        u64::from(
            connection
                .request
                .connection_timeout_multiplier
                .multiplier(),
        ) * u64::from(connection.response.o2t_actual_packet_interval),
    );
    let mut deadline = Instant::now() + FIRST_PACKET_GRACE.max(timeout);
    // None until the first accepted packet
    let mut last_sequence_number: Option<CipUdint> = None;
    let mut last_cip_sequence_count: Option<CipUint> = None;
    let mut receive_buffer = vec![0u8; MAX_UDP_PAYLOAD];

    loop {
        let Ok(received) = timeout_at(deadline, io_socket.recv_from(&mut receive_buffer)).await
        else {
            tracing::warn!("the connection timed out: no output packet in time");
            return;
        };
        let checked = received.map_err(Error::from).and_then(|(len, from)| {
            check_output_packet(
                connection,
                last_sequence_number,
                &receive_buffer[..len],
                from,
            )
        });
        let (sequenced_address, cip_io_data) = match checked {
            Ok(accepted) => accepted,
            Err(error) => {
                tracing::debug!(%error, "discarded an output packet");
                continue;
            }
        };
        last_sequence_number = Some(sequenced_address.encapsulation_sequence_number);
        // Each accepted packet restarts the timeout
        deadline = Instant::now() + timeout;

        // The same CIP sequence count is old data
        let repeated = cip_io_data.cip_sequence_count == last_cip_sequence_count;
        last_cip_sequence_count = cip_io_data.cip_sequence_count;
        // Idle outputs keep the connection open but are not applied
        let idle = cip_io_data
            .run_idle_header
            .is_some_and(|header| !header.run_idle());
        let CipDataOpt::Raw(output_data) = &cip_io_data.data else {
            unreachable!("data read from the wire is always raw");
        };
        if repeated || idle || output_data.is_empty() {
            continue;
        }
        match decode_data(output_data) {
            Ok(outputs) => device.consume_outputs(connection.output_instance, outputs),
            Err(status) => tracing::warn!(?status, "outputs not consumed"),
        }
    }
}

/// Checks the `packet_bytes` of a packet of `connection` from `from`, given the last accepted
/// encapsulation sequence number (`None` before the first). Returns its Sequenced Address and
/// Connected Data Item, or `Error::UnexpectedPacket` saying why it was discarded.
fn check_output_packet(
    connection: &ImplicitConnection,
    last_sequence_number: Option<CipUdint>,
    packet_bytes: &[u8],
    from: SocketAddr,
) -> Result<(SequencedAddress, CipIoData)> {
    // 1. It is an I/O packet for this connection
    let packet = EnIpIoPacket::read(&mut Cursor::new(packet_bytes))?;
    let sequenced_address: SequencedAddress = packet.sequenced_address;
    if sequenced_address.connection_id != connection.response.o2t_network_connection_id {
        return Err(Error::UnexpectedPacket(format!(
            "packet for another connection ({:#010x})",
            sequenced_address.connection_id
        )));
    }

    // 2. It comes from the scanner (whatever port it chose)
    if from.ip() != connection.originator_ip {
        return Err(Error::UnexpectedPacket(format!(
            "packet from {from}, not the scanner"
        )));
    }

    // 3. It is newer than the last accepted one (gap modulo 2^32 in 1..2^31), by at most
    //    the timeout multiplier plus one (never under 16). Any first number is accepted
    if let Some(last) = last_sequence_number {
        let sequence_number: CipUdint = sequenced_address.encapsulation_sequence_number;
        let gap: u32 = sequence_number.wrapping_sub(last);
        if gap == 0 || gap >= 1 << 31 {
            return Err(Error::UnexpectedPacket(format!(
                "stale sequence number {sequence_number} (last accepted {last})"
            )));
        }
        // The timeout multiplier as a factor: 4, 8, ... 512
        let multiplier: u32 = connection
            .request
            .connection_timeout_multiplier
            .multiplier();
        let max_gap: u32 = MIN_ALLOWED_SEQUENCE_GAP.max(multiplier.saturating_add(1));
        if gap > max_gap {
            return Err(Error::UnexpectedPacket(format!(
                "sequence number {sequence_number} is more than {max_gap} ahead of the last accepted {last}"
            )));
        }
    }

    // 4. It carries the output assembly after the sequence count and real-time header
    let CipDataOpt::Raw(connected_data) = &packet.connected_data else {
        unreachable!("data read from the wire is always raw");
    };
    let real_time_format = connection.o2t_real_time_format;
    let o2t_connection_size = connection_size(
        connection.output_size,
        TransportClass::Class1,
        real_time_format,
    );
    if connected_data.len() != usize::from(o2t_connection_size) {
        return Err(Error::UnexpectedPacket(format!(
            "{} connected data bytes, the connection carries {o2t_connection_size}",
            connected_data.len()
        )));
    }
    let cip_io_data = CipIoData::read_le_args(
        &mut Cursor::new(connected_data),
        (
            o2t_connection_size,
            TransportClass::Class1,
            real_time_format,
        ),
    )?;

    Ok((sequenced_address, cip_io_data))
}
