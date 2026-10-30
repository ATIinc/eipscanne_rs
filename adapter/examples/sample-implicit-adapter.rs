//! A sample adapter with an Identity object and the OpENer sample's assemblies: configuration 151,
//! output 150 and input 100, 32 bytes each, and the heartbeat point 254 of an Input Only
//! connection. Its inputs echo the last run outputs. Two connections can be open at once: one
//! owns the outputs. Served on 127.0.0.1 until Ctrl-C.
//!
//! The scanner binds UDP 2222, so on the same host the adapter takes its outputs on another port
//! and names it in the Forward_Open reply:
//!
//! ```text
//! cargo run --example sample-implicit-adapter
//! cargo run --example read-identity -- --host 127.0.0.1
//! cargo run --example opener-implicit -- --host 127.0.0.1
//! cargo run --example opener-eds-implicit -- --host 127.0.0.1
//! ```

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use eipscanne_rs::cip::connection_manager::parameters::RealTimeFormat;
use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::types::CipShortString;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;

use adapter::implicit::{ImplicitDevice, ImplicitOptions};
use adapter::message_router::{ExplicitOptions, MessageRouter};
use adapter::server::serve_implicit;

const OUTPUT_INSTANCE: u16 = 150;
const INPUT_INSTANCE: u16 = 100;
/// The O->T point of an Input Only connection: heartbeats, no data
const INPUT_ONLY_HEARTBEAT_INSTANCE: u16 = 254;
/// Data bytes of each assembly
const DATA_SIZE: usize = 32;

/// Serves a sample adapter whose inputs echo its outputs
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address to listen on; 0.0.0.0 also serves other hosts
    #[arg(long, default_value_t = Ipv4Addr::LOCALHOST)]
    host: Ipv4Addr,

    /// UDP port of the I/O; 2222 is taken by a scanner on the same host
    #[arg(long, default_value_t = 2223)]
    io_port: u16,
}

/// An Identity object and the output assembly; the input assembly is the same bytes
struct SampleImplicitAdapter {
    outputs: Mutex<[u8; DATA_SIZE]>,
}

impl MessageRouter for SampleImplicitAdapter {
    fn handle_get_attributes_all(
        &self,
        class: u16,
        instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        match (class, instance) {
            (IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID) => {
                Ok(CipDataOpt::Typed(Box::new(IdentityResponse {
                    vendor_id: VendorId::Unknown(0xffff),
                    device_type: DeviceType::GenericDevice,
                    product_code: 1,
                    revision: Revision { major: 1, minor: 0 },
                    status: IdentityStatusBits::default().into(),
                    serial_number: 0x0000_0001,
                    product_name: CipShortString::from("Sample Implicit Adapter".to_string()),
                })))
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }
}

impl ImplicitDevice for SampleImplicitAdapter {
    type Outputs = [u8; DATA_SIZE];
    type Inputs = [u8; DATA_SIZE];

    fn assembly_layout(&self, instance: u16) -> Option<(u16, RealTimeFormat)> {
        match instance {
            // A run/idle header on the outputs only, like OpENer
            OUTPUT_INSTANCE => Some((DATA_SIZE as u16, RealTimeFormat::Header32Bit)),
            INPUT_INSTANCE => Some((DATA_SIZE as u16, RealTimeFormat::Modeless)),
            INPUT_ONLY_HEARTBEAT_INSTANCE => Some((0, RealTimeFormat::Heartbeat)),
            _ => None,
        }
    }

    fn consume_outputs(&self, _instance: u16, outputs: [u8; DATA_SIZE]) {
        tracing::debug!(?outputs, "outputs consumed");
        *self
            .outputs
            .lock()
            .expect("nothing panics while holding the outputs lock") = outputs;
    }

    fn produce_inputs(&self, _instance: u16) -> [u8; DATA_SIZE] {
        *self
            .outputs
            .lock()
            .expect("nothing panics while holding the outputs lock")
    }

    fn enter_idle(&self, _instance: u16) {
        tracing::info!("outputs idle");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `RUST_LOG` when set, otherwise info
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();

    let listener = TcpListener::bind((args.host, ETHERNET_IP_TCP_PORT)).await?;
    tracing::info!("serving on {}", listener.local_addr()?);
    tracing::info!(
        "try: cargo run --example read-identity -- --host {}",
        args.host
    );
    tracing::info!(
        "try: cargo run --example opener-implicit -- --host {}",
        args.host
    );
    tracing::info!(
        "try: cargo run --example opener-eds-implicit -- --host {}",
        args.host
    );
    let adapter = SampleImplicitAdapter {
        outputs: Mutex::new([0; DATA_SIZE]),
    };

    tokio::select! {
        served = serve_implicit(
            Arc::new(adapter),
            listener,
            ExplicitOptions::default(),
            ImplicitOptions {
                io_port: args.io_port,
                // An Exclusive Owner and an Input Only connection
                max_io_connections: 2,
                ..Default::default()
            },
        ) => served?,
        stopped = tokio::signal::ctrl_c() => stopped?,
    }
    tracing::info!("stopped");

    Ok(())
}
