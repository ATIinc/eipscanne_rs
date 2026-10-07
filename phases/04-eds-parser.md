# Phase 4 — EDS parser crate and `eds-implicit-io` example

## Goal

Read a device's EDS file, pick one of its connections and turn it into the Forward_Open request
and the two real-time formats that phase 3's `scanner::implicit::connection::forward_open` takes,
then open that connection and exchange I/O with the `eds-implicit-io` example. The code reads top
to bottom in the order the work happens: text → document → typed sections → `ForwardOpenRequest`.

## Scope

### Workspace

* Crate `eds_parser/` (package `eds_parser`, `publish = false`, `license = "MIT OR Apache-2.0"`),
  in the root `members` and `default-members`, so `cargo test`, `cargo clippy` and
  `cargo run --example eds-implicit-io` work from the root.
* Dependencies: `pest`, `pest_derive`, `bilge`, `eipscanne_rs`. Dev-dependencies: `scanner`,
  `tokio`, `clap`, `rand` for the example, and `binrw`, `hex-test-macros` for the tests.

### `src/lib.rs`

Module docs listing the four steps above and the module that implements each. `Eds::parse(&str)`
runs steps 1 to 3 into `Eds { document, params, assemblies, connections }`, and `to_forward_open`
is step 4. `Eds::connection` finds a connection by its `ConnectionN` keyword or its name, and
`Eds::first_exclusive_owner_connection` gives the default one. There are no re-exports: each type
is imported from its module (`eds_parser::to_forward_open::to_forward_open`).

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
* A second rule for the path string of a connection: hex bytes and `[ParamN]` references
  (`"20 04 24 [Param3] 2C 96 2C 64"`).
* Not supported: `{ }` nested fields and `L"..."` strings, which fail with pest's line and column.
  Backslashes in a string are kept as written, so an escaped quote ends the string and the entry
  fails the same way. A `0b` number reads as a word.

### Step 2 — document: `src/document.rs`

* `Document { sections }`, `Section { name, entries }`, `Entry { keyword, fields, line }`,
  `Field { Empty, Integer(i64), Text(String), Word(String) }`.
* Section and keyword lookups ignore case.
* Gives no EDS meaning to anything; this is only the shape of the file, and the only code that
  touches pest's parse tree.

### Step 3 — typed sections

Each typed view resolves its references while it is built, so the bridge only maps values. Fields
are read by position through named index constants (`const O2T_RPI: usize = 2;`) and
`entry.field(index)`, which gives `Empty` past the end of a short entry.

* `src/params.rs` — `Param { keyword, name, data_size, min, max, default }`, raw values. Display
  scaling is ignored: an RPI param is in microseconds whatever its scaling fields say. `EnumN`
  entries and the other param fields are skipped.
* `src/assembly.rs` — `Assembly { keyword, name, size }`. The member list, path and descriptor are
  skipped.
* `src/connection.rs` — `Connection { keyword, name, trigger_and_transport, connection_parameters,
  o2t, t2o, configuration, path }`:
  * `TriggerAndTransport` and `ConnectionParameters`: the two mask words as 32-bit `bilge`
    bitfields, so each bit is named once;
  * `o2t` / `t2o`: `DirectionSpec { requested_packet_interval: Option<u32>, size: u16, format:
    Option<String> }`. The RPI is a number or a `ParamN` default; the size is a number, a `ParamN`
    default, or, when empty, the size of the format's `AssemN`. EDS sizes exclude the sequence
    count and the real-time header; the bridge adds them;
  * `configuration`: `ConfigurationData` (proxy and target configuration size and format), read
    and kept, never sent;
  * `path`: the bytes after `[ParamN]` substitution (the param's default, in as many
    little-endian bytes as its data size).
* `[File]`, `[Device]`, `[Port]`, `[Capacity]` and the object class sections are parsed into the
  document but get no typed view.

### Step 4 — bridge: `src/to_forward_open.rs`

`to_forward_open(&Connection, OriginatorSettings) -> Result<(ForwardOpenRequest, RealTimeFormat,
RealTimeFormat), BridgeError>`: the request and the O->T and T->O real-time formats, which are the
arguments `scanner::implicit::connection::forward_open` takes after the session.
`OriginatorSettings` holds what an EDS does not describe: priority/time tick and timeout ticks of
the Forward_Open itself, timeout multiplier, T->O connection ID, connection triad,
Large_Forward_Open.

The masks list what the device *supports*; the bridge picks:

| `ForwardOpenRequest` field | From the EDS | Otherwise |
|---|---|---|
| `transport_type_trigger`: transport class | class 1 | `Unsupported` |
| `transport_type_trigger`: production trigger | cyclic | `Unsupported` |
| redundant owner, both parameter words | exclusive-owner | `Unsupported` (input-only, listen-only out of scope) |
| connection type, both parameter words | point-to-point | `Unsupported` |
| priority (the same in both parameter words) | scheduled, else high, else low, among those both directions support | `Unsupported` |
| connection size type per parameter word | fixed if supported, else variable | — |
| connection size per parameter word | resolved size plus `connection_size`'s overhead for class 1 and the direction's real-time format | `ConnectionSizeTooLarge` over 511 bytes without Large_Forward_Open |
| real-time format per direction (returned beside the request) | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | `Unsupported` for 2, 5–7 |
| `o2t/t2o_requested_packet_interval` | resolved RPI | `MissingRpi` |
| `connection_path` | path `20 04 24 cc 2C oo 2C tt` | `UnsupportedPath` (16-bit segments, other classes, extra segments) |

