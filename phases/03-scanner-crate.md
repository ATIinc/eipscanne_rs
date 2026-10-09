# Phase 3 — `scanner` crate: open a connection and exchange I/O

## Goal

Open a class 1 connection to a real adapter, exchange cyclic I/O with it and close it again, in code
a person can read top to bottom. The crate is the scanner side built on the library: production
code uses it as a reference and reuses the parts it needs (it is a workspace member, so another
crate in this repository can depend on it next to `eipscanne_rs`). The library (`eipscanne_rs`)
is packet (de)serialization only.

## Layout

```text
scanner/src/lib.rs                    module docs and four `pub mod` lines
scanner/src/error.rs                  Error and Result: what every fallible call returns
scanner/src/session.rs                the encapsulation session (TCP 44818), shared by both kinds
                                      of messaging
scanner/src/explicit.rs               explicit (unconnected) messaging: send_request,
                                      decode_reply, read_identity
scanner/src/implicit.rs               implicit messaging: module docs and three `pub mod` lines
scanner/src/implicit/connection.rs    OpenConnection, forward_open, forward_close
scanner/src/implicit/o2t.rs           the outputs: build_o2t_packet, send_io_packet
scanner/src/implicit/t2o.rs           the inputs: bind_io_socket, recv_io_packet,
                                      accept_t2o_packet, input_timeout, FIRST_PACKET_GRACE
```

There are no re-exports: a caller imports each item from the module that defines it
(`scanner::session::Session`, `scanner::implicit::t2o::accept_t2o_packet`,
`scanner::error::Error`).

Explicit and implicit messaging are kept apart so it is clear which functions each one needs: the
explicit examples import `session` and `explicit`, the `implicit-io` example imports `session` and
`implicit`. The Forward_Open and Forward_Close are unconnected messages, but they exist only to
bracket an I/O connection, so they sit under `implicit`.

## The stages of implicit messaging

```text
1. Session       TCP 44818: RegisterSession                        -> Session         session
2. Open          SendRRData(Forward_Open), read the reply          -> OpenConnection  connection
3. Exchange      UDP 2222, two independent directions:
     O->T          the outputs: one packet every O->T interval                        o2t
     T->O          the inputs: screen, decode, watch the timeout                      t2o
4. Close         SendRRData(Forward_Close), read the reply                            connection
5. End session   UnregisterSession                                                    session
```

`implicit.rs` holds stages 2 to 4 of this table in its module docs.

## Design

* A check that decides nothing does not exist. The T->O screen checks the connection ID, the
  sender's IP address, the sequence number and the data size, because each of them decides
  whether a packet is the next input of the connection; it does not check the sender's port
  (adapters choose it) or the sequence number of the first packet. The session checks the
  encapsulation status and that a reply answers the service sent.
* What is read off the wire and what is built on top of it are separate. `OpenConnection` holds
  the request the caller built (`request`), the reply as read (`response`), the real-time format
  of each direction (the one thing both ends agree on without the wire), the adapter's IP address
  and where the outputs go. An accepted input packet is returned as its Sequenced Address and its
  `CipIoData` as read.
* `OpenConnection` holds no value twice. Connection IDs, intervals, the timeout multiplier, the
  transport class and the connection sizes are read from `request` and `response` where they are
  used; data size and input timeout are functions of the connection, not fields.
* Stage 3 keeps no state. `build_o2t_packet` frames outputs and `accept_t2o_packet` screens an
  input packet; neither touches a socket. The caller's loop owns the encapsulation sequence
  numbers, the CIP sequence count and the input deadline. Only `session`, `forward_open`,
  `forward_close`, the three socket functions and the example's loop touch the network.

## Scope

### Workspace

* The root `Cargo.toml` declares `[workspace] members = ["hex_test_macros", "scanner"]` and
  `default-members = [".", "scanner"]`, so `cargo run --example <name>`, `cargo test` and
  `cargo clippy` cover the library and the scanner from the root without `-p`. The workspace has
  one `Cargo.lock`, at the root.
* `scanner/` is the package `scanner` with `publish = false` (the name is taken on crates.io). Its
  dependencies are `eipscanne_rs`, `tokio`, `binrw` and `bilge`, with no error-handling crate; its
  dev-dependencies are `clap` and `rand` (for the examples), `hex-test-macros` and
  `pretty_assertions`.
* Every example is in `scanner/examples/`: `read-identity`, `write-clearlink-io`,
  `write-nitra-io`, `clearlink-homing`, `io-hub-homing` and `implicit-io`. The library has no
  examples and no dev-dependency on the scanner, `tokio` or `clap`.
