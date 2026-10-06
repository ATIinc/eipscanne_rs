# eds_parser

Reads an EtherNet/IP device's EDS file and turns one of its connections into the
`ConnectionConfig` the `scanner` crate opens. Four steps, one module each, in the order the work
happens:

| Step | Module | What it does |
|---|---|---|
| 1 | `eds.pest` | The syntax: `[Section]` headers, `Keyword = field, ...;` entries spanning any number of lines, quoted strings (adjacent ones join), decimal and `0x` integers, words, `$` comments anywhere. A second rule reads a connection path (`"20 04 24 [Param3] 2C 96 2C 64"`). |
| 2 | `document` | `Document { sections }` → `Section { name, entries }` → `Entry { keyword, fields, line }` → `Field::{Empty, Integer, Text, Word}`. No EDS meaning; lookups ignore case. |
| 3 | `params`, `assembly`, `connection` | Typed views of `ParamN`, `AssemN` and `ConnectionN` entries with their references resolved: an RPI or size given as `ParamN` takes the param's default, an empty size takes the format's `AssemN` size, `[ParamN]` in a path becomes the default's little-endian bytes. The two mask words are `bilge` bitfields. |
| 4 | `to_connection_config` | `to_connection_config(&Connection, OriginatorSettings)`: picks what this scanner asks for from what the device supports. |

`Eds::parse(&str)` runs steps 1 to 3.

## What the bridge accepts

The masks say what the device *supports*; the bridge picks one value per field.

| `ConnectionConfig` field | Taken from the EDS | Refused when |
|---|---|---|
| transport class | class 1 | not supported |
| production trigger | cyclic | not supported |
| application type | exclusive owner | input-only or listen-only only |
| connection type, both directions | point-to-point | not supported |
| priority (one for both directions) | scheduled, else high, else low, among those both directions support | none shared |
| connection size type, per direction | fixed if supported, else variable | — |
| real-time format, per direction | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5, 6, 7 |
| data size, per direction | the resolved size (EDS sizes exclude the sequence count and real-time header, like `DirectionConfig::data_size`) | — |
| requested packet interval, per direction | the resolved RPI, in microseconds | none in the EDS |
| configuration instance and connection points | a path of the shape `20 04 24 cc 2C oo 2C tt` | any other path (16-bit segments, other classes, extra segments) |

`OriginatorSettings` holds what an EDS does not describe: the timeout multiplier, the T->O
connection ID, the connection triad and whether to use a Large_Forward_Open. The client/server bit
of the trigger mask is ignored (vendors disagree on it; a Forward_Open's originator is always the
client), and configuration data the EDS declares is kept but never sent.

Not parsed: `{ }` nested fields, `L"..."` strings, `0b` numbers, string escapes. Such a file
fails with pest's line and column.

## Running `eds-implicit-io`

The stages of the scanner's `implicit-io` example with the connection settings read from an EDS:

```
cargo run --example eds-implicit-io -- --eds eds_parser/tests/fixtures/sample_adapter.eds --host 172.28.0.10 --run
```

Flags: `--eds <file>`, `--connection <ConnectionN or name>` (default: the first exclusive-owner
connection), `--host`, `--cycles`, `--large`, `--run`. The outputs are zeros and the scanner stays
idle unless `--run` is given, because an output assembly may drive real outputs; with a modeless
O->T direction idle cannot be signalled, so the example refuses to start without `--run`.

## Tests

* `tests/fixtures/sample_adapter.eds`: a file written for this crate (no third-party file)
  describing the connection the OpENer sample application accepts plus an input-only one; it
  exercises CRLF line endings, joined strings, `ParamN` and `AssemN` references, a `[ParamN]` in
  the path and configuration data to be ignored.
* `tests/sample_adapter.rs`: the fixture end to end, down to the Forward_Open bytes of the
  library's captured request.
* Against a device's own file: `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -p eds_parser -- --ignored`.
