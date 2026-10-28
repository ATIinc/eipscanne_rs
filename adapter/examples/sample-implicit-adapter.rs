//! A sample adapter with an Identity object and the OpENer sample's assemblies: configuration 151,
//! output 150 and input 100, 32 bytes each. Its inputs echo the last outputs. Served on 127.0.0.1
//! until Ctrl-C.
//!
//! The scanner binds UDP 2222, so on the same host the adapter takes its outputs on another port
//! and names it in the Forward_Open reply:
//!
//! ```text
//! cargo run --example sample-implicit-adapter
//! cargo run --example read-identity -- --host 127.0.0.1
//! cargo run --example opener-implicit -- --host 127.0.0.1
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

use adapter::device::{Device, ImplicitDevice};
use adapter::server::serve;

const OUTPUT_INSTANCE: u16 = 150;
const INPUT_INSTANCE: u16 = 100;
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
    io_port: u16,
    outputs: Mutex<[u8; DATA_SIZE]>,
}

impl Device for SampleImplicitAdapter {
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

    fn as_implicit_device(&self) -> Option<&dyn ImplicitDevice> {
        Some(self)
    }
}

impl ImplicitDevice for SampleImplicitAdapter {
    fn io_port(&self) -> u16 {
        self.io_port
    }

    fn implicit_assembly(&self, instance: u16) -> Option<(u16, RealTimeFormat)> {
        match instance {
            // A run/idle header on the outputs only, like OpENer
            OUTPUT_INSTANCE => Some((DATA_SIZE as u16, RealTimeFormat::Header32Bit)),
            INPUT_INSTANCE => Some((DATA_SIZE as u16, RealTimeFormat::Modeless)),
            _ => None,
        }
    }

    fn consume_outputs(&self, _instance: u16, data: &[u8]) {
        tracing::debug!(?data, "outputs consumed");
        self.outputs
            .lock()
            .expect("nothing panics while holding the outputs lock")
            .copy_from_slice(data);
    }

    fn produce_inputs(&self, _instance: u16) -> CipDataOpt {
        CipDataOpt::Raw(
            self.outputs
                .lock()
                .expect("nothing panics while holding the outputs lock")
                .to_vec(),
        )
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
    let adapter = SampleImplicitAdapter {
        io_port: args.io_port,
        outputs: Mutex::new([0; DATA_SIZE]),
    };

    tokio::select! {
        served = serve(Arc::new(adapter), listener) => served?,
        stopped = tokio::signal::ctrl_c() => stopped?,
    }
    tracing::info!("stopped");

    Ok(())
}
