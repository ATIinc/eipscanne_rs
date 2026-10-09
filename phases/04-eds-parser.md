# Phase 4 — EDS parser crate, `eds-implicit-io` and device assemblies

## Goal

Read a device's EDS file and give, for one of its connections, the fields of the Forward_Open and
the two real-time formats that phase 3's `scanner::implicit::connection::forward_open` takes, and
print the layout of its assemblies so the hand-written `binrw` assemblies of `scanner/assemblies/`
can be written from it. The code reads top to bottom in the order the work happens: text →
document → typed sections → Forward_Open fields.

The assemblies are written by a person or by Claude from the layout printout; the parser does not
generate code. A generator can only emit a flat struct (`motor0_statusword`, ...), while the
hand-written assemblies have per-motor groups, named bits, enums such as `MoveType`, and helpers
such as `motor_input(n)`.

## Scope

### Workspace

* Crate `eds_parser/` (package `eds_parser`, `publish = false`, `license = "MIT OR Apache-2.0"`),
  in the root `members` and `default-members`, so `cargo test`, `cargo clippy` and
  `cargo run --example <name>` work from the root.
* Dependencies: `pest`, `pest_derive`, `bilge`, `binrw`, `eipscanne_rs`. Dev-dependencies:
  `scanner`, `tokio`, `clap`, `rand`, `anyhow` for the examples, and `hex-test-macros` for the
  tests.

### `src/lib.rs`

Module docs listing the four steps and the module that implements each. `Eds::parse(&str)` runs
steps 1 to 3 into `Eds { params, assemblies, connections }`. `Eds::connection` finds a connection
by its `ConnectionN` keyword or its name, `Eds::first_exclusive_owner_connection` gives the default
one, and `Eds::assembly` finds an assembly by keyword. There are no re-exports: each type is
imported from its module (`eds_parser::connection::Connection`).

### Errors — `src/error.rs`

One `Error { entry, message }` and `Result<T, E = Error>`. `entry` says where: `Assem100 (line
212)` for an entry, `line 4, column 7` for a syntax error, `[Connection Manager]` for a missing
section, `Connection2` for a connection this scanner cannot open.

### Step 1 — syntax: `src/eds.pest`

All EDS syntax in one grammar file; whitespace (space, tab, `\r`, `\n`) and `$` comments are silent
rules, so they may appear anywhere, including between the fields of one entry.

* `[Section]` headers: letters, digits, spaces, `/`, `-`, `_`, `.` (`[TCP/IP Interface Class]`),
  looked up case-insensitively.
* `Keyword = field, field, ...;` entries spanning any number of lines; empty fields (`,,`) and an
  empty last field (`1,;`).
* Field forms:
  * quoted strings; adjacent strings join into one (`IconContents = "AAA" "BBB";`);
  * integers: decimal with an optional `-`, and `0x` hex;
  * words: any other unquoted token (`Param999`, `Assem100`, `EtherNetIP`, `Rx`, `TCP`, dates
    `08-11-2025`, times `12:10:26`, revisions `1.6`).
* A second rule for a path string: hex bytes and `[ParamN]` references
  (`"20 04 24 [Param3] 2C 96 2C 64"`).
* Not supported: `{ }` nested fields and `L"..."` strings, which fail with pest's line and column.
  Backslashes in a string are kept as written, so an escaped quote ends the string and the entry
  fails the same way. A `0b` number reads as a word.

### Step 2 — document: `src/document.rs`

* `Document { sections }`, `Section { name, entries }`, `Entry { keyword, fields, line }`,
  `Field { Empty, Integer(i64), Text(String), Word(String) }`, all crate-internal.
* Section lookups ignore case; `Section::numbered_entries("Param")` gives `Param1`, `Param12`, ...
* `Entry::integer`, `Entry::optional_integer` and `Entry::text` read a field by position, and
  `Entry::error` / `Entry::bad_field` word every error about an entry, with its line:
  `Param1 (line 2): field 6 should be the data size in bytes, found the text "four"`.
* The only code that touches pest's parse tree.

### Step 3 — typed sections

