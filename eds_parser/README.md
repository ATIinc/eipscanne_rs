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

`Eds::parse(&str)` runs steps 1 to 3. An `Assembly` also keeps its members (`size, ParamN`
pairs, each param with its type, units, help and `EnumN` names), and `check` compares a caller's
assembly struct with them.

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
O->T direction idle cannot be signalled, so the example refuses to start without `--run`. Ctrl+C
ends the exchange early; the connection is still closed and the session unregistered.

## Writing a device's assemblies

Assemblies are plain `binrw` structs written by hand in `scanner/assemblies/<device>/`, so they
keep groups, named bits, enums and helpers. The same struct then decodes an explicit
Get_Attribute_Single reply (`decode_reply`) and an implicit input packet (`Input::decode`), and
is sent as implicit outputs with `Producer::next_packet_from`.

1. Print the layout, one line per member with its byte offset, size, type, param, units, help
   and bit names:
   ```
   cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100
   ```
   Without `--assembly` it lists which assemblies each connection carries, then all of them.
2. Write the structs from it, or hand the printout to Claude to write them. Give reserved bytes a
   named `_reserved` field, so every byte of the layout shows up in the struct.
3. Check them with `check_assembly::<T>(&assembly)`, in a test and before opening a connection:

   | Step | What is done | What it catches |
   |---|---|---|
   | size | read `size` zero bytes, write them back | a missing, extra or wrong-width field |
   | coverage | per member: only its bits set, read, write back, compare | padding over bytes the EDS calls data |
   | values | per number member: a distinctive value, read, look for it in the struct's `Debug` output | a field of the wrong width or signedness, reordered fields |

   Members named "Reserved…" may be padding. Bit strings (BYTE, WORD, DWORD, LWORD) only get the
   coverage step, since they are usually bitfields. A member whose probe the struct refuses (an
   enum without that value) is returned as not checked rather than failing. Two adjacent fields of
   the same type that are swapped pass every step: a test against captured data catches that.
4. Add a test against a capture of the real device.

`io-hub-implicit` does all of this for the IO-HUB-4-E: it derives the connection from the hub's
EDS, refuses to open it unless `InputAssemblyHub4E` and `OutputAssemblyHub4E` pass the check, then
decodes every input packet as an `InputAssemblyHub4E` and prints one motor's status when it
changes. Outputs are `OutputAssemblyHub4E::default()` (nothing enabled), sent idle unless `--run`:

```
cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip> --motor 0
```

## Tests

* `tests/fixtures/sample_adapter.eds`: a file written for this crate (no third-party file)
  describing the connection the OpENer sample application accepts plus an input-only one; it
  exercises CRLF line endings, joined strings, `ParamN` and `AssemN` references, a `[ParamN]` in
  the path and configuration data to be ignored.
* `tests/sample_adapter.rs`: the fixture end to end, down to the Forward_Open bytes of the
  library's captured request.
* Against a device's own file: `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`,
  which also checks the IO-HUB assemblies of `scanner/assemblies/` (`tests/io_hub_assemblies.rs`).
