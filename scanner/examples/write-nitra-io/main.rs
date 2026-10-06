//! Energizes or releases solenoid valves on a Nitra EtherNet/IP pneumatic valve manifold over
//! explicit messaging: read the status byte, then write the valve assembly.
//!
//! `cargo run --example write-nitra-io -- --host 172.31.19.60 --valves 0 2 --on`
//! `cargo run --example write-nitra-io -- --host 172.31.19.60 --valves 7 --pulse 800`

use std::time::Duration;

use clap::Parser;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{ASSEMBLY_CLASS_ID, ASSEMBLY_DATA_ATTRIBUTE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{decode_reply, send_request};
use scanner::session::Session;

mod nitra;

use nitra::{
    STATUS_ASSEMBLY_INSTANCE, SolenoidValves, StatusByte, VALVE_COUNT, VALVES_ASSEMBLY_INSTANCE,
};

/// Sets solenoid valves on a Nitra EtherNet/IP pneumatic valve manifold
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the manifold
    #[arg(long)]
    host: String,

    /// The valves to act on (0-15), space separated
    #[arg(long, required = true, num_args = 1.., value_parser = clap::value_parser!(u8).range(0..VALVE_COUNT as i64))]
    valves: Vec<u8>,

    /// Energize the valves and leave them energized
    #[arg(long, conflicts_with_all = ["off", "pulse"])]
    on: bool,

    /// Release the valves
    #[arg(long, conflicts_with_all = ["on", "pulse"])]
    off: bool,

    /// Energize the valves for this many milliseconds, then release them
    #[arg(long, value_name = "MS", conflicts_with_all = ["on", "off"])]
    pulse: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.on && !args.off && args.pulse.is_none() {
        return Err("one of --on, --off or --pulse <MS> is required".into());
    }

    // ========= Register the session ============
    println!("REQUESTING - REGISTER session");
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;

    // ========= Read the manifold status ============
    println!("REQUESTING - GET status");
    let status_reply = send_request(
        &mut session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            STATUS_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await?;
    let status: StatusByte = decode_reply(&status_reply)?;
    println!("  {status:?}");

    // ========= Write the valves ============
    let mut valves = SolenoidValves::default();
    for &index in &args.valves {
        valves.set_valve(usize::from(index), args.on || args.pulse.is_some());
    }
    write_valves(&mut session, valves).await?;

    if let Some(ms) = args.pulse {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        write_valves(&mut session, SolenoidValves::default()).await?;
    }

    // ========= UnRegister the session ============
    println!("REQUESTING - UN REGISTER session");
    session.unregister().await?;

    Ok(())
}

/// Set_Attribute_Single on the valve assembly: every valve takes the state given here
async fn write_valves(
    session: &mut Session,
    valves: SolenoidValves,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("REQUESTING - SET valves {valves:?}");
    send_request(
        session,
        CipPath::new_full(
            ASSEMBLY_CLASS_ID,
            VALVES_ASSEMBLY_INSTANCE,
            ASSEMBLY_DATA_ATTRIBUTE_ID,
        ),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(valves)),
    )
    .await?;
    Ok(())
}