* Every example is a single file that reads top to bottom and parses its arguments with `clap`;
  the ones that wait or keep hardware moving (`implicit-io`, `write-nitra-io --pulse` and the two
  homing examples) handle Ctrl+C. The device assemblies live outside the library in
  `scanner/assemblies/` (`clearlink`, `io_hub`, `nitra`), which an example includes with
  `#[path = "../assemblies"]`.
* The ClearLink configuration and output assembly tests are in `scanner/tests/clearlink/`
  (`config.rs`, `output.rs`, with `main.rs` including the assemblies).

### `src/lib.rs`

Module docs: the shared session, explicit versus implicit messaging, the error type, and which
examples use which module. Then `pub mod error`, `explicit`, `implicit` and `session`.

### Errors — `src/error.rs`

* `Result<T, E = Error>`, returned by every fallible call of the crate, the socket functions
  included.
* `Error`:
  * `Io(std::io::Error)`, `Parse(binrw::Error)` (a packet or a caller's assembly did not encode
    or decode);
  * `EncapsulationStatus(EncapsStatusCode)`: an encapsulation status other than success;
  * `NotIpv4(IpAddr)`: the adapter's address is not IPv4, which is all I/O connections support;
  * `Rejected(Rejection)`: the adapter refused the request;
  * `NoResponse`: the reply carries no Message Router response;
  * `UnexpectedReply(String)`: the reply parsed but does not answer what was sent;
  * `UnexpectedPacket(String)`: an I/O packet that is not the next input of the connection; the
    caller reports it and waits for the next one;
  * `OutputSize { connection_size_type, data_size, actual }`: the outputs do not fit the
    connection.
* `Rejection` is the library's plain data (service, general status, Additional Status words).
  The hand-written `Display` writes the service and general status by their `Debug` names, then
  the Connection Manager extended status from `rejection.extended_status()` by name, or any other
  object's Additional Status words as hex.
* `std::error::Error` with `source()` for `Io` and `Parse`; `From` for `std::io::Error`,
  `binrw::Error` and `Rejection`. `Error` is `Send + Sync + 'static`, checked at compile time.

### Stage 1 and 5 — `src/session.rs` (shared)

* `Session { stream: TcpStream, session_handle: CipUdint, peer_ip: Ipv4Addr }`, fields private.
* `Session::register(address: impl ToSocketAddrs) -> Result<Session>`: connect, send
  RegisterSession, keep the handle from the reply. `Error::NotIpv4` when the adapter's address is
  not IPv4.
* `send(&EnIpPacket)`, `read_reply() -> Result<EnIpPacket>`: a reply is read as the 24-byte
  encapsulation header followed by exactly `length` bytes, and an encapsulation status other than
  success is `Error::EncapsulationStatus`.
* `request(&EnIpPacket) -> Result<EnIpPacket>`: `send`, `read_reply`, then `Error::NoResponse`
  without a Message Router response, `Error::UnexpectedReply` when the reply answers another
  service than the request, and `Error::Rejected` when `Rejection::from_response` finds a
  rejection. `explicit::send_request`, `forward_open` and `forward_close` all go through it.
* `session_handle()`, `peer_ip()`: the handle every packet carries, and the adapter's IP address.
* `unregister(self) -> Result<()>`: send UnregisterSession and drop the stream.
* A hack for Claude: with `EIP_DUMP` set, every packet the session sends or reads is printed to
  stderr as hex, ready for `scripts/dissect.sh`. This is how the traffic is seen where packets
  cannot be captured (the devcontainer).

### Explicit messaging — `src/explicit.rs`

* `send_request(&mut Session, request_path: CipPath, service: ServiceCode,
  data: Option<Box<dyn CipData>>) -> Result<EnIpPacket>`: builds the Message Router request with
  `RequestObjectAssembly::new_service_request` and sends it with `Session::request`.
* `decode_reply::<T>(&EnIpPacket) -> Result<T>`: the reply's data decoded as a caller-declared
  `binrw` type; `Error::NoResponse` without a Message Router response, `Error::UnexpectedReply`
  when the data is not raw bytes, `Error::Parse` when it does not decode.
* `read_identity(&mut Session) -> Result<IdentityResponse>`: Get_Attributes_All on the Identity
  object.

### Stages 2 and 4 — `src/implicit/connection.rs`

* `OpenConnection { request: ForwardOpenRequest, response: ForwardOpenResponse,
  o2t_real_time_format, t2o_real_time_format, target_ip: Ipv4Addr, o2t_endpoint: SocketAddrV4 }`:
  plain data with public fields and no methods. The transport class, timeout multiplier and
  connection parameters come from `request`; the connection IDs and actual packet intervals from
  `response`, since the request's connection IDs and requested packet intervals are proposals the
  adapter may replace. `target_ip` is the session's peer IP, where the inputs come from.
