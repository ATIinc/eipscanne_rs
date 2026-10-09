# eds_parser

Reads an EtherNet/IP device's EDS file and gives the fields of the Forward_Open that opens one of
its connections from the `scanner` crate. Four steps, one module each, in the order the work
happens:

| Step | Module | What it does |
|---|---|---|
| 1 | `eds.pest` | The syntax: `[Section]` headers, `Keyword = field, ...;` entries spanning any number of lines, quoted strings (adjacent ones join), decimal and `0x` integers, words, `$` comments anywhere. A second rule reads a path (`"20 04 24 [Param3] 2C 96 2C 64"`). |
| 2 | `document` | Sections, entries and fields, with no EDS meaning; crate-internal. Every error about a field is worded here, with the entry's line. |
| 3 | `params`, `assembly`, `connection` | Typed views of `ParamN`, `AssemN` and `ConnectionN` entries with their references resolved: an RPI or size given as `ParamN` takes the param's default, a direction's format is its `AssemN` (and its size when the connection gives none), `[ParamN]` in a path becomes the default's little-endian bytes. The two mask words are `bilge` bitfields. |
| 4 | `forward_open` | One method on `Connection` per Forward_Open field the EDS decides, named after the field, plus the real-time format of each direction. |

`Eds::parse(&str)` runs steps 1 to 3. Every failure is one `error::Error { entry, message }`:
`Assem100 (line 212): has a size of 4 bytes, while its members add up to 16 bits`.

## Opening a connection

The caller writes the `ForwardOpenRequest`: what an EDS does not describe (tick time, timeouts,
connection IDs, the connection triad) by hand, the rest from the connection:

```rust
let request = ForwardOpenRequest {
    // ... originator fields ...
    o2t_requested_packet_interval: connection.o2t_requested_packet_interval()?,
    o2t_network_connection_parameters: connection.o2t_network_connection_parameters(large)?,
    t2o_requested_packet_interval: connection.t2o_requested_packet_interval()?,
    t2o_network_connection_parameters: connection.t2o_network_connection_parameters(large)?,
    transport_type_trigger: connection.transport_type_trigger()?,
    connection_path: connection.connection_path()?,
};
let o2t_real_time_format = connection.o2t_real_time_format()?;
let t2o_real_time_format = connection.t2o_real_time_format()?;
```

The masks say what the device *supports*; each method picks one value, or refuses the connection:

| Method | Taken from the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, client | class 1, cyclic or exclusive owner not supported |
| `*_network_connection_parameters` | point-to-point; the resolved size plus the 2-byte sequence count and, with a 32-bit header, 4 more (EDS sizes exclude both); fixed size if supported, else variable; scheduled, else high, else low priority, among those both directions support | not point-to-point, no shared priority, over 511 bytes without `large` |
| `*_requested_packet_interval` | the resolved RPI, in microseconds | none in the EDS |
| `*_real_time_format` | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5, 6, 7 |
| `connection_path` | the resolved path | segments other than logical ones |

The client/server bit of the trigger mask is ignored (vendors disagree on it; a Forward_Open's
originator is always the client), and configuration data the EDS declares is not read.

Not parsed: `{ }` nested fields, `L"..."` strings and escaped quotes inside strings. Such a file
fails with pest's line and column. A `0b` number reads as a word.

## Running `eds-implicit-io`

The stages of the scanner's `implicit-io` example with the connection settings read from an EDS.
Its defaults (`--eds` the `sample_adapter.eds` fixture, `--host 172.28.0.10`) describe the OpENer
adapter of `tests/integration`, so with that adapter running (`tests/integration/start-opener.sh`
on the host):

```
cargo run --example eds-implicit-io
```

Flags: `--eds <file>`, `--connection <ConnectionN or name>` (default: the first exclusive-owner
connection), `--host`, `--cycles`, `--large`, `--run`. The outputs are zeros and the scanner stays
idle unless `--run` is given, because an output assembly may drive real outputs; with a modeless
O->T direction idle cannot be signalled, so the example refuses to start without `--run`. Ctrl+C
ends the exchange early; the connection is still closed and the session unregistered.

## Writing a device's assemblies

Assemblies are plain `binrw` structs written by hand in `scanner/assemblies/<device>/`, so they
keep groups, named bits, enums and helpers. The same struct then decodes an explicit
Get_Attribute_Single reply (`decode_reply`) and the data of an implicit input packet
(`T::read_le` on the bytes `t2o::accept_t2o_packet` returns), and is sent as implicit outputs with
`o2t::build_o2t_packet(.., CipDataOpt::Typed(Box::new(outputs)), ..)`.

1. Print the layout, one line per member with its byte offset, size, type, param, units, help
   and bit names:
   ```
   cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100
   ```
   Without `--assembly` it lists which assemblies each connection carries, then all of them.
2. Write the structs from it, or hand the printout to Claude to write them. Give reserved bytes a
   named `_reserved` field, so every byte of the layout shows up in the struct.
3. Add a test against a capture of the real device.

`io-hub-implicit` opens the IO-HUB-4-E's connection from the hub's EDS, then decodes every input
packet as an `InputAssemblyHub4E` and prints one motor's status when it changes. Outputs are
`OutputAssemblyHub4E::default()` (nothing enabled), sent idle unless `--run`:

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
* Against a device's own file: `EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored`.
