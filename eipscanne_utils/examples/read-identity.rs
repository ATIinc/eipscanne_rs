//! Registers a session with an adapter, reads its Identity object and unregisters again.
//!
//! `cargo run --example read-identity -- [adapter IP]`

use eipscanne_rs::cip::identity::IdentityResponse;
use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use eipscanne_rs::object_assembly::RequestObjectAssembly;
use eipscanne_utils::session::Session;

/// The adapter to talk to unless one is given on the command line
const DEFAULT_ADAPTER_IP: &str = "172.31.19.10";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter_ip = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADAPTER_IP.to_string());

    // ========= Register the session ============
    println!("REQUESTING registration");
    let mut session = Session::register((adapter_ip.as_str(), ETHERNET_IP_TCP_PORT)).await?;
    println!("  session handle {:#010x}\n", session.session_handle());
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    // ========= Request the identity object ============
    println!("REQUESTING identity");
    session
        .send(&RequestObjectAssembly::new_identity(
            session.session_handle(),
        ))
        .await?;
    let (identity_response_object, identity_response) =
        session.read_typed_reply::<IdentityResponse>().await?;

    // println!("{:#?}\n", identity_response_object);      // NOTE: the :#? triggers a pretty-print
    println!("{:?}\n", identity_response_object);

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
