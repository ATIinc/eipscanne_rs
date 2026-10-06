//! Registers a session with an adapter, reads its Identity object and unregisters again.
//!
//! `cargo run --example read-identity -- --host <adapter IP>`

use clap::Parser;

use eipscanne_rs::eip::constants::ETHERNET_IP_TCP_PORT;
use scanner::explicit::read_identity;
use scanner::session::Session;

/// Reads the Identity object of an EtherNet/IP adapter
#[derive(Parser)]
#[command(version)]
struct Args {
    /// IP address of the adapter
    #[arg(long, default_value = "172.31.19.10")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // ========= Register the session ============
    println!("REQUESTING registration");
    let mut session = Session::register((args.host.as_str(), ETHERNET_IP_TCP_PORT)).await?;
    println!("  session handle {:#010x}\n", session.session_handle());
    // ^^^^^^^^^ Register the session ^^^^^^^^^^^^

    // ========= Request the identity object ============
    println!("REQUESTING identity");
    let identity_response = read_identity(&mut session).await?;

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
