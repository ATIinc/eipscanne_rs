//! Exchanges cyclic I/O with a SICK flexi-net over a class 1 connection, in the five stages of
//! `implicit-io`. The connection is the device's: configuration assembly 1278 with no
//! configuration data, outputs to connection point 151 and inputs from connection point 152, 4
//! data bytes each way. The connection path carries no electronic key.
//!
//! The outputs count slowly so each value can be watched on the device: one byte at a time,
//! byte `i` counts from `10 * i` to `10 * i + 10` and then holds it while the next byte counts,
//! one step every `--count-delay`, while packets keep going out every RPI. Once the last count
//! has been held for one delay, or on Ctrl+C, one packet sets every byte back to 0 and the
//! connection closes. A longer test is a longer `--count-delay`.
//!
//! The FLX3-GEPR100's standard assemblies, from SICK's "Flexi Gateway GEPR1" operating
//! instructions (8026788, chapter 14.4.2). The EDS file is "EDS (FLX3-GEPR)" at
//! <https://www.sick.com/us/en/downloads/media/swp1656593>.
//!
//! | Instances | Direction | Size |
//! |---|---|---|
//! | 130, 131, 132 | inputs (T->O) | 50, 50, 100 bytes |
//! | 140, 141, 142 | outputs (O->T) | 50, 50, 100 bytes |
//! | 150 to 189 | up to 4, defined in Safety Designer | up to 100 bytes |
//!
//! 151 and 152 are of the last kind: they exist only once the Safety Designer configuration
//! defining them is on the device, with the sizes it gives them.
//!
//! The electronic key (vendor 808, device type 100, product code 11584, revision 1.1, the
//! device's Identity object) is optional; with it in the path, a different device would refuse
//! the connection.
//!
//! `cargo run --example flexi-net-implicit -- --host 172.31.19.210`

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bilge::prelude::{u4, u9};
use clap::Parser;
use tokio::net::UdpSocket;

use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::parameters::{
    ConnectionPriority, ConnectionSizeType, ConnectionTimeoutMultiplier, ConnectionType, Direction,
    NetworkConnectionParameters, PriorityTimeTick, ProductionTrigger, RealTimeFormat,
    RedundantOwner, StandardNetworkConnectionParameters, TransportClass, TransportTypeTrigger,
    connection_size,
};
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::object_ids::ASSEMBLY_CLASS_ID;
use eipscanne_rs::cip::path::{CipPath, LogicalPathSegment, LogicalSegmentType};
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use eipscanne_rs::eip::io_packet::EnIpIoPacket;
use scanner::connection_manager::forward_close;
use scanner::error::Error;
use scanner::implicit::connection::{OpenConnection, forward_open};
use scanner::implicit::o2t::{build_o2t_packet, send_io_packet};
use scanner::implicit::t2o::{
    FIRST_PACKET_GRACE, accept_t2o_packet, bind_io_socket, input_timeout, recv_io_packet,
};
use scanner::session::Session;

/// Who this scanner is in the Forward_Open (the same values as `implicit-io`). The three form
/// the connection triad: the adapter tells connections apart by it, so it only has to differ
/// from other connections to the device, and the Forward_Close repeats it to close this one.
const ORIGINATOR_VENDOR_ID: u16 = 342;
const ORIGINATOR_SERIAL_NUMBER: u32 = 0x0001_2345;
const CONNECTION_SERIAL_NUMBER: u16 = 1;
/// The connection ID the adapter puts on every input packet. On a point-to-point T->O
/// connection the scanner picks it; any value works, and this one is easy to spot in a capture.
const T2O_NETWORK_CONNECTION_ID: u32 = 0x1234_5678;

