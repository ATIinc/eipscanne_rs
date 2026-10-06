# Phase 5 — socket utilities and `implicit-io` example

**Status:** Not started

## Goal

Run a real connection against an adapter without putting sockets in the library.

## Scope

* New dev-only workspace crate `eip_stream_utils/` (tokio): the stream helpers currently duplicated
  between `examples/stream_utils.rs` and `examples/write-teknic-io/duplicated_stream_utils.rs`, plus a
  UDP I/O socket wrapper (bind `0.0.0.0:2222`, send/receive `IoPacket`s with the sender address) and a
  cyclic loop helper (first send immediately, then every O->T period; receive; time out).
* `examples/implicit-io/`: register session → bind UDP → `Forward_Open`
  (`RequestObjectAssembly::new_forward_open`, reply read with `EnIpPacket::read_response`) → cyclic
  I/O printing the input data → `Forward_Close` (`RequestObjectAssembly::new_forward_close`) →
  unregister. Defaults match the OpENer sample application
  (config 151, output 150, input 100, 32 bytes each); flags for host, instances, sizes, RPI and
  `Large_Forward_Open` (`ConnectionParameters::large`).
* Root `Cargo.toml` becomes a workspace (`hex_test_macros`, `eip_stream_utils`); the existing examples
  switch to the shared crate. The dev-dependency cycle is allowed by Cargo but means the library is
  compiled twice; never use the utilities from unit tests inside `src/`.
* `examples/README.md` and `tests/integration/README.md` gain an implicit I/O walkthrough.

## Verification

With the OpENer container from `tests/integration`: the example opens the connection, exchanges data
for the requested number of cycles (OpENer echoes output assembly 150 into input assembly 100), closes
cleanly, and Wireshark dissects the packets as Forward Open / Connected Data Item / Forward Close.
