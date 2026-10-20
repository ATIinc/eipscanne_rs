# Phase 4 — EDS parser crate, `eds-implicit-io` and device assemblies

## Goal

Read a device's EDS file into its params, assemblies and connections, and one connection into the
values of the Forward_Open that opens it, in the `eipscanne_rs` types, plus the two real-time
formats that phase 3's `scanner::implicit::connection::forward_open` takes. Each kind of entry is
a struct whose fields are the entry's fields in file order; an entry that does not fit fails
loudly, with its keyword and field number.

The assemblies of `scanner/assemblies/` are hand-written `binrw` structs, with per-motor groups,
named bits, enums such as `MoveType` and helpers such as `motor_input(n)`; the IO-HUB ones follow
the integer types of the hub's EDS.

## Scope

### Workspace

* Crate `eds_parser/` (package `eds_parser`, `publish = false`, `license = "MIT OR Apache-2.0"`),
  in the root `members` and `default-members`, so `cargo test`, `cargo clippy` and
  `cargo run --example <name>` work from the root.
* Crate `eds_parser_derive/` (proc-macro, in `members`): `#[derive(FromEntry)]` and
  `#[derive(FromField)]`, which Rust requires to live in a proc-macro crate of their own.
* Dependencies: `bilge`, `eds_parser_derive`, `parse_int`, `pest`, `pest_derive`, `unescaper`;
  no EtherNet/IP types. The `scanner` crate depends on `eds_parser` for `implicit::eds` (a
  comment there notes it may be worth a feature gate), and holds the two examples, the end-to-end
  test and its fixture.

### The file — `src/lib.rs`

`Eds::parse(&str)` reads every `ParamN` of `[Params]`, `AssemN` of `[Assembly]` and `ConnectionN`
of `[Connection Manager]` into `Eds { params, assemblies, connections }`, maps by N. Other sections
and keywords (`Object_Name`, `EnumN`, vendor keywords, `AssemExaN`) are not read. The same keyword
twice is an error.

`Eds` resolves a connection's references when the connection is opened: a `NumberOrParam` to its
number or the param's integer default, a path's `ParamN` to the default in as many little-endian
bytes as the param's data size, and an `AssemN` format to the assembly's size field or the sum of
its member sizes. A reference that is missing or has no usable value fails that connection.

`eds.requested_packet_interval`, `eds.data_size` and `eds.path_bytes` resolve a connection's
fields. Every error is an `error::Error` of one of three kinds, `Syntax`, `Entry` or `Reference`,
whose message says where: the keyword, field and line while reading
(`Connection1: field 4: line 3: Param999,x,Assem101,: expected a number or a ParamN, found "x"`),
the reference while resolving (`refers to Param7, which the file does not define`);
`error.context(place)` puts more of where in front (`O->T: ...`).

### Sections and entries — `src/eds.pest`, `src/syntax.rs`

A `pest` grammar splits the text into `Section { name, entries }` and `Entry`s; crate-internal. An
entry is one `Keyword = field, field, ...;` statement over any number of lines, and a field one of
its comma-separated values, kept as the text of its strings, `{ }` groups and bare words.

* `$` comments run to the end of the line; fields may be empty (`,,`, `1,;`).
* `,`, `;` and `$` inside a quoted string (which `\"` does not end), and `,` inside `{ }`, do not
  split.
* A syntax error shows where it is and what was expected (`` expected `,`, `;`, a string, a { }
  group, or a value ``).

`Entry::read::<T>()` reads the entry as a `T`, its fields in file order. A field past the end of a
short entry is empty, and a non-empty field past the last one `T` reads is an error.

### Reading fields — `src/fields.rs`

* `FromEntry`: a struct read from consecutive fields, each member by its own `FromEntry` in the
  order the struct declares them; `#[derive(FromEntry)]` implements it. A member holding one
  field's value reads the next field, and a `Vec` reads the rest of the entry up to the last
  non-empty field, one element after another (the `16,Param1, 8,` members of an `AssemN`).
* `FromField`: a value read from one field's text. An `Option` is `None` for an empty field;
  `u8`, `u16` and `u32` read decimal, `0x` or `0b`; a `String` joins adjacent quoted strings
  (`"AAA" "BBB"`, `L"..."` alike), reads escapes with `unescaper` (a malformed one stays as
  written) and an empty field as `""`; `Unread` takes a field this crate does not use.
  `#[derive(FromField)]` implements it for a `bilge` bitfield or enum, read as its backing integer
  (`Bitsized::ArbitraryInt`).

### Field values — `src/values.rs`

The kinds of value a field holds beyond strings and integers, each a `FromField`:

* `NumberOrParam`: `Number(i64)` or `Param(u32)`; `DataFormat`: `Assem`, `AssemExa` or `Param`;
* `ParamValue`: `Integer`, `Real` or `Text`, a param's minimum, maximum or default;
* `Path`: hex bytes and `[ParamN]` or `ParamN` references (`"20 04 24 [Param3] 2C 65"`);
* `MemberReference`: `Param`, `Assem`, `Number` or `Path`.

