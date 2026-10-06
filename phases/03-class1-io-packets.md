# Phase 3 — class 1 I/O packets

**Status:** Delivered with phase 2 (PR #4, branch `feat/SW-4573-2-connection-manager`)

## Goal

The UDP payload exchanged on port 2222 once a connection is open.

## Scope

* `CommonPacketItem` (`src/eip/description.rs`) gains two variants, read, written and falling
  back to `Unknown` like the existing ones; SendRRData packets are untouched:
  * `SequencedAddressItem(SequencedAddress { connection_id, encapsulation_sequence_number })` (0x8002, length 8);
  * `ConnectedDataItem(CipDataOpt)` (0x00B1). A Connected Data Item is always read as
    `CipDataOpt::Raw` of the item's length, because the Common Packet Format layer cannot know
    the transport class and real-time format of the connection the data belongs to; on write the
    `CipDataOpt` (raw or typed) is serialized and the Length derived from it.
* `src/eip/io_packet.rs` — `IoPacket`: a Common Packet Format packet without encapsulation header
  (the item count, derived on write like `RRPacketData`, then the items), normally exactly a
  Sequenced Address Item followed by a Connected Data Item.
  `IoPacket::new(connection_id, encapsulation_sequence_number, data)` builds one;
  `sequenced_address()` and `connected_data()` find the two items.
* `src/cip/io_data.rs` — `IoData`, the content of a Connected Data Item once the connection is
  known: `cip_sequence_count: Option<CipUint>` (transport classes 1, 2 and 3),
  `run_idle_header: Option<RunIdleHeader>` (when the direction's `RealTimeFormat` is
  `Header32Bit`) and `data: CipDataOpt`, the application data declared by the caller. Read with
  `(byte_len, TransportClass, RealTimeFormat)` arguments: the optional fields are read when the
  arguments say so and the data gets what is left of the item (`byte_len` minus the connection
  size of zero data bytes, the same `connection_size()` arithmetic the Forward_Open used). Written
  with no arguments, so an `IoData` is a `CipData` and goes into
  `ConnectedDataItem(CipDataOpt::Typed(..))`. On the wire the order is sequence count, header,
  data. (EIPScanner reads the header before the sequence count; that is not mirrored.)
* `RunIdleHeader` (`io_data.rs`): a 32-bit `bilge` bitfield (`run_idle`, `claim_output_ownership`, `ready_for_ownership_of_outputs`, 28
  reserved bits), built with its builder like every other bitfield (`new = pub`).
* `RealTimeFormat` and `TransportClass` stay in `parameters.rs`; `IoData` only uses them.

## Tests

`tests/test_io_packet.rs`: the two first packets of the connection the phase 2 tests open
(`tests/common.rs`: 32-byte assemblies, class 1, a 32-bit header on the originator to target
data only): the packet the scanner sends (connection ID 0xa1b2c3d4, the header in run, item
length 38) and the packet the adapter sends (connection ID 0x12345678, modeless, item length 34).
Each is built through `IoPacket::new` with a typed `IoData`, written and compared byte for byte,
read back (the data item raw) and compared with the typed packet, then the raw data item is
decoded into the typed `IoData` with the transport class and real-time format of its direction.
The dissection comments are generated with `scripts/dissect.sh --udp`; Wireshark shows the data
item as plain data because it has not seen the Forward_Open of the connection.

## Verification

```
cargo fmt --check && cargo clippy --all-targets && cargo test --all && cargo test --all --features adapter && cargo test --examples
```
