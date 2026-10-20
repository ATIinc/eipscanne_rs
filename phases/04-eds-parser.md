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
* Dependencies: `bilge`, `binrw`, `eipscanne_rs`, `serde` (derive), `serde_with` (macros only).
  Dev-dependencies: `scanner`, `tokio`, `clap`, `rand`, `anyhow` for the examples, and
  `hex-test-macros` for the tests.

### The file — `src/lib.rs`

`Eds::parse(&str)` reads every `ParamN` of `[Params]`, `AssemN` of `[Assembly]` and `ConnectionN`
of `[Connection Manager]` into `Eds { params, assemblies, connections }`, maps by N. Other sections
and keywords (`Object_Name`, `EnumN`, vendor keywords, `AssemExaN`) are not read. The same keyword
twice is an error.

`Eds` resolves a connection's references when the connection is opened: a `NumberOrParam` to its
number or the param's integer default, a path's `ParamN` to the default in as many little-endian
bytes as the param's data size, and an `AssemN` format to the assembly's size field or the sum of
its member sizes. A reference that is missing or has no usable value fails that connection.

### Field values — `src/field_values.rs`

The kinds of value a field holds, each read from the field's text by its `FromStr`, the only
place field text is parsed besides plain integers:

* `QuotedString`: a quoted string, adjacent quoted strings joined (`"AAA" "BBB"`), `L"..."` read
  the same way, `\n`, `\t`, `\r` and `\<other character>` unescaped;
* `NumberOrParam`: `Number(i64)` or `Param(u32)`; `DataFormat`: `Assem`, `AssemExa` or `Param`;
* `ParamValue`: `Integer`, `Real` or `Text`, a param's minimum, maximum or default;
* `PathTokens`: hex bytes and `[ParamN]` or `ParamN` references (`"20 04 24 [Param3] 2C 65"`);
* `MemberReference`: `Param`, `Assem`, `Number` or `Path`.

`number` reads decimal, `0x` hex and `0b` binary; `numbered` reads the N of `ParamN` and the like.

### Entry structs — `src/entries.rs`

`ParamEntry` (24 fields), `AssemblyEntry` (6 fields, then `members`, the remaining pairs as
`AssemblyMember { size_bits, reference }`) and `ConnectionEntry` (17 fields) list every field of
their entry in order, named after the field; the fields this crate does not use are private
`IgnoredAny`s. The maps of `Eds` hold them by their N. Each struct's doc comment shows an example entry.
They derive `serde::Deserialize`, and `file` implements `serde::Deserializer` for an `Entry`, so
`ConnectionEntry::deserialize(&entry)` reads one:

* an integer field (`u8`, `u16`, `u32`) from decimal, `0x` or `0b` text, range-checked;
* an `Option` field as `None` when the field is empty;
* a `Vec` field from the rest of the entry's fields up to the last non-empty one, each element
  taking one field, or as many as it has fields when it is a struct;
* any other field through its type's `FromStr` (`serde_with::DeserializeFromStr`);
* a field past the end of a short entry as empty; a non-empty field past the struct's last field
  is an error.

### Errors — `src/error.rs`

