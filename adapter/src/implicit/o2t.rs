//! O->T: the outputs, always as bytes. [`route_outputs`] hands each packet to its connection, and
//! [`receive_outputs`] checks every packet of one connection and keeps the deadline.

use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use binrw::BinRead;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::time::{Instant, timeout_at};

use eipscanne_rs::cip::connection_manager::parameters::{TransportClass, connection_size};
use eipscanne_rs::cip::io_data::{CipIoData, RunOrIdle};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::{EnIpIoPacket, SequencedAddress};

use crate::error::{Error, Result};
use crate::implicit::ImplicitDevice;
use crate::implicit::connection::{ImplicitConnection, OpenConnection};
use crate::packet::decode_data;

/// The largest UDP payload, so no packet is cut short
const MAX_UDP_PAYLOAD: usize = 65_535;

/// The least time the scanner gets for its first packet
const FIRST_PACKET_GRACE: Duration = Duration::from_secs(10);
/// The smallest allowed sequence number gap
const MIN_ALLOWED_SEQUENCE_GAP: u32 = 16;

/// Hands each O->T packet on `io_socket` to the open connection its connection id names
pub(crate) async fn route_outputs(
    io_socket: Arc<UdpSocket>,
    open_connections: Arc<Mutex<HashMap<CipUdint, OpenConnection>>>,
) {
    let mut receive_buffer = vec![0u8; MAX_UDP_PAYLOAD];
    loop {
        let (len, from) = match io_socket.recv_from(&mut receive_buffer).await {
            Ok(received) => received,
            Err(error) => {
                tracing::debug!(%error, "no output packet received");
                continue;
            }
        };
        let packet = match EnIpIoPacket::read(&mut Cursor::new(&receive_buffer[..len])) {
            Ok(packet) => packet,
            Err(error) => {
                tracing::debug!(%error, "discarded an output packet");
                continue;
            }
        };
        let connection_id = packet.sequenced_address.connection_id;
        let open_connections = open_connections
            .lock()
            .expect("nothing panics while holding the connections lock");
        let Some(open_connection) = open_connections.get(&connection_id) else {
            tracing::debug!(
                connection_id = format_args!("{connection_id:#010x}"),
                "discarded an output packet for no open connection"
            );
            continue;
        };
        if open_connection
            .output_packet_sender
            .try_send((packet, from))
            .is_err()
        {
            tracing::debug!("discarded an output packet: its connection is behind");
        }
    }
}

/// Hands the device the new run mode outputs of `connection` until no packet comes in time, and
/// tells it when they switch to idle or time out
pub(crate) async fn receive_outputs(
    device: &impl ImplicitDevice,
    connection: &ImplicitConnection,
    output_packet_receiver: &mut Receiver<(EnIpIoPacket, SocketAddr)>,
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
    // Until the first idle packet
    let mut running = true;

    loop {
        let (packet, from) = match timeout_at(deadline, output_packet_receiver.recv()).await {
            Ok(Some(received)) => received,
            // The connection was closed
            Ok(None) => return,
            Err(_) => {
                tracing::warn!("the connection timed out: no output packet in time");
                device.enter_idle(connection.output_instance);
                return;
            }
        };
        let (sequenced_address, cip_io_data) =
            match check_output_packet(connection, packet, from, last_sequence_number) {
                Ok(accepted) => accepted,
                Err(error) => {
                    tracing::debug!(%error, "discarded an output packet");
                    continue;
                }
            };
        last_sequence_number = Some(sequenced_address.encapsulation_sequence_number);
        // Each accepted packet restarts the timeout
        deadline = Instant::now() + timeout;

        let run = cip_io_data
            .output_control_header
            .is_none_or(|header| header.run_or_idle() == RunOrIdle::Run);
        // The same CIP sequence count in the same mode is old data
        let repeated = cip_io_data.cip_sequence_count == last_cip_sequence_count && run == running;
        last_cip_sequence_count = cip_io_data.cip_sequence_count;
        if repeated {
            continue;
        }
        // Idle outputs keep the connection open but are not applied; the device hears of the switch
        if running && !run {
            device.enter_idle(connection.output_instance);
        }
        running = run;
        let CipDataOpt::Raw(output_data) = &cip_io_data.data else {
            unreachable!("data read from the wire is always raw");
        };
        if !run || output_data.is_empty() {
            continue;
        }
        match decode_data(output_data) {
            Ok(outputs) => device.consume_outputs(connection.output_instance, outputs),
            Err(status) => tracing::warn!(?status, "outputs not consumed"),
        }
    }
}

/// Checks a `packet` of `connection` from `from`, given the last accepted encapsulation sequence
/// number (`None` before the first). Returns its Sequenced Address and Connected Data Item, or
/// `Error::UnexpectedPacket` saying why it was discarded.
fn check_output_packet(
    connection: &ImplicitConnection,
    packet: EnIpIoPacket,
    from: SocketAddr,
    last_sequence_number: Option<CipUdint>,
) -> Result<(SequencedAddress, CipIoData)> {
    let sequenced_address: SequencedAddress = packet.sequenced_address;

    // 1. It comes from the scanner (whatever port it chose)
    if from.ip() != connection.originator_ip {
        return Err(Error::UnexpectedPacket(format!(
            "packet from {from}, not the scanner"
        )));
    }

    // 2. It is newer than the last accepted one (gap modulo 2^32 in 1..2^31), by at most
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

    // 3. It carries the output assembly after the sequence count and real-time header
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