Each typed view resolves its references while it is built. Fields are read by position through
named index constants (`const O2T_RPI: usize = 2;`); a field past the end of a short entry is
`Empty`.

* `src/params.rs` — `Param { keyword, name, data_type, data_size, units, help, min, max, default,
  enum_names }`, raw values: display scaling is ignored, so an RPI param is in microseconds
  whatever its scaling fields say. `data_type` is the CIP type code; `Param::type_name` gives its
  name (`UINT`) and `Param::is_bit_string` tells BYTE, WORD, DWORD and LWORD apart. `enum_names`
  are the `(value, name)` pairs of the `EnumN` with the param's number, bit numbers for a bit
  string and values otherwise.
* `src/assembly.rs` — `Assembly { keyword, name, path, size, members }`:
  * `path`: the bytes with `[ParamN]` resolved; `Assembly::instance()` reads them as a `CipPath`
    and gives its instance segment (`20 04 24 64 30 03` -> 100), or `None`;
  * `members: Vec<Member>`, with `Member { offset_bits, size_bits, param: Option<Param> }`, read
    from the `bits, ParamN` pairs after the descriptor field, each placed right after the previous
    one; `bits,` with no param is an opaque member, and `bits, AssemN` (a nested assembly) is
    refused;
  * the member bits must add up to `size * 8` when a size is given; an empty size is the sum of
    the members.
* `src/connection.rs` — `Connection { keyword, name, trigger_and_transport, connection_parameters,
  o2t, t2o, path }`:
  * `TriggerAndTransport` and `ConnectionParameters`: the two mask words as 32-bit `bilge`
    bitfields, so each bit is named once;
  * `o2t` / `t2o`: `DirectionSpec { requested_packet_interval: Option<u32>, size: u16, assembly:
    Option<Assembly> }`. The RPI and size are a number or a `ParamN` default; `assembly` is the
    `AssemN` the format field names, and gives the size when the size field is empty. EDS sizes
    exclude the sequence count and the real-time header;
  * `path`: the bytes after `[ParamN]` substitution (the param's default, in as many
    little-endian bytes as its data size);
  * configuration data the connection declares is not read.
* `[File]`, `[Device]`, `[Port]`, `[Capacity]` and the object class sections are parsed into the
  document but get no typed view.

### Step 3 output — the layout printout

`impl Display for Assembly`: a heading with keyword, name, instance and size, then one line per
member with byte offset (`byte.bit` when not byte-aligned), bit size, CIP type name, param keyword
and name, units and help, plus the enum names indented under their member. Example
`eds-assemblies --eds <file> [--assembly Assem100]` prints it for one assembly, or lists which
assemblies each connection carries and then prints every assembly. This output goes into the
prompt when asking Claude to write `scanner/assemblies/<device>/`; reserved bytes become named
`_reserved` fields, so every byte of the layout shows up in the struct.

### Step 4 — Forward_Open fields: `src/forward_open.rs`

An `impl Connection` with one method per Forward_Open field the EDS decides, named after the
field, each returning the `eipscanne_rs` type. The caller writes the `ForwardOpenRequest` from
them, with what an EDS does not describe: tick time and timeout ticks of the Forward_Open itself,
timeout multiplier, connection IDs (`o2t_network_connection_id` 0: the target chooses it) and the
connection triad.

The masks list what the device *supports*; each method picks one value or refuses the connection
with an `Error` naming it:

| Method | From the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, `Direction::Client` | class 1, cyclic or exclusive owner not supported (input-only and listen-only are out of scope) |
| `o2t_network_connection_parameters(large)`, `t2o_...` | point-to-point, exclusive owner; priority: scheduled, else high, else low, among those both directions support; fixed size if supported, else variable; resolved size plus `connection_size`'s overhead for class 1 and the direction's real-time format; 32-bit when `large` | not point-to-point, no shared priority, over 511 bytes without `large` |
| `o2t_requested_packet_interval`, `t2o_...` | resolved RPI | none in the EDS |
| `o2t_real_time_format`, `t2o_...` (the Forward_Open does not carry them) | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5–7 |
| `connection_path` | the resolved path read as a `CipPath` | segments other than logical ones |

