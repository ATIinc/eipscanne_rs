# Phase 2 — Connection Manager packets and class 1 I/O packets

## Goal

Typed `binrw` + `bilge` packets for the Connection Manager services used to open and close a class 1
connection, sent through the explicit-messaging path of phase 1 (`SendRRData` to class 6,
instance 1, addressed with 8-bit segments `20 06 24 01`), and the class 1 I/O packets exchanged
once the connection is open.

## Scope

Module `src/cip/connection_manager.rs` with its files in `src/cip/connection_manager/`:

* `parameters.rs` — everything the open services share:
  * `StandardNetworkConnectionParameters` (16-bit: 9-bit connection size, fixed/variable,
    priority, a reserved bit, connection type, redundant owner) and
    `LargeNetworkConnectionParameters` (32-bit: 16-bit size, 9 reserved bits, then the same upper
    fields), with a named enum for every field (`ConnectionSizeType`, `ConnectionPriority`,
    `ConnectionType`, `RedundantOwner`); a caller assembles the word field by field with the
    builder;
  * `NetworkConnectionParameters`, a two-case `binrw` enum (`Standard` / `Large`) read with a
    `large: bool` argument, like `PathData::FormatAsU8/FormatAsU16`, so one request struct serves
    both services;
  * the free function `connection_size()` adds the 16-bit sequence count (transport classes 1, 2
    and 3) and the real-time header length to the application data size, the one piece of
    arithmetic Wireshark does not show; whether the result fits the nine bits of a Forward_Open is
    the caller's check (`u9::try_new`);
  * `TransportTypeTrigger` (`TransportClass`, `ProductionTrigger`, `Direction`);
  * `PriorityTimeTick` (tick time, priority) and `ConnectionTimeoutMultiplier` (x4 … x512,
    `Unknown` for reserved values, `multiplier()`);
  * `RealTimeFormat` (modeless, zero length, heartbeat, 32-bit header) with `header_len()`. The
    run/idle header itself belongs to the I/O packets below.
* `shared.rs` — the block every service carries in the same layout: `ConnectionTriad`
  (connection serial number, originator vendor ID, originator serial number; in every request and
  reply, and what a Forward_Close is matched against).
* `forward_open.rs` — `ForwardOpenRequest` (one struct for 0x54 and 0x5B, built field by field
  like every other packet; `service_code()` picks the service from the width of the O->T
  parameters, so both directions use the same case; the Connection Path Size byte is derived from
  the path on write) and `ForwardOpenResponse` (connection IDs, triad, actual packet intervals,
  application reply size, reserved byte and application reply data, flat as Wireshark shows them).
  For point-to-point the originator chooses the T->O connection ID; the O->T ID comes back in the
  reply.
* `forward_close.rs` — `ForwardCloseRequest` (priority/time tick, timeout ticks and triad, then the
  Connection Path Size byte derived from the path on write, a reserved byte and the path) and
  `ForwardCloseResponse` (triad, application reply size, reserved byte, application reply data).
* `response.rs` — why a request was refused: `ConnectionManagerExtendedStatus` (a `binrw` magic
  enum of every Connection Manager extended status code + `Unknown`, like `ResponseStatusCode`;
  plain data whose `Debug` names the status, no `Display`) and
  `Rejection::extended_status() -> Option<ConnectionManagerExtendedStatus>`, which reads the first
  Additional Status word of a phase 1 `Rejection` through `binrw`. It is `Some` only for a
  Forward_Open, Large_Forward_Open or Forward_Close rejection that carries Additional Status; the
  words of any other service mean something else. The reply data of an accepted Forward_Open or
  Forward_Close is read directly as a `ForwardOpenResponse` / `ForwardCloseResponse`, like any
  other typed reply. The reply data of a rejected request is not modelled and stays raw on the
  Message Router response: the triad plus the remaining path size and reserved byte, or no data
  at all when the Message Router refuses the request before the Connection Manager sees it (a
  path segment error, for one).
