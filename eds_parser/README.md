# eds_parser

Reads an EtherNet/IP device's EDS file into its params, assemblies and connections, and resolves
what a connection refers to. The `scanner` crate's `implicit::eds` turns a connection into the
Forward_Open that opens it (see its [examples](../scanner/examples/)).

An EDS file is `[Section]` headers, each followed by `Keyword = field, field, ...;` entries. Each
kind of entry read here is a struct whose fields are the entry's fields in file order, in the
module of its section:

| Module | What it holds |
|---|---|
| `syntax` | The text split into sections and entries by the grammar in `eds.pest`, and an entry's fields read in file order. `$` comments run to the end of the line, entries span any number of lines, and `,` `;` `$` inside quoted strings (and `,` inside `{ }`) do not split. |
| `fields` | `FromEntry` (derived by `eds_parser_derive`) for a struct read from consecutive fields, and `FromField` (derived by `eds_parser_derive` for `bilge` bitfields and enums) for a value read from one field. An empty field is `None` (`""` for a `String`), a field past the end of a short entry is empty, a `Vec` member takes the rest of the fields, and a non-empty field past the last one read is an error. Numbers are decimal, `0x` or `0b`; adjacent quoted strings join, and escapes such as `\n` and `\x41` are read. The values of more than one section: `NumberOrParam` and `Path` (`"20 04 24 [Param3] 2C 96 2C 64"`). |
| `params` | `[Params]`: `ParamEntry`, its `ParamDescriptor` bits, `CipDataType` code (`0xC8` is `Udint`) and `ParamValue`s. |
| `assembly` | `[Assembly]`: `AssemblyEntry`, its members and their `MemberReference`s. |
| `connection_manager` | `[Connection Manager]`: `ConnectionEntry`, its two mask words as `bilge` bitfields, whose `Debug` names every bit, and its `DataFormat`s. |
| `error` | `Error`: `Syntax`, `Entry` or `Reference`, each with its message, and the `Result` alias. |

`Eds::parse(&str)` reads every `ParamN`, `AssemN` and `ConnectionN` entry; `eds.connections`,
`eds.params` and `eds.assemblies` hold them by their N. The params and assemblies a connection
refers to are looked up when they are read: `eds.requested_packet_interval`, `eds.data_size` (a
direction's bytes of data, from its size field or its `AssemN` format) and `eds.path_bytes` (the
path, `[ParamN]` replaced by the param's default). Every failure is an `error::Error` whose message
says where: the keyword, field and line of an entry being read
(`Connection1: field 4: line 3: Param999,x,Assem101,: expected a number or a ParamN, found "x"`),
or the reference (`refers to Param7, which the file does not define`).

Not read: `{ }` groups (split as one field).
