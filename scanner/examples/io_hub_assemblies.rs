//! The assemblies of a Teknic IO-HUB-4-E (ClearPath-IP motors), declared by the caller as plain
//! `binrw` structs and shared by the examples that include this file with `#[path]`. The library
//! frames them but never models their content.
//!
//! Path-based layout: this file declares the modules, `io_hub_assemblies/` holds them. The paths
//! are spelled out because a file included with `#[path]` otherwise looks for its submodules
//! next to itself.
//!
//! Based on the ClearPath-IP Software Reference, appendices A to E and H.

#[path = "io_hub_assemblies/input.rs"]
pub mod input;
#[path = "io_hub_assemblies/output.rs"]
pub mod output;
