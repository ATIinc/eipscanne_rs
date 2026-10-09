//! The hand-written IO-HUB-4-E assemblies of `scanner/assemblies/` against the hub's own EDS, which
//! stays local:
//! `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`

use eds_parser::Eds;

// The IO-HUB assemblies live outside the library, in scanner/assemblies/
#[allow(dead_code)]
#[path = "../../scanner/assemblies"]
mod assemblies {
    pub mod io_hub;
}

use assemblies::io_hub::input::INPUT_ASSEMBLY_INSTANCE;
use assemblies::io_hub::output::OUTPUT_ASSEMBLY_INSTANCE;

fn local_eds() -> Eds {
    let path = std::env::var("EDS_FILE").expect("EDS_FILE names the file to read");
    // Cargo runs tests from the crate directory; a relative path is taken from the repository
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path);
    Eds::parse(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[test]
#[ignore = "needs the IO-HUB-4-E EDS file named by the EDS_FILE environment variable"]
fn the_io_hub_assembly_instances_match_the_hubs_eds() {
    let eds = local_eds();
    let connection = eds.first_exclusive_owner_connection().unwrap();
    let inputs = connection.t2o.assembly.as_ref().unwrap();
    let outputs = connection.o2t.assembly.as_ref().unwrap();

    assert_eq!(inputs.instance(), Some(INPUT_ASSEMBLY_INSTANCE.into()));
    assert_eq!(outputs.instance(), Some(OUTPUT_ASSEMBLY_INSTANCE.into()));
}
