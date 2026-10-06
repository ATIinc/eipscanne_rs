# Phase 4 — EDS parser crate and `eds-implicit-io` example

## Goal

Read a device's EDS file, pick one of its connections and turn it into the `ConnectionConfig` of
phase 3, then open that connection and exchange I/O with the `eds-implicit-io` example. The code
reads top to bottom in the order the work happens: text → document → typed sections →
`ConnectionConfig`.

## Scope

### Workspace

* New crate `eds_parser/` (package `eds_parser`, `publish = false`,
  `license = "MIT OR Apache-2.0"`), added to the root `members` and `default-members` so
  `cargo test`, `cargo clippy` and `cargo run --example eds-implicit-io` work from the root.
* Dependencies: `pest`, `pest_derive`, `bilge`, `eipscanne_rs`, `scanner`. Dev-dependencies, for
  the example only: `tokio`, `clap`, `rand`.

### `src/lib.rs`

Module docs listing the four steps above and the module that implements each; `Eds::parse(&str)`
is the single entry point.

### Step 1 — syntax: `src/eds.pest`

All EDS syntax in one grammar file; whitespace (space, tab, `\r`, `\n`) and `$` comments are silent
rules, so they may appear anywhere, including between the fields of one entry.

* `[Section]` headers: letters, digits, spaces, `/`, `-`, `_` (`[TCP/IP Interface Class]`),
  matched case-insensitively.
* `Keyword = field, field, ...;` entries spanning any number of lines; empty fields (`,,`) and an
  empty last field (`1,;`).
* Field forms:
  * quoted strings; adjacent strings join into one (`IconContents = "AAA" "BBB";`);
  * integers: decimal with an optional `-`, and `0x` hex;
  * words: any other unquoted token (`Param999`, `Assem100`, `EtherNetIP`, `Rx`, `TCP`, dates
    `08-11-2025`, times `12:10:26`, revisions `1.6`).
* A second rule for the path string of a connection: hex bytes and `[ParamN]` references
  (`"20 04 24 [Param3] 2C 96 2C 64"`).
* Not supported: `{ }` nested fields, `L"..."` strings, `0b` numbers, string escapes. A file that
  uses them fails with pest's line and column.

### Step 2 — document: `src/document.rs`

* `Document { sections }`, `Section { name, entries }`, `Entry { keyword, fields, line }`,
  `Field { Empty, Integer(i64), Text(String), Word(String) }`.
* Section and keyword lookups ignore case.
* Gives no EDS meaning to anything; this is only the shape of the file, and the only code that
  touches pest's parse tree.

### Step 3 — typed sections

Each typed view resolves its references while it is built, so the bridge only maps values. Fields
are read by position with slice patterns
(`let [trigger_and_transport, connection_parameters, o2t_rpi, o2t_size, o2t_format, ..] = fields`).

* `src/params.rs` — `Param { name, data_size, min, max, default }`, raw values. Display scaling is
  ignored: an RPI param is in microseconds whatever its scaling fields say. `EnumN` entries and the
  other param fields are skipped.
