# Phase 2 — Connection Manager packets

## Goal

Typed `binrw` + `bilge` packets for the Connection Manager services used to open and close a class 1
connection, sent through the existing explicit-messaging path (`SendRRData` to class 6, instance 1).

## Scope

New module `src/cip/connection_manager.rs` with its files in `src/cip/connection_manager/`:

* `parameters.rs` — everything the open services share:
  * `StandardNetworkConnectionParameters` (16-bit: connection size, fixed/variable, priority,
    connection type, redundant owner) and `LargeNetworkConnectionParameters` (32-bit: 16-bit size,
    9 reserved bits, then the same upper fields), with a named enum for every field
    (`ConnectionSizeType`, `ConnectionPriority`, `ConnectionType`, `RedundantOwner`); a caller
    assembles the word field by field with the builder;
  * `NetworkConnectionParameters`, a two-case `binrw` enum (`Standard` / `Large`) read with a
    `large: bool` argument, like `PathData::FormatAsU8/FormatAsU16`; one request struct serves both
    services instead of a generic parameter;
  * the free function `connection_size()` adds the 16-bit sequence count (transport classes 1, 2
    and 3) and the real-time header length to the application data size, the one piece of
    arithmetic Wireshark does not show; whether the result fits the nine bits of a Forward_Open is
    the caller's check (`u9::try_new`);
  * `TransportTypeTrigger` (`TransportClass`, `ProductionTrigger`, `Direction`);
  * `PriorityTimeTick` (tick time, priority) and `ConnectionTimeoutMultiplier` (x4 … x512,
    `Unknown` for reserved values, `multiplier()`);
  * `RealTimeFormat` (modeless, zero length, heartbeat, 32-bit header) with `header_len()`. The
    run/idle header itself belongs to the I/O packets of phase 3.
* `shared.rs` — the blocks every service carries in the same layout: `ConnectionTriad`
  (connection serial number, originator vendor ID, originator serial number; in every request and
  reply, and what a Forward_Close is matched against) and `UnsuccessfulResponse`, the reply data
  of any rejected request: the triad plus the remaining path size and reserved byte that only a
  routing error carries (optional fields, read when the bytes are there).
* `forward_open.rs` — `ForwardOpenRequest` (one struct for 0x54 and 0x5B, built field by field
  like every other packet; `service_code()` picks the service from the width of the O->T
  parameters, so both directions use the same case; the Connection Path Size byte is derived from
  the path on write) and `ForwardOpenResponse` (connection IDs, triad, actual packet intervals,
  application reply size, reserved byte and application reply data, flat as Wireshark shows them).
  For point-to-point the originator chooses the T->O connection ID; the O->T ID comes back in the
  reply.
* `forward_close.rs` — `ForwardCloseRequest` (size byte, then a reserved byte, then the path) and
  `ForwardCloseResponse` (triad, application reply size, reserved byte, application reply data).
* `response.rs` — the reply side: `ConnectionManagerResponse`, a `binrw` enum read with the service
  and general status of the enclosing Message Router response (`ForwardOpen` / `ForwardClose` on
  success, `Unsuccessful` otherwise), the way `CommandSpecificData` branches on the header command;
  `ConnectionManagerExtendedStatus` (a `binrw` magic enum of every Connection Manager extended
  status code + `Unknown`, like `ResponseStatusCode`) with `from_additional_status`, which reads
  the first Additional Status word of a Message Router response through `binrw`; and
  `ConnectionManagerResponse::from_message_router_response` (`binrw::BinResult`), which writes the
  reply data of the Message Router response (raw or typed) through the `BinWrite` impl of
  `CipDataOpt`, then reads the typed reply from those bytes; the reply to any other service is a
  `binrw` assertion error. A rejected request is not an error: it parses as `Ok(Unsuccessful(..))`,
  and its general status and Additional Status words stay on the Message Router response.
* `src/object_assembly.rs` — `RequestObjectAssembly::new_forward_open` (sends Forward_Open or
  Large_Forward_Open depending on the request) and `new_forward_close`, both addressed to the
  Connection Manager through the `CONNECTION_MANAGER_CLASS_ID` / `CONNECTION_MANAGER_INSTANCE_ID`
  constants of `src/cip/object_ids.rs`.
* The `CipData` blanket impl (`src/cip/message/data.rs`) no longer requires `BinRead` with empty
  arguments: `write_to` only writes, and `ForwardOpenRequest` reads with a `large` argument. Every
  type that satisfied the bound before still does.
* The `async` feature is gone: `CipData` always requires `Send + Sync` (what `async` used to
  add), so packets carrying typed data can be held across `.await` points without opting in.
  `adapter` is the only feature left.

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
encapsulated packet using the EIPScanner example parameters (path `20 04 24 97 2C 96 2C 64`, 32-byte
assemblies, a 1 s requested packet interval, class 1, point-to-point, scheduled priority): the
Forward_Open, Large_Forward_Open and Forward_Close requests (written, read back and parsed into the
typed request), the Forward_Open and Forward_Close success replies (read, parsed into the typed
reply and written back) and the rejected Forward_Open reply (read and parsed into `Unsuccessful`,
its general and extended status read from the Message Router response). The dissection comment of
every packet is generated with
`scripts/dissect.sh` (tshark), a reply behind its request (`--request`) because Wireshark only names
the service of a reply once it has seen the request.

## Verification

```
cargo fmt --check && cargo clippy --all-targets && cargo test --all && cargo test --all --features adapter && cargo test --examples
```