/// The flexi-net's assembly instances and data sizes, fixed by the device's connection
const CONFIGURATION_INSTANCE: u16 = 1278;
const OUTPUT_CONNECTION_POINT: u8 = 151;
const INPUT_CONNECTION_POINT: u8 = 152;
const OUTPUT_SIZE: u16 = 4;
const INPUT_SIZE: u16 = 4;
/// The device's run/idle signalling. The outputs carry a 32-bit header whose run flag tells the
/// device to apply them or go idle; the inputs carry data only, as the device has no run/idle
/// to report. The Forward_Open has no field for the format: it shows only in the connection
/// sizes, 10 bytes out (4 data, 2 sequence count, 4 header) and 6 in, which the device checks
/// exactly (they are fixed), so a wrong format makes it refuse the connection.
const O2T_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Header32Bit;
const T2O_REAL_TIME_FORMAT: RealTimeFormat = RealTimeFormat::Modeless;

/// How far each output byte counts: 10 steps, enough to see it move; the test's length comes
/// from the delay between steps, not from more steps
const COUNT_STEPS: u8 = 10;
/// Distance between the start values of neighbouring bytes. Equal to the count, so byte `i`
/// covers `10 * i ..= 10 * i + 10`: each byte starts where the one before it ended, and byte 3
/// ends at 40, well inside a byte.
const BYTE_SPACING: u8 = 10;

/// Exchanges cyclic I/O with a SICK flexi-net over a class 1 connection
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the flexi-net
    #[arg(long, default_value = "172.31.19.210")]
    host: String,

    /// Requested packet interval of both directions, in milliseconds
    #[arg(long, default_value_t = 20)]
    rpi: u32,

    /// Time between two counts of the outputs, in milliseconds; rounded up to whole RPIs
    #[arg(long, default_value_t = 1000)]
    count_delay: u64,
}

impl Args {
    /// A class 1 cyclic, point-to-point Forward_Open that times out after four missed packets
    fn forward_open_request(&self) -> ForwardOpenRequest {
        let requested_packet_interval = self.rpi * 1000;
        // Each connection size counts the 16-bit sequence count and the real-time header too
        let o2t_size: u16 =
            connection_size(OUTPUT_SIZE, TransportClass::Class1, O2T_REAL_TIME_FORMAT);
        let t2o_size: u16 =
            connection_size(INPUT_SIZE, TransportClass::Class1, T2O_REAL_TIME_FORMAT);

        ForwardOpenRequest {
            // 5 ticks of 1024 ms (2^10): about 5 s for the device to answer the request itself
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            // Point-to-point: the adapter chooses the O->T connection ID and returns it
            o2t_network_connection_id: 0,
            t2o_network_connection_id: T2O_NETWORK_CONNECTION_ID,
            connection_triad: ConnectionTriad {
                connection_serial_number: CONNECTION_SERIAL_NUMBER,
                originator_vendor_id: ORIGINATOR_VENDOR_ID,
                originator_serial_number: ORIGINATOR_SERIAL_NUMBER,
            },
            // The smallest multiplier: four missed packets close the connection, so a lost
            // link is noticed within four RPIs
            connection_timeout_multiplier: ConnectionTimeoutMultiplier::X4,
            o2t_requested_packet_interval: requested_packet_interval,
            o2t_network_connection_parameters: network_connection_parameters(o2t_size),
            t2o_requested_packet_interval: requested_packet_interval,
            t2o_network_connection_parameters: network_connection_parameters(t2o_size),
            transport_type_trigger: TransportTypeTrigger::builder()
                .transport_class(TransportClass::Class1)
                .production_trigger(ProductionTrigger::Cyclic)
                .direction(Direction::Client)
                .build(),
            // The configuration instance is above 255, so it needs a 16-bit segment; the
            // connection points fit in 8 bits
            connection_path: CipPath::from_segments(vec![
                LogicalPathSegment::new_u8(LogicalSegmentType::ClassId, ASSEMBLY_CLASS_ID),
                LogicalPathSegment::new_u16(LogicalSegmentType::InstanceId, CONFIGURATION_INSTANCE),
                LogicalPathSegment::new_u8(
                    LogicalSegmentType::ConnectionPoint,
                    OUTPUT_CONNECTION_POINT,
                ),
                LogicalPathSegment::new_u8(
                    LogicalSegmentType::ConnectionPoint,
                    INPUT_CONNECTION_POINT,
                ),
            ]),
        }
    }
}