* `src/object_assembly.rs` — `RequestObjectAssembly::new_forward_open` (sends Forward_Open or
  Large_Forward_Open depending on the request) and `new_forward_close`, both addressed to the
  Connection Manager through the `CONNECTION_MANAGER_CLASS_ID` / `CONNECTION_MANAGER_INSTANCE_ID`
  constants of `src/cip/object_ids.rs` (`u8`), as 8-bit segments (`CipPath::new_u8`:
  `20 06 24 01`). Both segment widths are valid for these values, but the Teknic IO-HUB-4-E
  refuses a 16-bit request path (`21 00 06 00 25 00 01 00`) with a path segment error.

## Class 1 I/O packets

The UDP payload exchanged on port 2222 once a connection is open; delivered in the same pull
request as the Connection Manager packets.
* `CommonPacketItem` (`src/eip/description.rs`) has two variants for I/O packets:
  * `SequencedAddressItem(SequencedAddress { connection_id, encapsulation_sequence_number })`
    (0x8002, length 8); an item of that type with any other length is read as `Unknown`;
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
  size of zero data bytes, the same `connection_size()` arithmetic the Forward_Open uses). Written
  with no arguments, so an `IoData` is a `CipData` and goes into
  `ConnectedDataItem(CipDataOpt::Typed(..))`. On the wire the order is sequence count, header,
  data.
* `RunIdleHeader` (`io_data.rs`): a 32-bit `bilge` bitfield (`run_idle`, `claim_output_ownership`,
  the 2-bit `ready_for_ownership_of_outputs`, 28 reserved bits), built with its builder like every
  other bitfield (`new = pub`).
* `RealTimeFormat` and `TransportClass` are defined in `parameters.rs`; `IoData` imports them.

## Design notes

* Every bitfield of `parameters.rs` (`StandardNetworkConnectionParameters`,
  `LargeNetworkConnectionParameters`, `TransportTypeTrigger`, `PriorityTimeTick`) is built with its
  builder, in the library and in the tests alike
  (`PriorityTimeTick::builder().tick_time(u4::new(10)).priority(false).build()`). bilge 0.5 gives
  `builder()`, its setters and `build()` the visibility of the generated `new`, so all four carry
  `new = pub` for no other reason than to expose the builder; the positional `new` that comes with
  it is never called.
* Field names keep Wireshark's `O->T` / `T->O` abbreviations as the `o2t_` / `t2o_` prefixes and
  spell the intervals out (`o2t_requested_packet_interval` for "O->T RPI",
  `t2o_actual_packet_interval` for "T->O API"). Every such field's docstring gives the full name,
  and the module docs of `src/cip/connection_manager.rs` map the terminology to the devices:
  originator = the scanner (this library), target = the adapter, O->T = the outputs we send,
  T->O = the inputs we receive.

## Tests

`tests/test_forward_open.rs`, `tests/test_forward_close.rs`: one test per packet, each a full
encapsulated packet for one connection (path `20 04 24 97 2C 96 2C 64`: Assembly, configuration
instance 0x97, connection points 0x96 and 0x64; 32-byte assemblies, a 1 s requested packet
interval, class 1, point-to-point, scheduled priority): the Forward_Open, Large_Forward_Open and
Forward_Close requests (written field by field and through `new_forward_open` /
`new_forward_close`, read back and parsed into the typed request), the Forward_Open and
Forward_Close success replies (read, parsed into the typed reply and written back), the rejected
Forward_Open reply (read, then checked as a `Rejection`: its service, general status and
Additional Status words, and `extended_status()`), and the path segment error a Teknic IO-HUB-4-E
returned for a 16-bit request path (captured; no reply data, a `Rejection` whose
`extended_status()` is `None`). `tests/common.rs` holds the values of that connection. The
dissection comments of these packets are generated with `scripts/dissect.sh`, which prints
Wireshark's (tshark's) dissection of a packet given as hex; a reply is dissected behind its request
(`--request`) because Wireshark only names the service of a reply once it has seen the request.
The devcontainer installs tshark.

`tests/test_general_status.rs` holds rejected Forward_Open replies with one and two Additional
Status words, read and written back unchanged. `tests/test_common_packet.rs` holds a Forward_Open
reply followed by a Sockaddr Info item (read and written back, and built, written and read back),
and, with the `adapter` feature, `read_request` accepting a request and rejecting a response.

### Class 1 I/O packets

`tests/test_io_packet.rs`: the two first packets of the connection the Forward_Open tests above open
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
