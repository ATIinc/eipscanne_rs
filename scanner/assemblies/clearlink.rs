//! The assemblies of a Teknic ClearLink controller, declared by the caller as plain `binrw`
//! structs. Not part of the scanner library: the `write-clearlink-io` and `clearlink-homing`
//! examples include `scanner/assemblies/` as a module, and the library frames the assemblies
//! without modelling their content.
//!
//! Based on the ClearLink Ethernet/IP Object Reference:
//! https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf

pub mod config;
pub mod input;
pub mod output;
