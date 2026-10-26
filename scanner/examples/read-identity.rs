//! Registers a session with an adapter, reads its Identity object and unregisters again.
//!
//! Start OpENer (`tests/integration`) on the host, then run the example in the devcontainer:
//!
//! ```text
//! tests/integration/start-opener.sh
//! cargo run --example read-identity
//! ```
//!
//! `-- --host <adapter IP>` reads another adapter.

use clap::Parser;

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::{decode_reply, send_request};
use scanner::session::{Session, SessionConfig};

/// Reads the Identity object of an EtherNet/IP adapter
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the adapter
    #[arg(long, default_value = "172.28.0.10")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Register the session ============
    println!("REQUESTING registration");
    let mut session = Session::register(
        (args.host.as_str(), ETHERNET_IP_TCP_PORT),
        SessionConfig::default(),
    )
    .await?;
    println!("  session handle {:#010x}\n", session.session_handle());
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    // ========= Request the identity object ============
    println!("REQUESTING identity");
    let reply = send_request(
        &mut session,
        CipPath::new(IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID),
        ServiceCode::GetAttributeAll,
        None,
    )
    .await?;
    let identity_response: IdentityResponse = decode_reply(&reply)?;

    // println!("{:#?}\n", identity_response);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", identity_response);

    println!(
        "  --> Product Name: {:?}\n",
        String::from(identity_response.product_name)
    );
    // ^^^^^^^^^ Request the identity object ^^^^^^^^^^^^

    // ========= UnRegister the session ============
    println!("REQUESTING un-registration");
    session.unregister().await?;

    println!("UN Registered the CIP session");
    // ^^^^^^^^^ UnRegister the session ^^^^^^^^^^^^

    Ok(())
}
