//! T->O: the inputs. [`send_inputs`] sends one packet every T->O actual packet interval and owns
//! the sequence numbers.

use std::io::Cursor;
use std::time::Duration;

use bilge::prelude::u2;
use binrw::BinWrite;
use tokio::net::UdpSocket;
use tokio::time::interval;

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::io_data::{CipIoData, OutputControlHeader, RunOrIdle};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::types::{CipUdint, CipUint};
use eipscanne_rs::eip::io_packet::EnIpIoPacket;

use crate::implicit::ImplicitDevice;
use crate::implicit::connection::ImplicitConnection;

/// Sends the device's inputs on `connection` every T->O actual packet interval; never returns
pub(crate) async fn send_inputs(
    device: &impl ImplicitDevice,
    connection: &ImplicitConnection,
    io_socket: &UdpSocket,
) {
    let mut t2o_interval = interval(Duration::from_micros(u64::from(
        connection.response.t2o_actual_packet_interval,
    )));
    // Starts at random and advances every packet
    let mut encapsulation_sequence_number: CipUdint = rand::random();
    // Advances when the inputs change
    let mut cip_sequence_count: CipUint = 0;
    let mut last_inputs: Option<Vec<u8>> = None;
    // Always run mode
    let output_control_header = match connection.t2o_real_time_format {
        RealTimeFormat::Header32Bit => Some(
            OutputControlHeader::builder()
                .run_or_idle(RunOrIdle::Run)
                .claim_output_ownership(false)
                .ready_for_ownership_of_outputs(u2::new(0))
                .build(),
        ),
        RealTimeFormat::Modeless | RealTimeFormat::ZeroLength | RealTimeFormat::Heartbeat => None,
    };

    loop {
        t2o_interval.tick().await;
        let mut inputs = Vec::new();
        if let Err(error) = device
            .produce_inputs(connection.input_instance)
            .write_le(&mut Cursor::new(&mut inputs))
        {
            tracing::warn!(%error, "inputs not sent");
            continue;
        }
        // Only inputs of the input assembly's size are sent
        if inputs.len() != usize::from(connection.input_size) {
            tracing::warn!(
                len = inputs.len(),
                input_size = connection.input_size,
                "inputs not sent: not the size of the input assembly"
            );
            continue;
        }
        if last_inputs.as_ref() != Some(&inputs) {
            cip_sequence_count = cip_sequence_count.wrapping_add(1);
            last_inputs = Some(inputs.clone());
        }

        let packet = EnIpIoPacket::new(
            connection.response.t2o_network_connection_id,
            encapsulation_sequence_number,
            CipDataOpt::Typed(Box::new(CipIoData {
                cip_sequence_count: Some(cip_sequence_count),
                output_control_header,
                data: CipDataOpt::Raw(inputs),
            })),
        );
        encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
        let mut packet_bytes = Vec::new();
        packet
            .write(&mut Cursor::new(&mut packet_bytes))
            .expect("an I/O packet of raw inputs writes to memory");
        if let Err(error) = io_socket
            .send_to(&packet_bytes, connection.t2o_endpoint)
            .await
        {
            tracing::warn!(%error, "inputs not sent");
        }
    }
}