/// One direction's 16-bit parameter word: a fixed size, as the device's assemblies never
/// change length, and a scheduled, exclusive point-to-point connection
fn network_connection_parameters(size: u16) -> NetworkConnectionParameters {
    NetworkConnectionParameters::Standard(
        StandardNetworkConnectionParameters::builder()
            .connection_size(u9::new(size))
            .connection_size_type(ConnectionSizeType::Fixed)
            .priority(ConnectionPriority::Scheduled)
            .connection_type(ConnectionType::PointToPoint)
            .redundant_owner(RedundantOwner::Exclusive)
            .build(),
    )
}

/// The value byte `index` starts counting from
fn start_value(index: usize) -> u8 {
    BYTE_SPACING * index as u8
}

/// The value byte `index` counts up to and then keeps
fn end_value(index: usize) -> u8 {
    start_value(index) + COUNT_STEPS
}

/// The outputs one count later: the first byte short of its end value jumps to its start value
/// if it has not started, or else goes up by one. `None` once every byte is at its end.
fn next_count(outputs: &[u8]) -> Option<Vec<u8>> {
    let index = (0..outputs.len()).find(|&index| outputs[index] < end_value(index))?;
    let mut next = outputs.to_vec();
    next[index] = if next[index] < start_value(index) {
        start_value(index)
    } else {
        next[index] + 1
    };
    Some(next)
}

