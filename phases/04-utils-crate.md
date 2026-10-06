# Phase 4 — `eipscanne_utils` crate: open a connection and exchange I/O

**Status:** Implemented, awaiting review (verified on loopback against a fake adapter; the OpENer run is still to do, see Verification)

## Goal

Open a class 1 connection to a real adapter, exchange cyclic I/O with it and close it again, in code
a person can read top to bottom. The crate is scaffolding: production code uses it as a reference
and reuses the parts it needs. The library (`eipscanne_rs`) stays packet (de)serialization only.

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
connection object. Only `session`, `open`, `close` and the example's loop touch the network;
`Producer` and `Consumer` take packets and timestamps in and give packets and verdicts out.

## Scope

### Workspace

* The root `Cargo.toml` gains `[workspace] members = ["hex_test_macros", "eipscanne_utils"]`;
  `hex_test_macros/Cargo.lock` goes away.
* New crate `eipscanne_utils/` (package `eipscanne-utils`, not published), depending on
  `eipscanne_rs`, `tokio` and `binrw`.
* All examples move into the new crate (`eipscanne_utils/examples/`): `read-identity`,
  `write-teknic-io` and the new `implicit-io`. `examples/stream_utils.rs` and
  `examples/write-teknic-io/duplicated_stream_utils.rs` are deleted in favour of `session.rs`.
  The library then has no dependency on its utilities (no dev-dependency cycle), and `tokio` and
  `clap` leave its dev-dependencies. The example tests (`clearlink_config.rs`,
  `clearlink_output.rs`) move unchanged and keep their bytes.

### `src/lib.rs`

Module docs listing the five stages above, in order, with the module that implements each.

### Stage 1 and 5 — `src/session.rs`

* `Session { stream: TcpStream, session_handle: CipUdint }`.
* `Session::register(address)`: connect, send RegisterSession, keep the handle from the reply.
* `send(packet)`, `read_reply() -> EnIpPacket`, `read_typed_reply::<T>()`: what the duplicated
  stream utils do today, except that a reply is read as the 24-byte encapsulation header followed
  by exactly `length` bytes, instead of a single read into a 500-byte buffer.
* `peer_ip()`: the adapter's IP address, needed by stages 2 and 3b.
* `unregister(self)`: send UnregisterSession and drop the stream.

### Connection settings — `src/config.rs`

* `ConnectionConfig`: everything the caller decides before opening, as plain fields:
  configuration instance, per direction (`o2t`, `t2o`) the connection point, application data
  size, requested packet interval, real-time format and fixed/variable size; transport class and
  trigger, priority, timeout multiplier, the T->O connection ID, the connection triad, and whether
  to use Large_Forward_Open.
* `to_forward_open_request() -> ForwardOpenRequest`: builds the request with the existing
  `connection_size()`, the bitfield builders and `CipPath::new_assembly_connection`.
* The real-time formats live here because the Forward_Open does not carry them, yet both
  directions need them to frame their data.
* Phase 5 (EDS parser) produces a `ConnectionConfig`.

### Stage 2 — `src/open.rs`

* `forward_open(&mut Session, ConnectionConfig) -> Result<OpenConnection, OpenError>`:
  sends `RequestObjectAssembly::new_forward_open`, reads the reply with
  `ConnectionManagerResponse::from_message_router_response`.
* `OpenError::Rejected { general_status, extended_status }` for an `Unsuccessful` reply (extended
  status from `ConnectionManagerExtendedStatus::from_additional_status`), plus I/O and parse
  errors.
* `OpenConnection { config, request, response, target_ip, o2t_endpoint }`: everything stages 3
  and 4 need, kept as the typed packets instead of copied fields.
* Where outputs are sent (`o2t_endpoint`): the O->T Sockaddr Info item of the reply if present
  (address `0.0.0.0` meaning the session's peer IP), otherwise the peer IP on
  `ETHERNET_IP_IO_UDP_PORT`.

### Stage 3a — `src/produce.rs`

* `Producer::new(&OpenConnection, initial_encapsulation_sequence_number)`: the caller picks the
  starting number (random in the example), which keeps the type deterministic and testable.
* `next_packet(&mut self, outputs, run: bool) -> Result<IoPacket, SizeError>`:
  * checks the output size against the O->T data size (equal when fixed, at most when variable);
  * increments the encapsulation sequence number on every packet;
  * increments the CIP sequence count only when the outputs differ from the previous packet's
    (a resend of unchanged data keeps the count);
  * adds the run/idle header when the O->T real-time format is the 32-bit header;
  * addresses the packet with the O->T connection ID from the reply.
* `period() -> Duration`: the O->T actual packet interval.

### Stage 3b — `src/consume.rs`

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
* `deadline() -> Instant`: before the first accepted packet, `established_at` plus the larger of
  10 s and multiplier × T->O actual packet interval; afterwards, the last accepted packet plus
  multiplier × T->O actual packet interval. Computed in `u64` / `Duration` (512 × 10 s overflows
  `u32` microseconds).

### Stage 4 — `src/close.rs`

* `forward_close(&mut Session, &OpenConnection) -> Result<(), CloseError>`: a
  `ForwardCloseRequest` with the open request's connection triad, connection path, priority/time
  tick and timeout ticks, sent with `RequestObjectAssembly::new_forward_close`.

### UDP — `src/udp.rs`

* `bind_io_socket()`: `0.0.0.0:2222`, bound before the Forward_Open (adapters start producing as
  soon as they reply).
* `send_io_packet(&UdpSocket, &IoPacket, SocketAddrV4)`,
  `recv_io_packet(&UdpSocket) -> (IoPacket, SocketAddr)`.

### Example — `eipscanne_utils/examples/implicit-io/`

`main` is the stages in order: register, bind UDP, `forward_open`, then one `tokio::select!` loop
over the O->T send timer, received packets and `consumer.deadline()`, printing input data, for a
given number of cycles; then `forward_close` and `unregister`. A comment notes that production code
may run the producer on its own task so a slow input handler cannot delay outputs. Flags: host,
configuration / output / input instances, sizes, RPI, cycle count, `--large`. Defaults match the
OpENer sample application (configuration 151, output 150, input 100, 32 bytes each).

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
  multipliers, CIP sequence count on unchanged and changed data, both timeout formulas.
* `config.rs`: the request built from the OpENer defaults has the bytes of the phase 2
  Forward_Open test.
* `open.rs`: endpoint resolution with no Sockaddr Info, with `0.0.0.0`, and with an address.
* `tests/loopback.rs`: the five stages against a fake adapter on the loopback interface
  (RegisterSession, Forward_Open with a Sockaddr Info O->T item, one I/O packet each way,
  Forward_Close, UnregisterSession), so the session framing, the open/close round trips and the
  UDP helpers are exercised without hardware.

## Verification

```
cargo fmt --all --check && cargo clippy --workspace --all-targets && cargo test --workspace && cargo test -p eipscanne_rs --features adapter
```

Against the OpENer container from `tests/integration`: `implicit-io` opens the connection,
exchanges data for the requested number of cycles (OpENer echoes output assembly 150 into input
assembly 100), closes cleanly, and Wireshark dissects the traffic as Forward Open / Connected Data
Item / Forward Close. Not yet done: the devcontainer has no Docker client, so this run needs the
container started from the host (steps in `tests/integration/README.md`).
