# Phase 2 — Connection Manager packets

**Status:** Not started

## Goal

Typed `binrw` + `bilge` packets for the Connection Manager services used to open and close a class 1
connection, sent through the existing explicit-messaging path (`SendRRData` to class 6, instance 1).

## Scope

New module `src/cip/connection_manager/`:

* `network_connection_parameters.rs` — `NetworkConnectionParameters` (16-bit: connection size,
  fixed/variable, priority, connection type, redundant owner) and `LargeNetworkConnectionParameters`
  (32-bit: 16-bit size, 9 reserved bits, then the same upper fields), plus a builder that adds the
  sequence count (class 1/3) and the 32-bit header to the application data size.
* `transport_type_trigger.rs` — `TransportTypeTrigger` (transport class, production trigger,
  direction).
* `timing.rs` — `PriorityTimeTick`, `ConnectionTimeoutMultiplier` (x4 … x512) and the request
  timeout computation.
* `real_time.rs` — `RealTimeFormat` (modeless, zero length, heartbeat, 32-bit header) and
  `RunIdleHeader` (run/idle, COO, ROO, reserved).
* `forward_open.rs` — `ForwardOpenRequest<NCP>` generic over the parameter width (0x54 / 0x5B),
  `ForwardOpenResponse`, `ForwardOpenUnsuccessfulResponse` (trailing remaining-path-size bytes are
  optional), `ConnectionManagerExtendedStatus` (0x0100 … 0x0207 + `Unknown`), a result helper that
  turns a `MessageRouterResponse` into success or a typed error, and a plain `ConnectionParameters`
  struct mirroring EIPScanner's. For point-to-point the originator chooses the T->O connection ID; the
  O->T ID comes back in the reply.
* `forward_close.rs` — `ForwardCloseRequest`, `ForwardCloseResponse`, unsuccessful variant.
* `object_assembly.rs` — `new_forward_open`, `new_large_forward_open`, `new_forward_close`
  constructors.

## Tests

`tests/test_forward_open.rs`, `tests/test_forward_close.rs`: byte-exact vectors using the EIPScanner
example parameters (path `20 04 24 97 2C 96 2C 64`, 32-byte assemblies, 1 s RPI, class 1,
point-to-point, scheduled priority), full encapsulated requests, success and error replies, bitfield
layouts for both parameter widths.
