//! T->O: the inputs. The I/O task sends one packet every T->O actual packet interval and owns the
//! sequence numbers.

use std::io::Cursor;
use std::net::SocketAddr;

use bilge::prelude::u2;
use binrw::BinWrite;
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::io_data::{CipIoData, RunIdleHeader};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::EnIpIoPacket;

use crate::error::{Error, Result};
use crate::implicit::connection::{ImplicitConnection, data_len_matches_connection, data_size};

/// The T->O packet of `connection` carrying `inputs`, always in run mode
pub(crate) fn build_t2o_packet(
    connection: &ImplicitConnection,
    encapsulation_sequence_number: CipUdint,
    cip_sequence_count: CipUint,
    inputs: Vec<u8>,
) -> Result<EnIpIoPacket> {
    let (data_size, size_type) = data_size(
        &connection.request.t2o_network_connection_parameters,
        connection.t2o_real_time_format,
    );
    if !data_len_matches_connection(inputs.len(), data_size, size_type) {
        return Err(Error::UnexpectedPacket(format!(
            "{} input bytes, the connection carries {data_size}",
            inputs.len()
        )));
    }

    let run_idle_header: Option<RunIdleHeader> = match connection.t2o_real_time_format {
        RealTimeFormat::Header32Bit => Some(
            RunIdleHeader::builder()
                .run_idle(true)
                .claim_output_ownership(false)
                .ready_for_ownership_of_outputs(u2::new(0))
                .build(),
        ),
        RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => None,
    };
    let cip_io_data = CipIoData {
        cip_sequence_count: Some(cip_sequence_count),
        run_idle_header,
        data: CipDataOpt::Raw(inputs),
    };
    Ok(EnIpIoPacket::new(
        connection.response.t2o_network_connection_id,
        encapsulation_sequence_number,
        CipDataOpt::Typed(Box::new(cip_io_data)),
    ))
}

/// The bytes of `inputs`, written once: the size check needs them, and a typed assembly's size is
/// only known once written
pub(crate) fn to_bytes(inputs: &CipDataOpt) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    inputs.write_le_args(&mut bytes, (0,))?;
    Ok(bytes.into_inner())
}

/// Sends one I/O packet to `to`
pub(crate) async fn send_io_packet(
    socket: &UdpSocket,
    packet: &EnIpIoPacket,
    to: SocketAddr,
) -> Result<()> {
    let mut bytes = Cursor::new(Vec::new());
    packet.write(&mut bytes)?;
    socket.send_to(bytes.get_ref(), to).await?;
    Ok(())
}
