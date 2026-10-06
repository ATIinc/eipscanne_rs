# Phase 3 — class 1 I/O packets

**Status:** Not started

## Goal

The UDP payload exchanged on port 2222 once a connection is open.

## Scope

* `src/cip/io_data.rs` — `IoData`: optional 16-bit CIP sequence count (class 1/2/3), optional
  32-bit run/idle header (`RunIdleHeader`: run/idle, COO, ROO, reserved; present when the
  direction's `RealTimeFormat` is `Header32Bit`), then the application data. On the wire the order is sequence count, header,
  data. (EIPScanner reads the header before the sequence count; that is not mirrored.)
* `CommonPacketItem` (`src/eip/description.rs`) gains the `SequencedAddressItem` (0x8002:
  connection ID + encapsulation sequence number) and `ConnectedDataItem` (0x00B1) variants; their
  `CommonPacketItemId` values already exist. The Connected Data Item data stays raw bytes, because
  a receiver must look the connection up by ID before it knows how to decode the data.
* `src/eip/io_packet.rs` — `IoPacket`: a Common Packet Format packet without encapsulation header
  (item count, then the items) carrying those two items.

## Tests

`tests/test_io_packet.rs`: 32-byte class 1 packets with and without the 32-bit header, heartbeat
packets, round trips, run/idle bit layout.