* `src/assembly.rs` — `Assembly { name, size }`. The member list, path and descriptor are skipped.
* `src/connection.rs` — `Connection { keyword, name, trigger_and_transport, connection_parameters,
  o2t, t2o, path }`:
  * `TriggerAndTransport` and `ConnectionParameters`: the two mask words as 32-bit `bilge`
    bitfields, so each bit is named once;
  * `o2t` / `t2o`: `{ requested_packet_interval: Option<u32>, size: u16 }`. The RPI is a number or
    a `ParamN` default; the size is a number, a `ParamN` default, or, when empty, the size of the
    format's `AssemN`. EDS sizes exclude the sequence count and the real-time header, like
    `DirectionConfig::data_size`;
  * configuration data (config #1/#2 size and format) is read and kept, never sent;
  * `path`: the bytes after `[ParamN]` substitution (the param's default, in as many
    little-endian bytes as its data size).
* `[File]`, `[Device]`, `[Port]`, `[Capacity]` and the object class sections are parsed into the
  document but get no typed view.

### Step 4 — bridge: `src/to_connection_config.rs`

`to_connection_config(&Connection, OriginatorSettings) -> Result<ConnectionConfig, BridgeError>`,
producing `scanner::implicit::ConnectionConfig`. `OriginatorSettings` holds what an EDS does not
describe: timeout multiplier, T->O connection ID, connection triad, Large_Forward_Open.

The masks list what the device *supports*; the bridge picks:

| `ConnectionConfig` field | From the EDS | Otherwise |
|---|---|---|
| `transport_class` | class 1 | `Unsupported` |
| `production_trigger` | cyclic | `Unsupported` |
| (application type) | exclusive-owner | `Unsupported` (input-only, listen-only out of scope) |
| (connection type, both directions) | point-to-point | `Unsupported` |
| `priority` (one for both directions) | scheduled, else high, else low, among those both directions support | `Unsupported` |
| `connection_size_type` per direction | fixed if supported, else variable | — |
| `real_time_format` per direction | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | `Unsupported` for 2, 5–7 |
| `data_size` per direction | resolved size | — |
| `requested_packet_interval` per direction | resolved RPI | `MissingRpi` |
| `configuration_instance`, `o2t/t2o.connection_point` | path `20 04 24 cc 2C oo 2C tt` | `UnsupportedPath` (16-bit segments, other classes, extra segments) |

* The client/server bit of the trigger and transport mask is ignored: vendors disagree on it, and
  the originator of a Forward_Open is always the client.
* Configuration data the EDS declares is ignored, with a comment saying why: configuration data
  segments are out of scope, and adapters accept the Forward_Open without them.

### Errors — `src/error.rs`

`EdsError`: syntax errors with pest's line and column; missing section or entry; a field of the
wrong form; an unknown `ParamN` / `AssemN`. Every message names the entry
(`Connection1: O->T size and format are both empty`). `BridgeError` names the connection and the
unsupported value.

### Example — `eds_parser/examples/eds-implicit-io.rs`

The stages of `implicit-io`, with the connection settings coming from an EDS instead of flags.

1. Read and parse the file; select the connection (`--connection Connection1` or its name; default:
   the first exclusive-owner connection).
2. Print what was derived: connection name, path, both sizes, RPIs, real-time formats, priority.
3. `to_connection_config` (originator settings as the constants of `implicit-io`), then register,
   bind UDP, `forward_open`, the cyclic loop, `forward_close`, `unregister`. The loop is copied from
   `implicit-io` on purpose so the example reads top to bottom.
4. Outputs are zeros of the O->T size. The scanner stays **idle** (run flag cleared in the 32-bit
   header) unless `--run` is given: an output assembly may drive motors. With a modeless O->T, the
   example refuses to start without `--run`, since idle cannot be signalled.

Flags: `--eds <file>`, `--connection`, `--host`, `--cycles`, `--large`, `--run`.

### Documentation

* `eds_parser/README.md`: what is parsed, what the bridge accepts and rejects, how to run
  `eds-implicit-io`.
* The top-level README lists the crate; `tests/integration/README.md` gains the
  `eds-implicit-io` run against OpENer with the fixture.
* `phases/README.md`: phase 4's pull request number (the only edit a phase makes there).

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
  and times, an empty size resolved through `AssemN`, an RPI from `ParamN`, configuration data to
  be ignored.
* Grammar and document: inline snippets for every syntax form above, and one failing snippet per
  error showing line and column.
* `[ParamN]` path substitution: inline snippet with 1- and 2-byte params.
* Bridge: each row of the table, including every `Unsupported` case and the ignored client/server
  bit.
* End to end: the fixture's `Connection1` with the originator settings of the phase 3 config test
  equals that test's `ConnectionConfig`, so it builds the same Forward_Open bytes as the
  library's captured request.
* Ignored test for a local EDS:
  `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -p eds_parser -- --ignored`.
  For the IO-HUB it should give configuration 1, O->T 101 with 148 bytes and a 32-bit header,
  T->O 100 with 228 bytes modeless, RPI 10 ms, scheduled priority.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test && cargo test -p eipscanne_rs --features adapter
```

Against the OpENer container from `tests/integration`:
`cargo run --example eds-implicit-io -- --eds eds_parser/tests/fixtures/sample_adapter.eds --run`
opens the connection, exchanges data for the requested number of cycles, and closes cleanly
(`--run` to match `implicit-io`, which always sends in run mode).