The client/server bit of the trigger and transport mask is ignored: vendors disagree on it, and
the originator of a Forward_Open is always the client.

### Example — `eds_parser/examples/eds-implicit-io.rs`

The stages of `implicit-io`, with the connection settings taken from an EDS, not from flags. Its
defaults describe the OpENer adapter of `tests/integration`: `--eds` is the `sample_adapter.eds`
fixture (by its path under `CARGO_MANIFEST_DIR`) and `--host` is `172.28.0.10`, so
`cargo run --example eds-implicit-io` runs against that container. The module docs give the
commands that start the adapter and what the run prints.

1. Read and parse the file; select the connection (`--connection Connection1` or its name; default:
   the first exclusive-owner connection).
2. Write the `ForwardOpenRequest` from the `Connection` methods and the originator constants of
   `implicit-io` (`--large` for 32-bit parameter words), then print the connection name and
   keyword, path, both sizes, RPIs, real-time formats and the whole request.
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

The implicit counterpart of `io-hub-homing`, one linear file, with the IO-HUB assemblies included
through `#[path = "../../scanner/assemblies"]` and errors reported through `anyhow`:

1. Parse `--eds` (the local IO-HUB file), pick the first exclusive-owner connection, write its
   `ForwardOpenRequest` from the `Connection` methods and the originator constants of
   `implicit-io`, and print the connection path as hex.
2. Check that the path ends with `2C 65 2C 64`, the connection points `OUTPUT_ASSEMBLY_INSTANCE`
   and `INPUT_ASSEMBLY_INSTANCE` the structs are written for.
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

* `eds_parser/README.md`: the four steps, the `ForwardOpenRequest` a caller writes, what each
  method accepts and refuses, how to run `eds-implicit-io`, and how to write a device's
  assemblies.
* The top-level README lists the crate and its examples; `tests/integration/README.md` describes
  the `eds-implicit-io` run against OpENer.
* `phases/README.md` lists phase 4's pull request.

## Limits

* Class 1 cyclic, exclusive owner, point-to-point only; one connection per run.
* Configuration data is never read or sent.
* Paths of logical segments only.

## Tests

* Fixture `eds_parser/tests/fixtures/sample_adapter.eds`, written for this crate (no third-party
  file): the connection the OpENer container accepts (configuration 151, O->T 150, T->O 100,
  32 bytes each, 32-bit header O->T, modeless T->O, scheduled priority, RPI param defaulting to
  1 s) plus an input-only connection. It also covers CRLF line endings, a joined multi-line
  `IconContents`, a `[TCP/IP Interface Class]` section, dates and times, an empty size resolved
  through `AssemN`, an RPI from `ParamN`, a `[ParamN]` in the path, configuration data to be
  ignored.
* Grammar and document: inline snippets for every supported syntax form above, and failing
  snippets for a missing `;`, an entry outside a section and `{ }` nested fields, which report
  pest's line and column.
* Params, assemblies, connections: inline snippets for data types and `EnumN` names, members
  (opaque, nested, non-byte-aligned, a sum that does not match the size), a `[ParamN]` in a path,
  `instance()` with 8-bit and 16-bit instance segments, the layout printout, a direction without
  size or format.
* Forward_Open fields: each refusal of the table, both size types, every real-time format, the
  priority choice, the Large_Forward_Open size and the ignored client/server bit.
* End to end (`tests/sample_adapter.rs`): the fixture's `Connection1`, with the originator of the
  library's Forward_Open test, builds the same Forward_Open bytes as that test; the input-only
  `Connection2` is refused.
* Ignored, with a local EDS (`EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`):
  for the IO-HUB, configuration 1, O->T 101 with 148 bytes and a 32-bit header, T->O 100 with
  228 bytes modeless, RPI 10 ms, scheduled priority; `scanner/assemblies/io_hub` input (100) and
  output (101) have the instances of the connection's assemblies.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test
EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored
cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100
```

Against the OpENer container from `tests/integration`, `cargo run --example eds-implicit-io` opens
the connection, exchanges data for the requested number of cycles, and closes cleanly. Against an
IO-HUB-4-E, `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip>`
does the same with I/O every 10 ms.
