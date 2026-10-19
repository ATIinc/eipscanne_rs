//! The assemblies of a Teknic IO-HUB-4-E (ClearPath-IP motors), declared by the caller as plain
//! `binrw` structs. Not part of the scanner library: examples include `scanner/assemblies/` as a
//! module, and the library frames the assemblies without modelling their content.
//!
//! Based on the ClearPath-IP Software Reference (Appendix H has the assembly tables):
//! https://teknic.com/files/downloads/ClearPath-IP%20Software_Reference.pdf#page=60

pub mod input;
pub mod output;
