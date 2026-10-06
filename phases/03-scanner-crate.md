# Phase 3 — `scanner` crate: open a connection and exchange I/O

## Goal

Open a class 1 connection to a real adapter, exchange cyclic I/O with it and close it again, in code
a person can read top to bottom. The crate is the scanner side built on the library: production
code uses it as a reference and reuses the parts it needs (it is a workspace member, so another
crate can depend on it from this repository next to `eipscanne_rs`). The library (`eipscanne_rs`)
stays packet (de)serialization only.

## Layout

```text
scanner/src/session.rs     the encapsulation session over TCP 44818, shared by both kinds of messaging
scanner/src/explicit.rs    explicit (unconnected) messaging: send_request, decode_reply, read_identity
scanner/src/implicit.rs    implicit messaging: the stages below as submodules, re-exported flat
scanner/src/implicit/{config,open,produce,consume,close,udp}.rs
```

Explicit and implicit messaging are kept apart so it is clear which functions each one needs: the
`read-identity` and `write-clearlink-io` examples import `session` and `explicit`, the `implicit-io`
example imports `session` and `implicit`. The Forward_Open and Forward_Close are unconnected
messages, but they exist only to bracket an I/O connection, so they sit under `implicit`.

## The stages of implicit messaging

Each stage is one module, and each stage hands a plain struct to the next one:

```text
1. Session       TCP 44818: RegisterSession                       -> Session
2. Open          SendRRData(Forward_Open), read the reply          -> OpenConnection
3. Exchange      UDP 2222, two independent directions:
     3a. Produce   O->T: one packet every O->T interval (outputs)    Producer
     3b. Consume   T->O: screen, decode, watch the timeout (inputs)  Consumer
4. Close         SendRRData(Forward_Close), read the reply
5. End session   UnregisterSession
```

The sending and receiving directions share no state, so they are two types rather than one
connection object. Only `session`, `implicit::open`, `implicit::close`, `implicit::udp` and the
example's loop touch the network; `Producer` and `Consumer` take packets and timestamps in and
give packets and verdicts out.

## Scope

### Workspace

* The root `Cargo.toml` gains `[workspace] members = ["hex_test_macros", "scanner"]` and
  `default-members = [".", "scanner"]`, so `cargo run --example <name>`, `cargo test` and
  `cargo clippy` cover the library and the scanner from the root without `-p`;
  `hex_test_macros/Cargo.lock` goes away.
* New crate `scanner/` (package `scanner`, not published; the name is taken on crates.io, so
  publishing it would mean renaming), depending on `eipscanne_rs`, `tokio`, `binrw` and `bilge`.
* All examples move into the new crate (`scanner/examples/`): `read-identity`,
  `write-clearlink-io` and the new `implicit-io`. `examples/stream_utils.rs` and
  `examples/write-teknic-io/duplicated_stream_utils.rs` are deleted in favour of `session.rs`.
  The library then has no dependency on its utilities (no dev-dependency cycle), and `tokio` and
  `clap` leave its dev-dependencies. The example tests (`clearlink_config.rs`,
  `clearlink_output.rs`) move unchanged to `scanner/tests/clearlink/` and keep their bytes.
* Three explicit examples are ported from older branches: `write-nitra-io`, `clearlink-homing` and
  `io-hub-homing`. Every example is a single file that reads top to bottom and handles Ctrl+C. The
  device assemblies live outside the library in `scanner/assemblies/` (`clearlink`, `io_hub`,
  `nitra`), which an example includes with `#[path = "../assemblies"]`.

### `src/lib.rs`

Module docs: the shared session, then explicit versus implicit messaging and which examples use
which. `implicit.rs` lists the stages above with the submodule that implements each.

### Stage 1 and 5 — `src/session.rs` (shared)

* `Session { stream: TcpStream, session_handle: CipUdint, peer_ip: Ipv4Addr }`.
* `Session::register(address)`: connect, send RegisterSession, keep the handle from the reply.
  Fails when the adapter's address is not IPv4, which is all I/O connections support.
* `send(packet)`, `read_reply() -> EnIpPacket`: what the duplicated stream utils do today, except
  that a reply is read as the 24-byte encapsulation header followed by exactly `length` bytes,
  instead of a single read into a 500-byte buffer, and an encapsulation status other than success
  is an error.