### Entry structs — `src/params.rs`, `src/assembly.rs`, `src/connection_manager.rs`

One module per section. `Param` (24 members), `Assembly` (6, then `members`, the remaining pairs
as `AssemblyMember { size_bits, reference }`) and `Connection` (17) derive `FromEntry` and list
every field of their entry in order, named after the field; the fields this crate does not use are
private `Unread` members. Each struct's doc comment shows an example entry.

`ParamDescriptor` (field 4 of a `Param`, 16 bits: `0x0004` supports scaling, `0x0010` read only)
and `CipDataType` (field 5, the elementary data type codes `0xC1` BOOL to `0xDE` STRINGI, `0xC8`
is `Udint`, any other code `Unknown(code)`) are `bilge` types deriving `FromField`. So are the two
mask words of a `Connection`, `TriggerAndTransport` and `ConnectionParameters`, their bits grouped
as the EDS comments group them (`TransportClasses`, `Triggers`, `ApplicationTypes`; per direction
`SizeTypes`, `RealTimeTransferFormat`, `ConnectionTypes`, `Priorities`), so their `Debug` names
every bit. `RealTimeTransferFormat` codes 2 and 5 to 7 are `Reserved(code)`.

### The connection — `scanner/src/implicit/eds.rs`

`forward_open_request(&eds, connection, large)` is the `ForwardOpenRequest` for a `Connection`
with the fields the EDS decides: both requested packet intervals, both network connection
parameters, the transport class and trigger, and the connection path. Every other field is zero,
for the caller to set around `..forward_open_request(&eds, connection, large)?`: tick time and timeout ticks of the
Forward_Open itself, timeout multiplier, connection IDs (`o2t_network_connection_id` 0: the target
chooses it) and the connection triad. `large` is `Some(true)` for a Large_Forward_Open (32-bit
connection parameters), `Some(false)` for a Forward_Open (16-bit), `None` for a Large_Forward_Open
only when a direction's connection size is over 511 bytes.

`o2t_real_time_format(connection)` and `t2o_real_time_format(connection)` are the two real-time
formats `forward_open` takes, and `first_openable_connection(&eds, large)` the N of the first
connection this scanner can open. A caller finds `ConnectionN` in `eds.connections`, and its bytes
of output data with `eds.data_size(connection.o2t_size, connection.o2t_format)`.

Each value is picked from the `Connection`. An EDS reference that does not resolve is
`scanner::error::Error::Eds`; a connection this scanner does not open is
`Error::UnsupportedConnection`, and a refusal about a mask prints the mask with `Debug`, which
shows what the connection offers:

| Value | From the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, `Direction::Client` | class 1, cyclic or exclusive owner not supported (input-only and listen-only are out of scope) |
| `*_network_connection_parameters` | point-to-point, exclusive owner; priority: scheduled, else high, else low, among those both directions support; fixed size if supported, else variable; the data size plus `connection_size`'s overhead for class 1 and the direction's real-time format | not point-to-point, no shared priority, over 511 bytes with `Some(false)` |
| `*_requested_packet_interval` | a number or a `ParamN`'s default, in microseconds | none in the EDS |
| data size (`eds.data_size`) | a number or a `ParamN`'s default; when empty, the size of the format's `AssemN` | neither |
| `*_real_time_format` | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5–7 |
| `connection_path` | the path, `[ParamN]` replaced by the param's default, read as a `CipPath` | segments other than logical ones |

The client/server bit of the trigger and transport mask is ignored: vendors disagree on it, and
the originator of a Forward_Open is always the client. When `first_openable_connection` finds no
connection it can open, its error lists each connection's refusal (`Connection2: ...`).

### Example — `scanner/examples/eds-implicit-io.rs`

The stages of `implicit-io`, with the connection settings taken from an EDS, not from flags. Its
defaults describe the OpENer adapter of `tests/integration`: `--eds` is the `sample_adapter.eds`
fixture (`scanner/tests/fixtures`, by its path under `CARGO_MANIFEST_DIR`) and `--host` is
`172.28.0.10`, so
`cargo run --example eds-implicit-io` runs against that container. The module docs give the
commands that start the adapter and what the run prints.

1. Read the file and its connection (`--connection 1` for `Connection1`; default: the first
   connection this scanner can open; `--large true` or `--large false` to choose the Forward_Open
   service, automatic otherwise).
2. Write the `ForwardOpenRequest` from the originator constants of `implicit-io` and
   `..forward_open_request(&eds, connection, large)?`, then print the connection's N and name
   (`Connection1 (Exclusive Owner) from <file>`) and the whole request (`{request:#?}`).
