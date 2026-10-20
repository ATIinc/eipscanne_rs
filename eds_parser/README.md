# eds_parser

Reads an EtherNet/IP device's EDS file into its params, assemblies and connections, and one
connection into the values of the Forward_Open that opens it from the `scanner` crate, in the
`eipscanne_rs` types.

An EDS file is `[Section]` headers, each followed by `Keyword = field, field, ...;` entries. Each
kind of entry read here is a struct whose fields are the entry's fields in file order, in the
module of its section:

| Module | What it holds |
|---|---|
| `syntax` | The text split into sections and entries by the grammar in `eds.pest`, and an entry's fields read in file order. `$` comments run to the end of the line, entries span any number of lines, and `,` `;` `$` inside quoted strings (and `,` inside `{ }`) do not split. |
| `fields` | `FromEntry` (derived by `eds_parser_derive`) for a struct read from consecutive fields, and `FromField` (derived by `eds_parser_derive` for `bilge` bitfields and enums) for a value read from one field. An empty field is `None` (`""` for a `String`), a field past the end of a short entry is empty, a `Vec` member takes the rest of the fields, and a non-empty field past the last one read is an error. Numbers are decimal, `0x` or `0b`; adjacent quoted strings join, and escapes such as `\n` and `\x41` are read. |
| `values` | The kinds of value a field holds beyond strings and integers: `NumberOrParam`, `DataFormat`, `ParamValue`, `Path` (`"20 04 24 [Param3] 2C 96 2C 64"`), `MemberReference`. |
| `params` | `[Params]`: `Param`, its `ParamDescriptor` bits and `CipDataType` code (`0xC8` is `Udint`). |
| `assembly` | `[Assembly]`: `Assembly` and its members. |
| `connection_manager` | `[Connection Manager]`: `Connection` and its two mask words as `bilge` bitfields, whose `Debug` names every bit. |
| `forward_open` | A `Connection` as the fields of a `ForwardOpenRequest`, or why it is refused. |

`Eds::parse(&str)` reads every `ParamN`, `AssemN` and `ConnectionN` entry; the params and
assemblies a connection refers to are looked up when it is opened. Every failure is an
`anyhow::Error` that says where: the keyword, field and line of an entry being read
(`Connection1: field 4: line 3: Param999,x,Assem101,: expected a number or a ParamN, found "x"`),
else the connection (`Connection1: refers to Param7, which the file does not define`).

## Opening a connection

`eds.connections` holds every `Connection` by its N. The caller writes the `ForwardOpenRequest`:
what an EDS does not describe (tick time, timeouts, connection IDs, the connection triad) by
hand, the rest from the connection:

```rust
let eds = Eds::parse(&eds_text)?;
let number: u32 = eds.first_openable_connection(None)?; // or the N of any ConnectionN
let connection: &Connection = eds.connection(number)?;
let request = ForwardOpenRequest {
    // ... originator fields ...
    ..eds.forward_open_request(number, None)?
};
let outputs = vec![0u8; usize::from(eds.o2t_size(number)?)];
// forward_open(&mut session, request, connection.o2t_real_time_format()?, connection.t2o_real_time_format()?)
```

The last argument picks the service: `Some(true)` a Large_Forward_Open (32-bit connection
parameters), `Some(false)` a Forward_Open, `None` a Large_Forward_Open only when a direction is
over 511 bytes on the wire.

The masks say what the device *supports*; one value is picked from them, or the connection is
refused with the mask's `Debug`, which shows what it offers (`Connection2: this scanner opens
class 1, cyclic, exclusive-owner connections; this one offers TriggerAndTransport {
transport_classes: TransportClasses { class_0: false, class_1: true, ... }, ... }`):

| Value | Taken from the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, client | class 1, cyclic or exclusive owner not supported |
| `*_network_connection_parameters` | point-to-point; the size plus the 2-byte sequence count and, with a 32-bit header, 4 more (EDS sizes exclude both); fixed size if supported, else variable; scheduled, else high, else low priority, among those both directions support | not point-to-point, no shared priority, over 511 bytes with `Some(false)` |
| `*_requested_packet_interval` | a number or a `ParamN`'s default, in microseconds | none in the EDS |
| the size (`o2t_size`) | a number or a `ParamN`'s default; when empty, the size of the format's `AssemN` | neither |
| `*_real_time_format()` | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5, 6, 7 |
| `connection_path` | the path, `[ParamN]` replaced by the param's default | segments other than logical ones |

The client/server bit of the trigger mask is ignored (vendors disagree on it; a Forward_Open's
originator is always the client), and configuration data the EDS declares is not read.

Not read: `{ }` groups (split as one field), `SYMBOL_ANSI` in a path.

## Running `eds-implicit-io`

The stages of the scanner's `implicit-io` example with the connection settings read from an EDS.
Its defaults (`--eds` the `sample_adapter.eds` fixture, `--host 172.28.0.10`) describe the OpENer
adapter of `tests/integration`, so with that adapter running (`tests/integration/start-opener.sh`
on the host):

```
cargo run --example eds-implicit-io
```

Flags: `--eds <file>`, `--connection <N>` (default: the first connection this scanner can open), `--host`, `--cycles`, `--large <true|false>` (default: automatic), `--run`. The
outputs are zeros and the scanner stays idle unless `--run` is given, because an output assembly
may drive real outputs; with a modeless O->T direction idle cannot be signalled, so the example
refuses to start without `--run`. Ctrl+C ends the exchange early; the connection is still closed
and the session unregistered.

## Writing a device's assemblies

Assemblies are plain `binrw` structs written by hand in `scanner/assemblies/<device>/`, so they
keep groups, named bits, enums and helpers. The same struct then decodes an explicit
Get_Attribute_Single reply (`decode_reply`) and the data of an implicit input packet
(`T::read_le` on the bytes `t2o::accept_t2o_packet` returns), and is sent as implicit outputs with
`o2t::build_o2t_packet(.., CipDataOpt::Typed(Box::new(outputs)), ..)`.

The members of an `AssemN` entry are `bits, ParamN` pairs, and each `ParamN` gives the member's
type, name and units (`eds.assemblies` and `eds.params` hold both, read). Write the structs from
them, or hand the EDS to Claude to write them; give
reserved bytes a named `_reserved` field so every byte shows up in the struct, and add a test
against a capture of the real device.

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