* `o2t_network_connection_id` is 0: the target chooses it. The other fields come from
  `OriginatorSettings`.
* The client/server bit of the trigger and transport mask is ignored: vendors disagree on it, and
  the originator of a Forward_Open is always the client (`Direction::Client`).
* Configuration data the EDS declares is ignored, with a comment saying why: configuration data
  segments are out of scope, and adapters accept the Forward_Open without them.

### Errors — `src/error.rs`

`EdsError`: syntax errors with pest's line and column; a missing section; a field of the
wrong form; an unknown `ParamN` / `AssemN`. Every message about an entry names it
(`Connection1: refers to Param9, which the file does not define`). `BridgeError` (`Unsupported`,
`MissingRpi`, `UnsupportedPath`, `ConnectionSizeTooLarge`) names the connection and what it does
not support.

### Example — `eds_parser/examples/eds-implicit-io.rs`

The stages of `implicit-io`, with the connection settings taken from an EDS, not from flags. It
imports each scanner item from its defining module (`scanner::session::Session`,
`scanner::implicit::connection::{forward_open, forward_close}`,
`scanner::implicit::o2t::{build_o2t_packet, send_io_packet}`,
`scanner::implicit::t2o::{bind_io_socket, recv_io_packet, accept_t2o_packet, input_timeout,
FIRST_PACKET_GRACE}`).

1. Read and parse the file; select the connection (`--connection Connection1` or its name; default:
   the first exclusive-owner connection).
2. `to_forward_open` (originator settings as the constants of `implicit-io`), then print what it
   derived: connection name and keyword, path, both sizes, RPIs, real-time formats and the whole
   request (priority included).
3. Register, `bind_io_socket`, `forward_open` with the request and the two real-time formats, the
   cyclic loop, `forward_close`, `unregister`. Each send tick builds the outputs with
   `build_o2t_packet` and sends them with `send_io_packet`; each received packet goes through
   `recv_io_packet` and `accept_t2o_packet`, and an `Error::UnexpectedPacket`, or an
   `Error::Parse` for a datagram that is not an I/O packet, is printed as `DISCARDED` and the
   loop goes on; the deadline is `input_timeout` after the last accepted
   packet (at least `FIRST_PACKET_GRACE` after the Forward_Open reply for the first). The loop
   follows `implicit-io` and is repeated on purpose so the example reads top to bottom; the
   outputs never change, so the CIP sequence count stays at 1.
4. Outputs are zeros of the O->T size. The scanner stays **idle** (run flag cleared in the 32-bit
   header) unless `--run` is given: an output assembly may drive motors. Without a 32-bit header on
   O->T, the example refuses to start without `--run`, since idle cannot be signalled.
5. The exchange ends after `--cycles` output packets, on the input timeout or on Ctrl+C; the
   connection is closed and the session unregistered in all three cases.

Flags: `--eds <file>`, `--connection`, `--host` (default `172.28.0.10`), `--cycles`, `--large`,
`--run`.

### Documentation

* `eds_parser/README.md`: what is parsed, what the bridge accepts and rejects, how to run
  `eds-implicit-io`.
* The top-level README lists the crate; `tests/integration/README.md` describes the
  `eds-implicit-io` run against OpENer with the fixture.
* `phases/README.md` lists phase 4's pull request.

## Limits

* Class 1 cyclic, exclusive owner, point-to-point only; one connection per run.
* Configuration data is never sent.
* 8-bit path segments to the Assembly object only.

## Tests

* Fixture `eds_parser/tests/fixtures/sample_adapter.eds`, written for this crate (no third-party
  file): the connection the OpENer container accepts (configuration 151, O->T 150, T->O 100,
  32 bytes each, 32-bit header O->T, modeless T->O, scheduled priority, RPI param defaulting to
  1 s, the `implicit-io` default) plus an input-only connection. It also covers: CRLF line endings,
  a joined multi-line `IconContents`, a `[TCP/IP Interface Class]` section, `Revision = 1.0`, dates
  and times, an empty size resolved through `AssemN`, an RPI from `ParamN`, a `[ParamN]` in the
  path, configuration data to be ignored.
* Grammar and document: inline snippets for every supported syntax form above, and failing
  snippets for a missing `;`, an entry outside a section and `{ }` nested fields, which report
  pest's line and column.
* `[ParamN]` path substitution: inline snippet with 1- and 2-byte params.
* Bridge: each row of the table, including every `Unsupported` case, `ConnectionSizeTooLarge` and
  the ignored client/server bit.
* End to end (`tests/sample_adapter.rs`): the fixture's `Connection1` with the originator of the
  library's Forward_Open test builds the same Forward_Open bytes as that test; the input-only
  `Connection2` is refused.
* Ignored test for a local EDS:
  `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`.
  For the IO-HUB it asserts configuration 1, O->T 101 with 148 bytes and a 32-bit header,
  T->O 100 with 228 bytes modeless, RPI 10 ms, scheduled priority.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test && cargo test --features adapter
```

Against the OpENer container from `tests/integration`:
`cargo run --example eds-implicit-io -- --eds eds_parser/tests/fixtures/sample_adapter.eds --run`
opens the connection, exchanges data for the requested number of cycles, and closes cleanly
(`--run` to match `implicit-io`, which always sends in run mode).
