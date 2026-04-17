// Reproduces a bug in ClearPath-IP motors where sending repeated homing commands
// before homing has completed leaves the motor in a bad state.
//
// The repro sequence:
//   1. Clear any pre-existing faults
//   2. Enable the motor
//   3. Send HOMING_COUNT homing commands, spaced DELAY_MS apart
//      (each arrives while the previous homing is still in progress)
//   4. Wait up to POST_HOMING_OBSERVE_SECS for homing to complete
//
// Edit the constants below, then run with:
//   cargo run --bin teknic-ip-homing-repro

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
use ethernet_ip::io_hub_input::{InputAssemblyHub4E, MotorInputData};
use ethernet_ip::io_hub_output::OutputAssemblyHub4E;

// ── Configurable constants ────────────────────────────────────────────────────

/// Milliseconds to sleep between successive homing commands.
const DELAY_MS: u64 = 100;

/// Milliseconds between input polls during the post-homing observation window.
const OBSERVE_POLL_MS: u64 = 500;

/// Seconds to observe motor state after all homing commands complete.
const POST_HOMING_OBSERVE_SECS: u64 = 10;

/// Number of homing commands to send in rapid succession.
/// Set to > 1 to reproduce the bug: each command arrives while the previous
/// homing is still in progress.
const HOMING_COUNT: u32 = 1;

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

fn log_motor_status(m: &MotorInputData) {
    let sw = m.statusword;
    println!(
        "  pos={} vel={} torque={} | homing={} has_homed={} in_home_sensor={} | \
         enabled={} ready={} cmd_complete={} shutdown={} warning={} | \
         move_type_ack={} move_num_ack={}",
        m.position_measured,
        m.velocity_measured,
        m.torque_measured,
        sw.homing() as u8,
        sw.has_homed() as u8,
        sw.in_home_sensor() as u8,
        sw.enabled() as u8,
        sw.ready_for_command() as u8,
        sw.command_complete() as u8,
        sw.motor_shutdown_present() as u8,
        sw.motor_warning_present() as u8,
        m.move_type_ack,
        m.move_number_ack,
    );
}

async fn run_homing_loop(
    stream: &mut TcpStream,
    session: CipUdint,
    assembly: &mut OutputAssemblyHub4E,
) -> anyhow::Result<()> {
    // Read the current move number before starting so we can increment from it.
    let initial_input = read_input(stream, session).await?;
    let initial_move_number = initial_input.motor0_input_data.move_number_ack;
    println!(
        "Starting homing succession test: count={HOMING_COUNT}, delay={DELAY_MS}ms, \
         initial_move_number={initial_move_number}"
    );

    let mut move_number = initial_move_number;

    // Phase 1: fire homing commands in rapid succession, before the previous
    // homing has a chance to complete. This is what triggers the bug.
    for i in 0..HOMING_COUNT {
        move_number = move_number.wrapping_add(1);
        assembly.motor0_output_data.move_number = move_number;
        apply_motor_command(MotorCommand::HomingMove, &mut assembly.motor0_output_data);
        write_output(stream, session, *assembly).await?;
        println!("  HomingMove {}/{HOMING_COUNT} sent (move_number={move_number})", i + 1);
        tokio::time::sleep(Duration::from_millis(DELAY_MS)).await;
    }

    // Phase 2: poll until homing completes or the observation window expires.
    println!("All {HOMING_COUNT} commands sent — polling for completion (timeout={POST_HOMING_OBSERVE_SECS}s)");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(POST_HOMING_OBSERVE_SECS);
    loop {
        let input = read_input(stream, session).await?;
        log_motor_status(&input.motor0_input_data);
        if input.motor0_input_data.statusword.has_homed() {
            println!("Homing complete");
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            println!("Observation timeout — homing did not complete in {POST_HOMING_OBSERVE_SECS}s");
            break;
        }
        tokio::time::sleep(Duration::from_millis(OBSERVE_POLL_MS)).await;
    }
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

    // ── Homing loop (cancellable via Ctrl+C) ─────────────────────────────────
    tokio::select! {
        result = run_homing_loop(&mut stream, session, &mut assembly) => {
            result?;
        },
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C received — cancelled early");
        },
    }

    apply_motor_command(MotorCommand::Disable, &mut assembly.motor0_output_data);
    write_output(&mut stream, session, assembly).await?;
    println!("Motor M0 disabled");

    // ── Unregister session ────────────────────────────────────────────────────
    stream_utils::write_object_assembly(
        &mut stream,
        RequestObjectAssembly::new_unregistration(session),
    )
    .await?;
    println!("Session unregistered — done");

    Ok(())
}
