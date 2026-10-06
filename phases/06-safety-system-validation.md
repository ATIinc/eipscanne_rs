# Phase 6 — Safety System validation

## Goal

Prove the stack against the real target of SW-4573.

## Scope

* Derive the connection from the Safety System EDS through the phase 5 bridge and run the
  `implicit-io` example against the device.
* Capture the exchange with Wireshark and turn it into byte-exact regression tests for `Forward_Open`,
  the I/O packets and `Forward_Close` (same style as `tests/test_object_assembly.rs`).
* Fix device-specific behaviour found on the way (non-default UDP port via Sockaddr Info,
  variable-size connections, run/idle expectations, timeouts).
