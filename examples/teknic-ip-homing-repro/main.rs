// Bare-minimum homing succession repro using eipscanne_rs explicit messaging
// directly — no state machine, channels, or background tasks.
//
// Edit the constants below, then run with:
//   cargo run --example teknic-ip-homing-repro

mod ethernet_ip;

use std::net::Ipv4Addr;
use std::time::Duration;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

use ethernet_ip::consts::{ASSEMBLY_CLASS, ASSEMBLY_ATTRIBUTE, ETHERNET_IP_PORT, OUTPUT_ASSEMBLY_INSTANCE};
use ethernet_ip::io_hub_output::{MotorControlword, MoveType, OutputAssemblyHub4E};

// ── Configurable constants ────────────────────────────────────────────────────

/// Milliseconds to sleep between successive homing commands.
const DELAY_MS: u64 = 100;

/// Number of homing commands to send.
const HOMING_COUNT: u32 = 10;

/// IP address of the IO-HUB-4-E device.
const DEVICE_IP: [u8; 4] = [172, 31, 19, 18];

// ─────────────────────────────────────────────────────────────────────────────

async fn write_assembly<T>(stream: &mut TcpStream, request: RequestObjectAssembly<T>)
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    let mut buf: Vec<u8> = Vec::new();
    request.write(&mut std::io::Cursor::new(&mut buf)).unwrap();
    stream.write_all(&buf).await.unwrap();
}

async fn read_assembly<T>(stream: &mut TcpStream) -> Result<ResponseObjectAssembly<T>, binrw::Error>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    let mut buf = vec![0u8; 500];
    let n = stream.read(&mut buf).await?;
    buf.truncate(n);
    ResponseObjectAssembly::<T>::read(&mut std::io::Cursor::new(buf))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("{}:{}", Ipv4Addr::from(DEVICE_IP), ETHERNET_IP_PORT);
    println!("Connecting to {addr}");
    let mut stream = TcpStream::connect(&addr).await?;

    // ── Register session ──────────────────────────────────────────────────────
    write_assembly(&mut stream, RequestObjectAssembly::new_registration()).await;
    let reg = read_assembly::<u8>(&mut stream).await?;
    let session = reg.packet_description.header.session_handle;
    println!("Session registered: {session:#010x}");

    // ── Enable motor M0 ───────────────────────────────────────────────────────
    let mut assembly = OutputAssemblyHub4E::default();
    let mut cw = MotorControlword::default();
    cw.set_enable(true);
    assembly.motor0_output_data.controlword = cw;

    write_assembly(
        &mut stream,
        RequestObjectAssembly::new_service_request(
            session,
            CipPath::new_full(ASSEMBLY_CLASS, OUTPUT_ASSEMBLY_INSTANCE, ASSEMBLY_ATTRIBUTE),
            ServiceCode::SetAttributeSingle,
            Some(assembly),
        ),
    )
    .await;
    read_assembly::<u8>(&mut stream).await?;
    println!("Motor M0 enabled");

    // ── Homing succession loop ────────────────────────────────────────────────
    println!("Starting homing succession test: count={HOMING_COUNT}, delay={DELAY_MS}ms");

    for i in 0..HOMING_COUNT {
        assembly.motor0_output_data.move_type = MoveType::HomingMove.to_u8();
        assembly.motor0_output_data.move_number =
            assembly.motor0_output_data.move_number.wrapping_add(1);

        write_assembly(
            &mut stream,
            RequestObjectAssembly::new_service_request(
                session,
                CipPath::new_full(ASSEMBLY_CLASS, OUTPUT_ASSEMBLY_INSTANCE, ASSEMBLY_ATTRIBUTE),
                ServiceCode::SetAttributeSingle,
                Some(assembly),
            ),
        )
        .await;
        read_assembly::<u8>(&mut stream).await?;

        println!("HomingMove {}/{HOMING_COUNT} sent", i + 1);
        tokio::time::sleep(Duration::from_millis(DELAY_MS)).await;
    }

    // ── Unregister session ────────────────────────────────────────────────────
    write_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(session),
    )
    .await;
    println!("Session unregistered — done");

    Ok(())
}
