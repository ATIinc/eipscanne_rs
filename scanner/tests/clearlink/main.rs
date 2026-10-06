//! The ClearLink assemblies in `scanner/assemblies/` are not part of the library, so the tests
//! include them as a module, the same way the examples do.

// The tests use only part of each assembly
#[allow(dead_code)]
#[path = "../../assemblies"]
mod assemblies {
    pub mod clearlink;
}

mod config;
mod output;