* `peer_ip()`: the adapter's IP address, needed by stages 2 and 3b.
* `unregister(self)`: send UnregisterSession and drop the stream.
* A hack for Claude: with `EIP_DUMP` set, every packet the session sends or reads is printed to
  stderr as hex, ready for `scripts/dissect.sh`. This is how the traffic is seen where packets
  cannot be captured (the devcontainer).

### Explicit messaging — `src/explicit.rs`

* `send_request(&mut Session, path, service, data) -> EnIpPacket`: build the Message Router
  request, send it, read the reply, fail with `ExplicitError::Status { general_status,
  additional_status }` unless the general status is success.
* `decode_reply::<T>(&EnIpPacket) -> T`: the reply's data decoded as a caller-declared `binrw` type
  (what `read_typed_object_assembly` did in the old stream utils).
* `read_identity(&mut Session) -> IdentityResponse`: Get_Attributes_All on the Identity object.

### Connection settings — `src/implicit/config.rs`

* `ConnectionConfig`: everything the caller decides before opening, as plain fields:
  configuration instance, per direction (`o2t`, `t2o`) the connection point, application data
  size, requested packet interval, real-time format and fixed/variable size; transport class and
  trigger, priority, timeout multiplier, the T->O connection ID, the connection triad, and whether
  to use Large_Forward_Open.
* `to_forward_open_request() -> ForwardOpenRequest`: builds the request with the existing
  `connection_size()`, the bitfield builders and `CipPath::new_assembly_connection`.
* The real-time formats live here because the Forward_Open does not carry them, yet both
  directions need them to frame their data.
* Phase 4 (EDS parser) produces a `ConnectionConfig`.

### Stage 2 — `src/implicit/open.rs`

* `forward_open(&mut Session, ConnectionConfig) -> Result<OpenConnection, OpenError>`:
  sends `RequestObjectAssembly::new_forward_open`, reads the reply with
  `ConnectionManagerResponse::from_message_router_response`.
* `OpenError::Rejected { general_status, extended_status }` for an `Unsuccessful` reply (extended
  status from `ConnectionManagerExtendedStatus::from_additional_status`), plus I/O and parse
  errors. Rejections print the statuses in words (`the adapter rejected the Forward_Open: path
  segment error (0x04)`), and so do `CloseError` and `ExplicitError`.
* `OpenConnection { config, request, response, target_ip, o2t_endpoint }`: everything stages 3
  and 4 need, kept as the typed packets instead of copied fields.
