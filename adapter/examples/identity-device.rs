//! A device with an Identity object and one output assembly, served until Ctrl-C.
//!
//! Start the device, then read it with the scanner's `read-identity` example:
//!
//! ```text
//! cargo run --example identity-device
//! cargo run --example read-identity -- --host 127.0.0.1
//! ```
//!
//! The adapter serves unconnected messaging only, so `read-identity-connected` is refused.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use binrw::{BinRead, BinWrite};
use clap::Parser;
use tokio::net::TcpListener;

use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::types::CipShortString;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;

use adapter::handler::{Handler, decode};
use adapter::server::serve;

const ASSEMBLY_CLASS: u16 = 0x04;
const OUTPUT_INSTANCE: u16 = 0x64;
const DATA_ATTRIBUTE: u16 = 0x03;

/// Serves an Identity object and one output assembly
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address to listen on
    #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED)]
    host: Ipv4Addr,

    /// Product name in the Identity object
    #[arg(long, default_value = "Identity Device")]
    product_name: String,
}

/// The data of the output assembly
#[derive(BinRead, BinWrite, Debug, Clone, Copy)]
#[brw(little)]
struct Outputs {
    word: u32,
}

/// An Identity object and one output assembly
struct IdentityDevice {
    product_name: String,
    outputs: Mutex<Outputs>,
}

impl Handler for IdentityDevice {
    fn handle_get_attributes_all(
        &self,
        class: u16,
        instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        match (class, instance) {
            (IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID) => {
                Ok(CipDataOpt::Typed(Box::new(IdentityResponse {
                    vendor_id: VendorId::TeknicInc,
                    device_type: DeviceType::GenericDevice,
                    product_code: 1,
                    revision: Revision { major: 1, minor: 2 },
                    status: IdentityStatusBits::default().into(),
                    serial_number: 0x0102_0304,
                    product_name: CipShortString::from(self.product_name.clone()),
                })))
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn handle_get_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => Ok(CipDataOpt::Typed(Box::new(
                *self
                    .outputs
                    .lock()
                    .expect("nothing panics while holding the outputs lock"),
            ))),
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn handle_set_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, DATA_ATTRIBUTE) => {
                let outputs: Outputs = decode(data)?;
                tracing::info!("outputs set to {:#010x}", outputs.word);
                *self
                    .outputs
                    .lock()
                    .expect("nothing panics while holding the outputs lock") = outputs;
                Ok(())
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let listener = TcpListener::bind((args.host, ETHERNET_IP_TCP_PORT)).await?;
    tracing::info!("serving on {}", listener.local_addr()?);
    let device = IdentityDevice {
        product_name: args.product_name,
        outputs: Mutex::new(Outputs { word: 0 }),
    };

    tokio::select! {
        served = serve(listener, Arc::new(device)) => served?,
        stopped = tokio::signal::ctrl_c() => stopped?,
    }
    tracing::info!("stopped");

    Ok(())
}