* `forward_open(&mut Session, ForwardOpenRequest, o2t_real_time_format, t2o_real_time_format)
  -> Result<OpenConnection>`: sends the caller's request with
  `RequestObjectAssembly::new_forward_open` through `Session::request` and decodes the reply with
  `decode_reply` into a `ForwardOpenResponse`. The width of the request's connection parameters
  makes it a Forward_Open or a Large_Forward_Open. The caller builds the request field by field
  (phase 4's EDS parser builds it from an EDS file). The real-time formats are passed next to it
  because the Forward_Open does not carry them, yet both directions need them to frame their data.
* `forward_close(&mut Session, &OpenConnection) -> Result<ForwardCloseResponse>`: a
  `ForwardCloseRequest` with the open request's priority/time tick, timeout ticks, connection
  triad and connection path, sent with `RequestObjectAssembly::new_forward_close` through
  `Session::request`, the reply decoded with `decode_reply`.
* `o2t_endpoint(reply, target_ip)` (private): where the outputs are sent. The O->T Socket Address
  Info item of the reply if present (address `0.0.0.0` meaning `target_ip`), otherwise `target_ip`
  on `ETHERNET_IP_IO_UDP_PORT`.
* `data_size(parameters, transport_class, real_time_format) -> (u16, ConnectionSizeType)`
  (crate-private): the connection size of a direction's parameter word minus
  `connection_size(0, transport class, real-time format)`, saturating at zero, with the size type.
  `data_len_matches_connection(data_len, data_size, size_type)` (crate-private): a fixed-size
  direction carries exactly `data_size` bytes, a variable-size one at most that many.
* `#[cfg(test)] test_support` (crate-private): `TARGET_IP`, the two connection IDs of the
  library's I/O packet tests, `standard_parameters(size, size_type)`, `sample_request()` (the
  phase 2 captured Forward_Open, built field by field) and `sample_connection()`, the
  `OpenConnection` the `o2t` and `t2o` tests build on.

### Stage 3, outputs — `src/implicit/o2t.rs`

* `build_o2t_packet(&OpenConnection, encapsulation_sequence_number: CipUdint,
  cip_sequence_count: CipUint, assembly_data: CipDataOpt, run: bool) -> Result<EnIpIoPacket>`:
  * writes the assembly data to bytes once (`CipDataOpt::Raw` bytes or a caller's `binrw` struct
    as `CipDataOpt::Typed`, whose size is only known once written), then checks their length
    against the O->T data size (`Error::OutputSize`);
  * adds the CIP sequence count for classes 1 to 3, and the run/idle header (run flag `run`) when
    the O->T real-time format is the 32-bit header;
  * addresses the packet with the O->T connection ID from the reply; its Connected Data Item is
    the `CipIoData` as `CipDataOpt::Typed`.
* `send_io_packet(&UdpSocket, &EnIpIoPacket, to: SocketAddrV4) -> Result<()>`.
* The numbering is the caller's: the encapsulation sequence number starts at a random number and
  moves on every packet; the CIP sequence count moves when the outputs change, so a resend of
  unchanged outputs keeps it. The caller sends one packet every
  `connection.response.o2t_actual_packet_interval`.

### Stage 3, inputs — `src/implicit/t2o.rs`

* `bind_io_socket() -> Result<UdpSocket>`: `0.0.0.0:2222`, bound before the Forward_Open
  (adapters start sending as soon as they reply). It carries both directions: the Forward_Open
  names no T->O address, so the adapter sends the inputs to the scanner's host on 2222.
* `recv_io_packet(&UdpSocket) -> Result<(EnIpIoPacket, SocketAddr)>`: one datagram parsed as an I/O
  packet, with its sender; cancel safe.