* Where outputs are sent (`o2t_endpoint`): the O->T Sockaddr Info item of the reply if present
  (address `0.0.0.0` meaning the session's peer IP), otherwise the peer IP on
  `ETHERNET_IP_IO_UDP_PORT`.

### Stage 3a — `src/implicit/produce.rs`

* `Producer::new(&OpenConnection, initial_encapsulation_sequence_number)`: the caller picks the
  starting number (random in the example), which keeps the type deterministic and testable.
* `next_packet(&mut self, outputs, run: bool) -> Result<IoPacket, SizeError>`:
  * checks the output size against the O->T data size (equal when fixed, at most when variable);
  * increments the encapsulation sequence number on every packet;
  * increments the CIP sequence count only when the outputs differ from the previous packet's
    (a resend of unchanged data keeps the count);
  * adds the run/idle header when the O->T real-time format is the 32-bit header;
  * addresses the packet with the O->T connection ID from the reply.
* `next_packet_from(&T, run) -> Result<IoPacket, OutputsError>`: the same with the outputs given
  as a caller's `binrw` assembly (the struct the explicit side sends), encoded first.
* `period() -> Duration`: the O->T actual packet interval.

### Stage 3b — `src/implicit/consume.rs`

* `Consumer::new(&OpenConnection, established_at: Instant)`.
* `accept(&mut self, &IoPacket, from: SocketAddr, now: Instant) -> Result<Input, Discarded>`,
  checks in this order:
  1. the packet carries the T->O connection ID (`Discarded::UnknownConnection`);
  2. it comes from the session's peer IP; the port is not checked (`Discarded::WrongSender`);
  3. its encapsulation sequence number is newer than the last accepted one
     (`Discarded::StaleSequenceNumber`) and at most the allowed gap ahead of it
     (`Discarded::SequenceGapTooLarge`), compared modulo 2^32. The allowed gap is
     `max(16, timeout multiplier + 1)` with the multiplier as a factor (4 … 512, saturating). The
     first packet is accepted whatever its number;
  4. the data decodes as `IoData` with the T->O transport class and real-time format, and has the
     T->O size (`Discarded::Malformed`, `Discarded::WrongSize`).
* Only accepted packets move the timeout. `Input { data, run_idle, new_data }`: `new_data` is false
  when the CIP sequence count equals the previous accepted packet's.
* `Input::decode::<T>()`: the data decoded as a caller's `binrw` assembly, the implicit counterpart
  of `decode_reply`; every byte must belong to `T`.
* `deadline() -> Instant`: before the first accepted packet, `established_at` plus the larger of
  10 s and multiplier × T->O actual packet interval; afterwards, the last accepted packet plus
  multiplier × T->O actual packet interval. Computed in `u64` / `Duration` (512 × 10 s overflows
  `u32` microseconds).

### Stage 4 — `src/implicit/close.rs`

* `forward_close(&mut Session, &OpenConnection) -> Result<(), CloseError>`: a
  `ForwardCloseRequest` with the open request's connection triad, connection path, priority/time
  tick and timeout ticks, sent with `RequestObjectAssembly::new_forward_close`.

### UDP — `src/implicit/udp.rs`

* `bind_io_socket()`: `0.0.0.0:2222`, bound before the Forward_Open (adapters start producing as
  soon as they reply).
* `send_io_packet(&UdpSocket, &IoPacket, SocketAddrV4)`,
  `recv_io_packet(&UdpSocket) -> (IoPacket, SocketAddr)`.

### Example — `scanner/examples/implicit-io.rs`

`main` is the stages in order: register, bind UDP, `forward_open`, then one `tokio::select!` loop
over the O->T send timer, received packets and `consumer.deadline()`, printing input data, for a
given number of cycles; then `forward_close` and `unregister`. A comment notes that production code
may run the producer on its own task so a slow input handler cannot delay outputs. Flags (all three
examples use `clap`): host, configuration / output / input instances, sizes, RPI, cycle count,
`--large`. Defaults match the OpENer sample application (configuration 151, output 150, input
100, 32 bytes each).

### Documentation

`examples/README.md` moves with the examples and gains the implicit I/O walkthrough;
`tests/integration/README.md` gains the OpENer steps for `implicit-io`.

## Limits

* One connection per host: only one socket can bind UDP 2222, and the example opens one
  connection. Several connections would share the socket and route packets by connection ID.
* Point-to-point in both directions; no multicast.

## Tests

* Unit tests in `produce.rs` and `consume.rs` for every rule above: each discard reason, first
  packet with any sequence number, sequence number rollover, the allowed gap for small and large
  multipliers, CIP sequence count on unchanged and changed data, both timeout formulas; typed
  inputs and outputs (`Input::decode` with bytes left over, `next_packet_from` against the same
  bytes and with a wrong size).
* `config.rs`: the request built from the OpENer defaults has the bytes of the phase 2
  Forward_Open test.
* `open.rs`: endpoint resolution with no Sockaddr Info, with `0.0.0.0`, and with an address.
* `tests/loopback.rs`: the five stages against a fake adapter on the loopback interface
  (RegisterSession, Forward_Open with a Sockaddr Info O->T item, one I/O packet each way,
  Forward_Close, UnregisterSession), so the session framing, the open/close round trips and the
  UDP helpers are exercised without hardware.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test && cargo test -p eipscanne_rs --features adapter
```

(`cargo test` and `cargo clippy` cover both default members; `--workspace` adds only
`hex_test_macros`, which has no tests.)

Against the OpENer container from `tests/integration`: `implicit-io` opens the connection,
exchanges data for the requested number of cycles (OpENer echoes output assembly 150 into input
assembly 100), closes cleanly, and Wireshark dissects the traffic as Forward Open / Connected Data
Item / Forward Close. Not yet done: the devcontainer has no Docker client, so this run needs the
container started from the host (steps in `tests/integration/README.md`).
