// Bare-minimum homing succession repro using eipscanne_rs explicit messaging
// directly — no io_hub_rs state machine, channels, or background tasks.
//
// Edit the constants below, then run with:
//   cargo run --bin homing_direct

use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::object_assembly::RequestObjectAssembly;

mod ethernet_ip;
mod stream_utils;

use ethernet_ip::commands::{apply_motor_command, MotorCommand};
use ethernet_ip::consts::{
    ASSEMBLY_ATTRIBUTE_ID, ASSEMBLY_OBJECT_ID, ETHERNET_IP_PORT,
    IO_HUB_4E_INPUT_ASSEMBLY_INSTANCE_ID, IO_HUB_4E_OUTPUT_ASSEMBLY_INSTANCE_ID,
};
use ethernet_ip::io_hub_input::InputAssemblyHub4E;
use ethernet_ip::io_hub_output::OutputAssemblyHub4E;

// ── Configurable constants ────────────────────────────────────────────────────

/// Milliseconds to sleep between successive homing commands.
const DELAY_MS: u64 = 100;

/// Number of homing commands to send.
const HOMING_COUNT: u32 = 10;

/// IP address of the IO-HUB-4-E device.
const DEVICE_IP: [u8; 4] = [172, 31, 19, 18];

const FAULT_CLEAR_TIMEOUT: Duration = Duration::from_millis(1000);

// ─────────────────────────────────────────────────────────────────────────────

async fn write_output(
    stream: &mut TcpStream,
    session: CipUdint,
    assembly: OutputAssemblyHub4E,
) -> anyhow::Result<()> {
    stream_utils::write_object_assembly(
        stream,
        RequestObjectAssembly::new_service_request(
            session,
            CipPath::new_full(
                ASSEMBLY_OBJECT_ID,
                IO_HUB_4E_OUTPUT_ASSEMBLY_INSTANCE_ID,
                ASSEMBLY_ATTRIBUTE_ID,
            ),
            ServiceCode::SetAttributeSingle,
            Some(assembly),
        ),
    )
    .await?;
    stream_utils::read_object_assembly::<u8>(stream).await?;
    Ok(())
}

async fn read_input(
    stream: &mut TcpStream,
    session: CipUdint,
) -> anyhow::Result<InputAssemblyHub4E> {
    stream_utils::write_object_assembly(
        stream,
        RequestObjectAssembly::<u8>::new_service_request(
            session,
            CipPath::new_full(
                ASSEMBLY_OBJECT_ID,
                IO_HUB_4E_INPUT_ASSEMBLY_INSTANCE_ID,
                ASSEMBLY_ATTRIBUTE_ID,
            ),
            ServiceCode::GetAttributeSingle,
            None,
        ),
    )
    .await?;
    let response = stream_utils::read_object_assembly::<InputAssemblyHub4E>(stream).await?;
    response
        .cip_message
        .and_then(|m| m.response_data.data)
        .ok_or_else(|| anyhow::anyhow!("Missing input assembly data in CIP response"))
}

/// Mirrors `motor_fault_clear::run_fault_clear` using direct EIP reads/writes.
///
/// Two-step handshake:
///   1. Set shutdown_reset → wait for shutdown_reset_ack high
///   2. Clear shutdown_reset (always) → wait for shutdown_reset_ack low
async fn fault_clear_if_needed(
    stream: &mut TcpStream,
    session: CipUdint,
    assembly: &mut OutputAssemblyHub4E,
) -> anyhow::Result<()> {
    let input = read_input(stream, session).await?;
    if !input.motor0_input_data.statusword.motor_shutdown_present() {
        println!("No motor fault present, skipping reset");
        return Ok(());
    }

    println!("Motor has active fault — sending ShutdownReset");
    apply_motor_command(MotorCommand::ShutdownReset, &mut assembly.motor0_output_data);
    write_output(stream, session, *assembly).await?;

    // Wait for ack high (best-effort; clear the bit regardless)
    let deadline = tokio::time::Instant::now() + FAULT_CLEAR_TIMEOUT;
    let ack_result: anyhow::Result<()> = loop {
        let input = read_input(stream, session).await?;
        if input.motor0_input_data.statusword.shutdown_reset_ack() {
            break Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            break Err(anyhow::anyhow!("Timeout waiting for shutdown_reset_ack high"));
        }
    };

    // Always clear the bit so the next fault-clear gets a clean rising edge
    apply_motor_command(
        MotorCommand::ClearShutdownReset,
        &mut assembly.motor0_output_data,
    );
    write_output(stream, session, *assembly).await?;

    let deadline = tokio::time::Instant::now() + FAULT_CLEAR_TIMEOUT;
    loop {
        let input = read_input(stream, session).await?;
        if !input.motor0_input_data.statusword.shutdown_reset_ack() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Timeout waiting for shutdown_reset_ack low");
        }
    }

    ack_result?;
    println!("Motor fault cleared");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = format!("{}:{}", Ipv4Addr::from(DEVICE_IP), ETHERNET_IP_PORT);
    println!("Connecting to {addr}");
    let mut stream = TcpStream::connect(&addr).await?;

    // ── Register session ──────────────────────────────────────────────────────
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_registration(),
    )
    .await?;
    let reg = stream_utils::read_object_assembly::<u8>(&mut stream).await?;
    let session = reg.packet_description.header.session_handle;
    println!("Session registered: {session:#010x}");

    let mut assembly = OutputAssemblyHub4E::default();

    // ── Clear any pre-existing fault ──────────────────────────────────────────
    fault_clear_if_needed(&mut stream, session, &mut assembly).await?;

    // ── Enable motor M0 ───────────────────────────────────────────────────────
    apply_motor_command(MotorCommand::Enable, &mut assembly.motor0_output_data);
    write_output(&mut stream, session, assembly).await?;
    println!("Motor M0 enabled");

    // ── Homing loop ───────────────────────────────────────────────────────────
    println!("Starting homing succession test: count={HOMING_COUNT}, delay={DELAY_MS}ms");

    for i in 0..HOMING_COUNT {
        apply_motor_command(MotorCommand::HomingMove, &mut assembly.motor0_output_data);
        write_output(&mut stream, session, assembly).await?;
        println!("HomingMove {}/{HOMING_COUNT} sent", i + 1);
        tokio::time::sleep(Duration::from_millis(DELAY_MS)).await;
    }

    // ── Unregister session ────────────────────────────────────────────────────
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(session),
    )
    .await?;
    println!("Session unregistered — done");

    Ok(())
}