/// Sends the outputs in one packet with the run flag set, so the device applies them instead of
/// idling; the encapsulation sequence number advances every packet
async fn send_outputs(
    socket: &UdpSocket,
    connection: &OpenConnection,
    encapsulation_sequence_number: &mut u32,
    cip_sequence_count: u16,
    outputs: &[u8],
) -> Result<(), Error> {
    let packet: EnIpIoPacket = build_o2t_packet(
        connection,
        *encapsulation_sequence_number,
        cip_sequence_count,
        CipDataOpt::Raw(outputs.to_vec()),
        true,
    )?;
    *encapsulation_sequence_number = encapsulation_sequence_number.wrapping_add(1);
    send_io_packet(socket, &packet, connection.o2t_endpoint).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= 1. Register the session ============
    println!("REGISTERING a session with {}", args.host);
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= 2. Open the connection ============
    // The socket first: the adapter starts sending as soon as it has replied
    let socket: UdpSocket = bind_io_socket().await?;
    println!("OPENING the connection");
    let connection: OpenConnection = forward_open(
        &mut session,
        args.forward_open_request(),
        O2T_REAL_TIME_FORMAT,
        T2O_REAL_TIME_FORMAT,
    )
    .await?;
    let established_at = Instant::now();
    println!(
        "  O->T connection {:#010x} every {} us to {}",
        connection.response.o2t_network_connection_id,
        connection.response.o2t_actual_packet_interval,
        connection.o2t_endpoint
    );
    println!(
        "  T->O connection {:#010x} every {} us from {}",
        connection.response.t2o_network_connection_id,
        connection.response.t2o_actual_packet_interval,
        connection.target_ip
    );

    // ========= 3. Exchange I/O ============
    // The outputs go out every actual packet interval, the one the adapter granted: the count
    // only decides what they carry, so the connection keeps its timing between counts
    let mut send_timer = tokio::time::interval(Duration::from_micros(u64::from(
        connection.response.o2t_actual_packet_interval,
    )));
    // The encapsulation sequence number starts at random and advances every packet; the CIP
    // sequence count advances only when the outputs change, which marks them as new data
    let mut encapsulation_sequence_number: u32 = rand::random();
    let mut cip_sequence_count: u16 = 0;

    // The count: checked on every tick, so a count lands on the first packet after it is due
    let count_delay = Duration::from_millis(args.count_delay);
    let mut next_count_at = Instant::now() + count_delay;
    let mut outputs: Vec<u8> = vec![0; usize::from(OUTPUT_SIZE)];

    // The inputs: the last accepted numbers, and the deadline every accepted packet moves
    let timeout: Duration = input_timeout(&connection);
    let mut last_sequence_number: Option<u32> = None;
    let mut last_cip_sequence_count: Option<u16> = None;
    let mut deadline = established_at + FIRST_PACKET_GRACE.max(timeout);

    // Counts output packets, one per RPI, to line up the printed sends and receives
    let mut cycle: u32 = 0;
    // One Ctrl+C future for the whole loop, so no press is lost; the connection is still closed
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = send_timer.tick() => {
                cycle += 1;
                if Instant::now() >= next_count_at {
                    // The last count has been held for one delay
                    let Some(next) = next_count(&outputs) else {
                        break;
                    };
                    outputs = next;
                    // New outputs are new data
                    cip_sequence_count = cip_sequence_count.wrapping_add(1);
                    println!("[{cycle:>5}] COUNT    {}", hex(&outputs));
                    // From the schedule, not from now, so late ticks do not add up
                    next_count_at += count_delay;
                }
                send_outputs(
                    &socket,
                    &connection,
                    &mut encapsulation_sequence_number,
                    cip_sequence_count,
                    &outputs,
                )
                .await?;
                println!("[{cycle:>5}] SENT     {}", hex(&outputs));
            }

            received = recv_io_packet(&socket) => {
                // A datagram that is not an I/O packet is logged and skipped, like a discarded one
                let (packet, from): (EnIpIoPacket, SocketAddr) = match received {
                    Ok(received) => received,
                    Err(Error::Parse(error)) => {
                        eprintln!("[{cycle:>5}] DISCARDED a datagram that does not parse: {error}");
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                match accept_t2o_packet(&connection, last_sequence_number, &packet, from) {
                    Ok((address, inputs)) => {
                        last_sequence_number = Some(address.encapsulation_sequence_number);
                        deadline = Instant::now() + timeout;
                        // An unchanged CIP sequence count means the adapter resent old inputs
                        let unchanged = last_cip_sequence_count.is_some()
                            && inputs.cip_sequence_count == last_cip_sequence_count;
                        last_cip_sequence_count = inputs.cip_sequence_count;
                        // Read from the wire, so the data is raw bytes
                        if let CipDataOpt::Raw(data) = &inputs.data {
                            println!(
                                "[{cycle:>5}] RECEIVED {}{}",
                                hex(data),
                                if unchanged { " (unchanged)" } else { "" }
                            );
                        }
                    }
                    Err(discarded) => eprintln!("[{cycle:>5}] DISCARDED {discarded}"),
                }
            }

            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                eprintln!("the connection timed out: no input packet in time");
                break;
            }

            _ = &mut ctrl_c => {
                println!("Ctrl+C: stopping");
                break;
            }
        }
    }

    // Every byte back to 0 before closing, so the device is not left at the last count
    outputs.fill(0);
    cip_sequence_count = cip_sequence_count.wrapping_add(1);
    send_outputs(
        &socket,
        &connection,
        &mut encapsulation_sequence_number,
        cip_sequence_count,
        &outputs,
    )
    .await?;
    println!("ZERO     {}", hex(&outputs));

    // ========= 4. Close the connection ============
    println!("CLOSING the connection");
    forward_close(&mut session, &connection.request).await?;

    // ========= 5. End the session ============
    println!("UNREGISTERING the session");
    session.unregister().await?;

    Ok(())
}

/// The bytes as one hex string, two digits each, in wire order
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
