//! The assemblies of a Teknic ClearLink controller, declared by the caller as plain `binrw`
//! structs and shared by the `write-teknic-io` and `clearlink-homing` examples, which include
//! this file with `#[path]`. The library frames them but never models their content.
//!
//! Path-based layout: this file declares the modules, `clearlink_assemblies/` holds them. The
//! paths are spelled out because a file included with `#[path]` otherwise looks for its
//! submodules next to itself.
//!
//! Based on the ClearLink Ethernet/IP Object Reference:
//! https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf

#[path = "clearlink_assemblies/config.rs"]
pub mod config;
#[path = "clearlink_assemblies/input.rs"]
pub mod input;
#[path = "clearlink_assemblies/output.rs"]
pub mod output;