3. Register, `bind_io_socket`, `forward_open` with the request and the two real-time formats, the
   cyclic loop, `forward_close`, `unregister`. Each send tick builds the outputs with
   `build_o2t_packet` and sends them with `send_io_packet`; each received packet goes through
   `recv_io_packet` and `accept_t2o_packet`, and an `Error::UnexpectedPacket`, or an
   `Error::Parse` for a datagram that is not an I/O packet, is printed as `DISCARDED` and the loop
   goes on; the deadline is `input_timeout` after the last accepted packet (at least
   `FIRST_PACKET_GRACE` after the Forward_Open reply for the first). The outputs never change, so
   the CIP sequence count stays at 1.
4. Outputs are zeros of the O->T size. The scanner stays **idle** (run flag cleared in the 32-bit
   header) unless `--run` is given: an output assembly may drive motors. Without a 32-bit header on
   O->T, the example refuses to start without `--run`, since idle cannot be signalled.
5. The exchange ends after `--cycles` output packets, on the input timeout or on Ctrl+C; the
   connection is closed and the session unregistered in all three cases.

### Example — `scanner/examples/io-hub-implicit.rs`

The implicit counterpart of `io-hub-explicit-homing`, one linear file, with the IO-HUB assemblies
included through `#[path = "../assemblies"]` and errors returned as
`Box<dyn std::error::Error>`:

1. Read the first connection this scanner can open from `--eds` (the local IO-HUB file), and
   check that its request's path is `CipPath::new_assembly_connection(1, OUTPUT_ASSEMBLY_INSTANCE,
   INPUT_ASSEMBLY_INSTANCE)`, the assemblies the structs are written for.
2. Write the `ForwardOpenRequest` from the originator constants of `implicit-io` and
   `..forward_open_request(&eds, connection, None)?`.
3. Register, `bind_io_socket`, `forward_open`; each cycle send
   `CipDataOpt::Typed(Box::new(OutputAssemblyHub4E::default()))` through `build_o2t_packet` and
   `send_io_packet` (idle unless `--run`; nothing is enabled either way; the outputs never change,
   so the CIP sequence count stays at 1). Each packet `accept_t2o_packet` accepts is read with
   `InputAssemblyHub4E::read_le`, and a status line for the chosen motor (`--motor`) is printed
   when the printed values change; a packet it discards (`Error::UnexpectedPacket`), and a
   datagram that does not parse as an I/O packet, is printed as `DISCARDED`. After `--cycles`
   packets, on the input timeout or on Ctrl+C, close and unregister.

`--host` has no default.

### IO-HUB assemblies follow the EDS

The hand-written IO-HUB structs use the EDS's integer types. The ClearPath-IP Software Reference
linked at the top of each file is their source, but where it and the EDS disagree the EDS is
followed and the doc comment says the documentation may be wrong:

| Field | EDS | Software Reference | Struct |
|---|---|---|---|
| Analog inputs I/O-0…12, analog output I/O-12 | UINT | INT (tags) | `CipUint`, with the callout |
| Read/Write Parameter ID | INT | INT | `CipInt` |
| Read Parameter ID Echo, Move Number, Move Number Ack | INT | no type | `CipInt` |

### Documentation

* `eds_parser/README.md`: the modules, `Eds::parse` and the reference functions.
* `scanner/examples/README.md`: the `ForwardOpenRequest` a caller writes with `implicit::eds`, what
  is accepted and refused, how to run `eds-implicit-io` and `io-hub-implicit`, and how to write a
  device's assemblies.
* The top-level README lists the crate and its examples; `tests/integration/README.md` describes
  the `eds-implicit-io` run against OpENer.
* `phases/README.md` lists phase 4's pull request.

## Limits

* Class 1 cyclic, exclusive owner, point-to-point only; one connection per run.
* Configuration data is never read or sent; of the file, only `ParamN`, `AssemN` and
  `ConnectionN` entries are read.
* Paths of logical segments only; `SYMBOL_ANSI` in a path is refused.
* `{ }` groups are kept as one field's text but never read.

## Tests

* Fixture `scanner/tests/fixtures/sample_adapter.eds`, written for this repository (no
  third-party file): the connection the OpENer container accepts (configuration 151, O->T 150, T->O 100,
  32 bytes each, 32-bit header O->T, modeless T->O, scheduled priority, RPI param defaulting to
  1 s) plus an input-only connection. It also covers CRLF line endings, a joined multi-line
  `IconContents`, a `[TCP/IP Interface Class]` section, dates and times, an empty size resolved
  through `AssemN`, an RPI from `ParamN`, a `[ParamN]` in the path, configuration data to be
  ignored.
* End to end (`scanner/tests/sample_adapter.rs`): the fixture's `Connection1`, with the originator of the
  library's Forward_Open test, builds the same Forward_Open bytes as that test.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test
```

Against the OpENer container from `tests/integration`, `cargo run --example eds-implicit-io` opens
the connection, exchanges data for the requested number of cycles, and closes cleanly. Against an
IO-HUB-4-E, `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip>`
does the same with I/O every 10 ms.