One `Error` enum: `Syntax`, `Value` (a field's text), `Reference`, `Refused`, `DefinedTwice`,
`ExtraField`, `NoSuchConnection`, `NoConnection` (every refusal, by N), `InEntry`, which puts any
of them in an entry and field, and `InConnection`, which puts one in `ConnectionN`. Every error says where: `Connection1: field 4: expected a number or a
ParamN, found "x"`, `Connection2: this scanner opens class 1, cyclic, exclusive-owner connections;
...`. The `FromStr` types return `Error::Value`.

### Bits and codes — `src/masks.rs`

`TriggerAndTransport` and `ConnectionParameters`: fields 1 and 2 of a `ConnectionEntry` as 32-bit
`bilge` bitfields read from a number (`#[serde(from = "u32")]`), so each bit is named once and
their `Debug` names every bit. `ConnectionParameters::shared_priority` and `real_time_format` (a
3-bit code to `RealTimeFormat`) pick values from them.

`ParamDescriptor`: field 4 of a `ParamEntry` as a 16-bit bitfield (`0x0004` supports scaling,
`0x0010` read only). `CipDataType`: field 5 as an 8-bit `bilge` enum of the elementary data type
codes, `0xC1` BOOL to `0xDE` STRINGI (`0xC8` is `Udint`), any other code `Unknown(code)`. Units
(field 8) are free text, a `QuotedString`.

### The connection — `src/connection.rs`

`eds.all_connections(large)` maps every N to its `Result<Connection>`: the connection, or why this
scanner cannot open it. `eds.connection(number, large)` is the one for `ConnectionN`, and
`eds.first_connection(large)` the first one this scanner can open, with its N. `large` is `Some(true)` for a Large_Forward_Open (32-bit connection parameters), `Some(false)` for
a Forward_Open (16-bit), `None` for a Large_Forward_Open only when a direction's connection size
is over 511 bytes.

`Connection { name, transport_type_trigger, o2t_requested_packet_interval,
o2t_network_connection_parameters, o2t_real_time_format, o2t_size, t2o_..., connection_path }`
holds the values in the library's types, named after the `ForwardOpenRequest` fields they fill;
`o2t_size` and `t2o_size` are the application data in bytes. The caller writes the
`ForwardOpenRequest` from them, with what an EDS does not describe: tick time and timeout ticks of
the Forward_Open itself, timeout multiplier, connection IDs (`o2t_network_connection_id` 0: the
target chooses it) and the connection triad.

Each value is picked from the `ConnectionEntry`, or the connection is refused with an `Error`;
a refusal about a mask prints the mask with `Debug`, which shows what the connection offers:

| Value | From the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, `Direction::Client` | class 1, cyclic or exclusive owner not supported (input-only and listen-only are out of scope) |
| `*_network_connection_parameters` | point-to-point, exclusive owner; priority: scheduled, else high, else low, among those both directions support; fixed size if supported, else variable; the size plus `connection_size`'s overhead for class 1 and the direction's real-time format | not point-to-point, no shared priority, over 511 bytes with `Some(false)` |
| `*_requested_packet_interval` | a number or a `ParamN`'s default, in microseconds | none in the EDS |
| `*_size` | a number or a `ParamN`'s default; when empty, the size of the format's `AssemN` | neither |
| `*_real_time_format` | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5–7 |
| `connection_path` | the path read as a `CipPath` | segments other than logical ones |

The client/server bit of the trigger and transport mask is ignored: vendors disagree on it, and
the originator of a Forward_Open is always the client. When `first_connection` finds no connection
it can open, its error lists each connection's refusal.

### Sections and entries — `src/file.rs`

`file::sections` splits the text into `Section { name, entries }` and `Entry { keyword, fields }`,
each field kept as its trimmed raw text; crate-internal. The same file holds the entry's
`Deserializer`.

* `$` comments run to the end of the line. An entry spans any number of lines and ends with `;`;
  its fields are separated by `,` and may be empty (`,,`, `1,;`).
* `,`, `;` and `$` inside a quoted string (which `\"` does not end), and `,` inside `{ }`, do not
  split.
* A missing `;` or `=`, an unclosed `[` and an entry before the first section are errors quoting
  where they are (`K 1: the entry has no =`).

### Example — `eds_parser/examples/eds-implicit-io.rs`

The stages of `implicit-io`, with the connection settings taken from an EDS, not from flags. Its
defaults describe the OpENer adapter of `tests/integration`: `--eds` is the `sample_adapter.eds`
fixture (by its path under `CARGO_MANIFEST_DIR`) and `--host` is `172.28.0.10`, so
`cargo run --example eds-implicit-io` runs against that container. The module docs give the
commands that start the adapter and what the run prints.

1. Read the file and its connection (`--connection 1` for `Connection1`; default: the first
   connection this scanner can open; `--large true` or `--large false` to choose the Forward_Open
   service, automatic otherwise).
2. Write the `ForwardOpenRequest` from the `Connection` fields and the originator constants of
   `implicit-io`, then print the connection's N and name (`Connection1 (Exclusive Owner)`) and the
   whole `Connection` (`{connection:#?}`).
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

### Example — `eds_parser/examples/io-hub-implicit.rs`

The implicit counterpart of `io-hub-explicit-homing`, one linear file, with the IO-HUB assemblies
included through `#[path = "../../scanner/assemblies"]` and errors reported through `anyhow`:

1. Read the first connection this scanner can open from `--eds` (the local IO-HUB file), and
   check that its path is `CipPath::new_assembly_connection(1, OUTPUT_ASSEMBLY_INSTANCE,
   INPUT_ASSEMBLY_INSTANCE)`, the assemblies the structs are written for.
2. Write the `ForwardOpenRequest` from the `Connection` fields and the originator constants of
   `implicit-io`.
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

* `eds_parser/README.md`: `Eds::parse` and the connection functions, the `ForwardOpenRequest` a caller writes,
  what is accepted and refused, how to run `eds-implicit-io`, and how to write a device's
  assemblies.
* The top-level README lists the crate and its examples; `tests/integration/README.md` describes
  the `eds-implicit-io` run against OpENer.
* `phases/README.md` lists phase 4's pull request.

## Limits

* Class 1 cyclic, exclusive owner, point-to-point only; one connection per run.
* Configuration data is never read or sent; of the file, only `ParamN`, `AssemN` and
  `ConnectionN` entries are read.
* Paths of logical segments only; `SYMBOL_ANSI` in a path is refused.
* `{ }` groups are split as one field but never read; `\xnn` and `\unnnn` escapes are not decoded.

## Tests

* Fixture `eds_parser/tests/fixtures/sample_adapter.eds`, written for this crate (no third-party
  file): the connection the OpENer container accepts (configuration 151, O->T 150, T->O 100,
  32 bytes each, 32-bit header O->T, modeless T->O, scheduled priority, RPI param defaulting to
  1 s) plus an input-only connection. It also covers CRLF line endings, a joined multi-line
  `IconContents`, a `[TCP/IP Interface Class]` section, dates and times, an empty size resolved
  through `AssemN`, an RPI from `ParamN`, a `[ParamN]` in the path, configuration data to be
  ignored.
* Sections and entries: comments, CRLF, separators inside strings and `{ }`, each syntax error
  with where it is.
* Field values: each `FromStr`, numbers in every notation.
* Entry structs: fields filled in order, an empty `Option`, a missing trailing field, an extra
  field, a `Vec` of member structs reading the rest of the fields, and the keyword and field
  number in errors; a param with its descriptor and data type, an assembly with members, a
  malformed connection.
* The file: references resolved to param defaults, paths and assembly sizes; a duplicate entry
  failing the parse, a missing param failing its connection.
* Mask words: the shared priority, every real-time format code.
* Connections: an IO-HUB style entry read into every value, each refusal of the table, both size
  types, the automatic and the forced Large_Forward_Open, and picking the first connection this
  scanner can open.
* End to end (`tests/sample_adapter.rs`): the fixture's `Connection1`, with the originator of the
  library's Forward_Open test, builds the same Forward_Open bytes as that test; the input-only
  `Connection2` is refused with what it offers.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test
```

Against the OpENer container from `tests/integration`, `cargo run --example eds-implicit-io` opens
the connection, exchanges data for the requested number of cycles, and closes cleanly. Against an
IO-HUB-4-E, `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip>`
does the same with I/O every 10 ms.