* `accept_t2o_packet(&OpenConnection, last_sequence_number: Option<CipUdint>, &EnIpIoPacket,
  from: SocketAddr) -> Result<(SequencedAddress, CipIoData)>`. A discarded packet is
  `Error::UnexpectedPacket` with one message per check, in this order:
  1. its Sequenced Address carries the T->O connection ID ("packet for another connection
     (0x…)");
  2. it comes from `target_ip`; the port is not checked ("packet from …, not the adapter");
  3. with a `last_sequence_number`, its encapsulation sequence number is newer, compared modulo
     2^32 ("stale sequence number … (last accepted …)"), and at most the allowed gap ahead
     ("sequence number … is more than … ahead of the last accepted …"). The allowed gap is
     `max(16, timeout multiplier + 1)`, with the multiplier as a factor (4 … 512, saturating).
     The first packet (`None`) is accepted whatever its number;
  4. its Connected Data Item is raw ("packet whose Connected Data Item is not raw") and its data, after
     the sequence count and run/idle header, has the T->O data size ("… input bytes, the
     connection carries …", the length being 0 when the item is shorter than those).

  The data is then decoded as `CipIoData` with the T->O transport class and real-time format; a
  failure there is `Error::Parse`. An accepted packet returns its Sequenced Address and its
  `CipIoData` as read, the run/idle header with all its bits included.
* Unchanged inputs: the caller compares `cip_sequence_count` with the previous accepted packet's.
  A caller's `binrw` input assembly is read from the raw data with `T::read_le`.
* `input_timeout(&OpenConnection) -> Duration`: timeout multiplier × T->O actual packet interval,
  computed in `u64` (512 × 10 s overflows `u32` microseconds).
* `FIRST_PACKET_GRACE` (10 s): the least time the adapter gets for its first packet.
* The deadline is the caller's: `established_at + FIRST_PACKET_GRACE.max(input_timeout)` until the
  first accepted packet, then the time of each accepted packet plus `input_timeout`. A discarded
  packet does not move it.

### Example — `scanner/examples/implicit-io.rs`

`main` is the five stages in order: register, bind the I/O socket, `forward_open`, then one
`tokio::select!` loop over the O->T send timer (`response.o2t_actual_packet_interval`), received
packets, the input deadline and Ctrl+C, for a given number of cycles; then `forward_close` and
`unregister`. Received inputs are printed, with "(unchanged)" when the CIP sequence count did not
move; a discarded packet, or a datagram that does not parse as an I/O packet, is printed with
its error and the loop goes on. The outputs differ every cycle, so the CIP
sequence count moves every cycle. `Args::forward_open_request()` builds every field of the
request; `network_connection_parameters(size)` picks the 16-bit or, with `--large`, the 32-bit
parameter word, and without `--large` fails with "a N-byte connection needs --large" when the size
does not fit 9 bits. The real-time formats are constants: `O2T_REAL_TIME_FORMAT = Header32Bit`,
`T2O_REAL_TIME_FORMAT = Modeless`. A comment notes that production code may send the outputs from
a task of its own so a slow input handler cannot delay them. Flags: host, configuration / output /
input instances, sizes, RPI, cycle count, `--large`. Defaults match the OpENer sample application
(configuration 151, output 150, input 100, 32 bytes each).

### Documentation

`scanner/examples/README.md` lists every example with its device, kind of messaging and the
hardware it moves, how the examples include the assemblies, and a walkthrough of each, the
implicit I/O one stage by stage. `tests/integration/README.md` has the OpENer steps for
`implicit-io`, and `tests/integration/start-opener.sh` runs them on the host: it creates the
`eip-network` network when missing, builds the `eip-adapter` image and runs the adapter at
`172.28.0.10` in the foreground.

## Limits

* One connection per host: only one socket can bind UDP 2222, and the example opens one
  connection. Several connections would share the socket and route packets by connection ID.
* Point-to-point in both directions; no multicast.

## Tests

* `connection.rs`: one test of endpoint resolution with no Socket Address Info, with `0.0.0.0`, and
  with an address next to a T->O item.
* `o2t.rs`: two tests. The first packet, raw or typed, has the bytes of the library's I/O packet
  test, and an idle packet differs only in the run flag. Fixed-size outputs, raw or typed, must
  match the data size; variable-size ones may be shorter.
* `t2o.rs`: four tests. Accepted inputs: the first packet with any sequence number from any
  sender port, variable-size data, a T->O run/idle header. The message of each discard by packet
  content: another connection, another host, data of the wrong size including an item shorter than the sequence count. Sequence numbers:
  repeated, older and back across the rollover are stale, forward across the rollover is
  accepted, and the allowed gap for small and large multipliers. `input_timeout` for the sample
  connection and without overflow for the largest multiplier and interval.
* `tests/loopback.rs`: the five stages against a fake adapter on the loopback interface
  (RegisterSession, Forward_Open with a Socket Address Info O->T item, one I/O packet each way,
  Forward_Close, UnregisterSession), on ephemeral ports, so the session framing, the open/close
  round trips, `send_io_packet`, `recv_io_packet` and `accept_t2o_packet` run without hardware.
* `tests/clearlink/`: the ClearLink configuration and output assembly tests.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test && cargo test --examples
```

(`cargo test` and `cargo clippy` cover both default members; `--workspace` adds only
`hex_test_macros`, which has no tests.)

Against the OpENer container from `tests/integration`: `implicit-io` opens the connection,
exchanges data for the requested number of cycles (OpENer echoes output assembly 150 into input
assembly 100), closes cleanly, and Wireshark dissects the traffic as Forward Open / Connected Data
Item / Forward Close. The devcontainer has no Docker client, so the container is started from the
host with `tests/integration/start-opener.sh`.
