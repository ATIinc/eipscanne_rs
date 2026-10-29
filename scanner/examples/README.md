# Examples

The examples of the [`scanner`](../) crate, each a single file. Run one from the
repository root: `cargo run --example <name>`; `-- --help` lists its flags.
`RUST_LOG=scanner=debug` shows the scanner's log lines.

| Example | Device | Messaging | Moves hardware |
|---|---|---|---|
| `read-identity` | any adapter | explicit | no |
| `read-identity-connected` | any class 3 adapter | explicit (connected) | no |
| `opener-implicit` | OpENer | implicit | no |
| `opener-eds-implicit` | OpENer, from its EDS | implicit | no |
| `io-hub-explicit-homing` | Teknic IO-HUB-4-E | explicit | a motor |
| `io-hub-eds-implicit` | Teknic IO-HUB-4-E, from its EDS | implicit | no |
| `flexi-net-implicit` | SICK flexi-net | implicit | safety logic inputs |

The identity and OpENer examples default to `172.28.0.10`, the adapter
`tests/integration/start-opener.sh` starts on the host
([tests/integration](../../tests/integration/)). Device assemblies live in
`scanner/assemblies/`.

## Identity

### read-identity

Registers a session, reads the Identity object and unregisters.

### read-identity-connected

Opens a class 3 connection, reads the Identity object over it a number of times and closes it.

## OpENer

OpENer echoes the outputs it receives (assembly 150) as its inputs (assembly 100), idle ones too.
The adapter crate's `sample-implicit-adapter` stands in for OpENer and echoes run outputs only.

### opener-implicit

Opens a class 1 connection, exchanges I/O for a number of cycles and closes it. `main` is the five
stages in order: register the session, open the connection, exchange I/O, close the connection,
unregister. `--toggle-every N` switches the outputs between run and idle every N cycles.

### opener-eds-implicit

`opener-implicit` with the connection read from OpENer's EDS
(`scanner/tests/fixtures/sample_adapter.eds`). `--idle` sends the outputs idle.

## Teknic

### io-hub-explicit-homing

Homes one ClearPath-IP motor on an IO-HUB-4-E, then disables it however it ended, Ctrl+C
included. The adapter crate's `sample-io-hub-adapter` stands in for the hub.

### io-hub-eds-implicit

Opens the IO-HUB-4-E's connection from its EDS and prints one motor's status whenever it
changes. Every output stays off; `--idle` sends them idle. Against `sample-io-hub-adapter` the
position moves in spurts.

## SICK

### flexi-net-implicit

`opener-implicit` against a SICK flexi-net. The outputs count up one byte at a time, then reset
to 0 before the connection closes.

## Writing a device's assemblies

Write plain `binrw` structs in `scanner/assemblies/<device>/`. The same struct decodes an explicit
reply and implicit inputs, and is sent as implicit outputs. An EDS's `AssemN` members are
`ParamN`s, each with a type, name and units: write the structs from them (or hand the EDS to
Claude), name reserved bytes `_reserved`, and test against the real device's packet bytes.
