# Phase 2 — Connection Manager packets

**Status:** Implemented, awaiting review (branch `feat/SW-4573-2-connection-manager`)

## Goal

Typed `binrw` + `bilge` packets for the Connection Manager services used to open and close a class 1
connection, sent through the existing explicit-messaging path (`SendRRData` to class 6, instance 1).

## Scope

New module `src/cip/connection_manager.rs` with its files in `src/cip/connection_manager/`:

* `parameters.rs` — everything the open services share:
  * `StandardNetworkConnectionParameters` (16-bit: connection size, fixed/variable, priority,
    connection type, redundant owner) and `LargeNetworkConnectionParameters` (32-bit: 16-bit size,
    9 reserved bits, then the same upper fields), with a named enum for every field
    (`ConnectionSizeType`, `ConnectionPriority`, `ConnectionType`, `RedundantOwner`);
  * `NetworkConnectionParameters`, a two-case `binrw` enum (`Standard` / `Large`) read with a
    `large: bool` argument, like `PathData::FormatAsU8/FormatAsU16`; one request struct serves both
    services instead of a generic parameter;
  * `NetworkConnectionParameters::new(direction, transport_class, large)` builds the word of one
    direction from a `ConnectionDirection` (type, priority, size type, owner, application data size,
    real-time format); the free function `connection_size()` adds the 16-bit sequence count
    (transport classes 1, 2 and 3) and the real-time header length to the application data size
    (`ConnectionDirection::data_size`); a size that does not fit
    the chosen width is a `ConnectionSizeError`;
  * `TransportTypeTrigger` (`TransportClass`, `ProductionTrigger`, `Direction`);
  * `PriorityTimeTick` (tick time, priority) and `ConnectionTimeoutMultiplier` (x4 … x512,
    `Unknown` for reserved values, `multiplier()`);
  * `RealTimeFormat` (modeless, zero length, heartbeat, 32-bit header) with `header_len()`. The
    run/idle header itself belongs to the I/O packets of phase 3.
* `forward_open.rs` — `ForwardOpenRequest` (one struct for 0x54 and 0x5B; `service_code()` picks
  the service from the parameter width; the Connection Path Size byte is derived from the path on
  write), `ConnectionParameters` (plain input struct mirroring EIPScanner's, turned into a request by
  `ForwardOpenRequest::new`), `ForwardOpenResponse`, `ForwardOpenUnsuccessfulResponse` (the trailing
  Remaining Path Size / reserved pair is optional), `ConnectionManagerExtendedStatus` (every
  Connection Manager extended status code + `Unknown`, plain `From<u16>` / `code()` matches),
  `ForwardOpenFailure` / `ForwardOpenError` and `ForwardOpenResponse::from_message_router_response`.
  For point-to-point the originator chooses the T->O connection ID; the O->T ID comes back in the
  reply.
* `forward_close.rs` — `ForwardCloseRequest` (size byte, then a reserved byte, then the path),
  `ForwardCloseResponse`, `ForwardCloseUnsuccessfulResponse`, `ForwardCloseFailure` /
  `ForwardCloseError` and `ForwardCloseResponse::from_message_router_response`, mirroring
  Forward_Open without a shared generic.
* `src/object_assembly.rs` — `RequestObjectAssembly::new_forward_open` (sends Forward_Open or Large_Forward_Open depending on
  the request) and `new_forward_close`.
* `CipDataOpt::to_bytes` (`src/cip/message/data.rs`) returns the reply data whether raw or typed.
  The `CipData` blanket impl no longer requires `BinRead` with empty arguments: `write_to` only
  writes, and `ForwardOpenRequest` reads with a `large` argument. Every type that satisfied the
  bound before still does.

## Design notes

* bilge 0.5 gives `builder()`, its setters and `build()` the visibility of the generated `new`
  (private unless `new = pub`). The builders are therefore used inside `parameters.rs` only, wrapped
  by `NetworkConnectionParameters::new`; callers and tests build the other bitfields by name with
  `Type::default()` and the `set_*` setters.

## Tests

`tests/test_forward_open.rs`, `tests/test_forward_close.rs`: byte-exact vectors using the EIPScanner
example parameters (path `20 04 24 97 2C 96 2C 64`, 32-byte assemblies, 1 s RPI, class 1,
point-to-point, scheduled priority), both parameter widths, full encapsulated requests, success and
rejected replies (with and without the remaining path size), a reply to another service, an
oversized standard connection, bitfield layouts, the timeout multiplier and the extended status
codes.

## Verification

```
cargo fmt --check && cargo clippy --all-targets && cargo test --all && cargo test --all --features adapter && cargo test --examples && cargo build --features async
```
