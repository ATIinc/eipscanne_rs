//! Prints the layout of the assemblies an EDS file describes: one line per member with its byte
//! offset, size, type and param, and the bit or value names under it. A device's assembly
//! structs (`scanner/assemblies/`) are written from this, by hand or by handing it to Claude, and
//! then checked against the same file with `eds_parser::check_assembly`.
//!
//! `cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100`

use clap::Parser;

use eds_parser::Eds;

/// Prints the layout of an EDS file's assemblies
#[derive(Parser)]
#[command(version)]
struct Args {
    /// The device's EDS file
    #[arg(long)]
    eds: std::path::PathBuf,

    /// Only this assembly (`Assem100`). Default: the connections, then every assembly
    #[arg(long)]
    assembly: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let text = std::fs::read_to_string(&args.eds)?;
    let eds = Eds::parse(&text)?;

    if let Some(keyword) = &args.assembly {
        let assembly = eds
            .assembly(keyword)
            .ok_or_else(|| format!("the EDS has no assembly called {keyword}"))?;
        print!("{assembly}");
        return Ok(());
    }

    // Which assembly each connection carries, so the right ones get written
    for connection in &eds.connections {
        println!(
            "{} \"{}\": O->T {}, T->O {}",
            connection.keyword,
            connection.name,
            connection.o2t.format.as_deref().unwrap_or("(no format)"),
            connection.t2o.format.as_deref().unwrap_or("(no format)")
        );
    }
    for assembly in &eds.assemblies {
        println!();
        print!("{assembly}");
    }

    Ok(())
}
