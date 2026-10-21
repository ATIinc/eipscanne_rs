# Examples

The examples live in the `scanner` crate, which also holds what they are built on: the
encapsulation `session` (shared), `explicit` messaging (one request, one reply; `connected` for
a class 3 connection) and `implicit` messaging (a class 1 I/O connection: `connection` plus one
submodule per direction). `read-identity` and `clearlink-explicit-outputs` use `session` and
`explicit`; `read-identity-connected` uses `session` and `explicit::connected`; `implicit-io` and
`flexi-net-implicit` use `session` and `implicit`. `eds-implicit-io` and `io-hub-implicit` also
use `implicit::eds`, which writes the Forward_Open of a connection read from an EDS file by the
`eds_parser` crate.

## Run Examples

From the repository root: `cargo run --example <name> -- <arguments>` (the root `Cargo.toml`
lists the scanner among the workspace's default members, so no `-p scanner` is needed).

## Examples Explained

| Example | Device | Messaging | Moves hardware |
|---|---|---|---|
| `read-identity` | any adapter | explicit | no |
| `read-identity-connected` | OpENer or any class 3 adapter | explicit (connected) | no |
| `clearlink-explicit-outputs` | Teknic ClearLink | explicit | digital outputs |
| `io-hub-explicit-homing` | Teknic IO-HUB-4-E / ClearPath-IP | explicit (polling) | a motor |
| `implicit-io` | OpENer or any class 1 adapter | implicit | outputs of the adapter |
| `eds-implicit-io` | OpENer or any class 1 adapter, from its EDS file | implicit | outputs of the adapter, with `--run` |
| `io-hub-implicit` | Teknic IO-HUB-4-E, from its EDS file | implicit | no (every output off) |
| `flexi-net-implicit` | SICK flexi-net | implicit | inputs of the safety logic |

Every example is a single file. The device assemblies they use live outside the library in
`scanner/assemblies/`: `clearlink.rs` (config, input, output) and `io_hub.rs` (input, output),
each with a directory of the same name. An example that needs them declares
`#[path = "../assemblies"] mod assemblies { pub mod clearlink; }` and imports
`assemblies::clearlink::...`. Application data is always declared by the caller as plain `binrw`
structs; the library never models it. Each request is written out where it happens: the assembly
paths are built once at the top of `main` and passed to `send_request`.

### read-identity

Requests the Identity object from the connected device.

i.e. `cargo run --example read-identity` (the OpENer adapter of `tests/integration` at
`172.28.0.10` by default; `-- --host <ip>` reads another adapter)

1. Registers a session (`Session::register`)
1. Reads the Identity object (`explicit::read_identity`: Get_Attributes_All, reply decoded as
   `IdentityResponse`)
1. Unregisters the session

### read-identity-connected

Reads the Identity object over a class 3 (connected explicit messaging) connection a number of
times, then closes the connection.

i.e. `cargo run --example read-identity-connected` (OpENer at `172.28.0.10` by default)
* `cargo run --example read-identity-connected -- --reads 10 --interval 500`

Flags: `--host`, `--reads`, `--interval` (milliseconds between reads, also the requested packet
interval, so the adapter drops the connection after four missed reads).

1. Registers a session
1. Opens the connection: a Forward_Open to the Message Router with transport class 3, application
   trigger, 504 bytes each way (`connected::forward_open_explicit`)
1. Every `--interval`: advances the CIP sequence count and sends Get_Attributes_All on the
   Identity object over the connection (`connected::send_request`, Send Unit Data on the O->T
   connection ID), and decodes the reply as `IdentityResponse` (`explicit::decode_reply`). Ctrl+C
   ends early
1. Closes the connection (`connection::forward_close`) and unregisters the session

### clearlink-explicit-outputs

Reads from and writes to a Teknic ClearLink motor controller board using the assembly objects
defined in Teknic's EtherNet/IP Object Reference:
https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=18

i.e. `cargo run --example clearlink-explicit-outputs -- --help`
* `cargo run --example clearlink-explicit-outputs -- --index 4 --on`
* `cargo run --example clearlink-explicit-outputs -- --index 4 --off`
* `cargo run --example clearlink-explicit-outputs -- --index 4 --pwm 100`
* `--host <ip>` picks another ClearLink than the default one

1. Parses the desired digital output to be modified from the command line
1. Registers a session
1. Writes the ConfigAssembly object (`explicit::send_request`, Set_Attribute_Single; a reply with
   a general status other than success is an error)
1. Requests the OutputAssembly object and decodes its data (`explicit::decode_reply`)
1. Modifies the value of the appropriate digital output (from the command line)
1. Writes the modified OutputAssembly object
1. Reads the InputAssembly object and checks the output: DIP Status set while the output is
   driven (on, or a PWM duty cycle other than 0), DOP Status clear (no overload)
1. Unregisters the session; an output that does not read back as written is an error

### io-hub-explicit-homing

Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E over explicit messaging, top to bottom like
the other explicit examples. This moves a real motor, so `--host` is required. The status line
printed while waiting shows the relevant bits and decodes a rejected command's AOI error code.

* `cargo run --example io-hub-explicit-homing -- --host 172.31.19.18 --motor 0`

1. Registers a session
1. Reads the inputs and, if a shutdown is present, raises Shutdown Reset briefly and lowers it again
1. Enables the motor
1. Sends the homing command with the next move number
1. Reads the inputs every 500 ms, printing the status, until `has_homed` or `--timeout-s`
1. Disables the motor, however steps 3 to 5 ended (done, failed or Ctrl+C), and unregisters the
   session

### implicit-io

Opens a class 1 (implicit messaging) connection, exchanges cyclic I/O for a number of cycles and
closes it again. `main` is the five stages of implicit messaging in order; the defaults match the
OpENer sample application of `tests/integration`, which copies the outputs it receives (assembly
150) into the inputs it sends (assembly 100).

i.e. `cargo run --example implicit-io -- --help`
* `cargo run --example implicit-io -- --host 172.28.0.10`
* `cargo run --example implicit-io -- --host 172.28.0.10 --rpi 100 --cycles 50`
* `cargo run --example implicit-io -- --host 172.28.0.10 --large`

Flags: `--host`, `--configuration-instance`, `--output-instance`, `--input-instance`,
`--output-size`, `--input-size`, `--rpi` (milliseconds, both directions), `--cycles`, `--large`.

1. **Session**: registers a session over TCP 44818 (`Session::register`)
1. **Open**: binds the UDP I/O socket on port 2222 first, because the adapter starts sending as
   soon as it has replied; then sends the Forward_Open, built field by field from the flags,
   together with the real-time format of each direction (`connection::forward_open_implicit`), and
   keeps request, reply and formats as an `ImplicitConnection`
1. **Exchange**: one `tokio::select!` loop over four events
    * the send timer fires every O->T actual packet interval: `o2t::build_o2t_packet` frames the
      outputs with the loop's encapsulation sequence number and CIP sequence count, and
      `o2t::send_io_packet` sends them to the O->T endpoint
    * a datagram arrives: `t2o::recv_io_packet` reads it as an I/O packet and
      `t2o::accept_t2o_packet` checks the connection ID, the sender, the sequence number against
      the last accepted one and the size, and returns the Sequenced Address and the I/O data as
      read; the loop prints the inputs and moves the deadline, or says why the datagram was
      discarded (one that does not parse as an I/O packet included) and keeps listening
    * the deadline passes: no input packet arrived within timeout multiplier × packet interval
      (`t2o::input_timeout`), the connection is considered timed out and the loop ends
    * Ctrl+C: the loop ends early; the connection is still closed and the session unregistered
1. **Close**: sends the Forward_Close matching the Forward_Open (`connection::forward_close`)
1. **End session**: unregisters the session

Production code may send the outputs on a task of its own so a slow input handler can never
delay an output packet; the example keeps one loop to stay readable.

Limits: one connection per host (only one socket can bind UDP 2222) and point-to-point in both
directions (no multicast).

### eds-implicit-io

The stages of `implicit-io` with the connection settings read from an EDS file. Its defaults
(`--eds` the `sample_adapter.eds` fixture of `scanner/tests/fixtures`, `--host 172.28.0.10`)
describe the OpENer adapter of `tests/integration`, so with that adapter running
(`tests/integration/start-opener.sh` on the host):

```
cargo run --example eds-implicit-io
```

Flags: `--eds <file>`, `--connection <N>` (default: the first connection this scanner can open),
`--host`, `--cycles`, `--large <true|false>` (default: automatic), `--run`. The outputs are zeros
and the scanner stays idle unless `--run` is given, because an output assembly may drive real
outputs; with a modeless O->T direction idle cannot be signalled, so the example refuses to start
without `--run`. Ctrl+C ends the exchange early; the connection is still closed and the session
unregistered.

The example writes what an EDS does not describe (tick time, timeouts, connection IDs, the
connection triad) and takes the rest from `implicit::eds`:

```rust
let eds = Eds::parse(&eds_text)?;
let number: u32 = first_openable_connection(&eds, None)?; // or the N of any ConnectionN
let connection: &ConnectionEntry = &eds.connections[&number];
let request = ForwardOpenRequest {
    // ... originator fields ...
    ..forward_open_request(&eds, connection, None)?
};
let outputs = vec![0u8; usize::from(eds.data_size(connection.o2t_size, connection.o2t_format)?)];
// forward_open_implicit(&mut session, request, o2t_real_time_format(connection)?, t2o_real_time_format(connection)?)
```

The last argument picks the service: `Some(true)` a Large_Forward_Open (32-bit connection
parameters), `Some(false)` a Forward_Open, `None` a Large_Forward_Open only when a direction is
over 511 bytes on the wire.

The masks say what the device *supports*; one value is picked from them, or the connection is
refused (`Error::UnsupportedConnection`) with the mask's `Debug`, which shows what it offers
(`this scanner opens class 1, cyclic, exclusive-owner connections; this one offers
TriggerAndTransportMask { transport_classes: TransportClasses { class_0: false, class_1: true,
... }, ... }`):

| Value | Taken from the EDS | Refused when |
|---|---|---|
| `transport_type_trigger` | class 1, cyclic, client | class 1, cyclic or exclusive owner not supported |
| `*_network_connection_parameters` | point-to-point; the size plus the 2-byte sequence count and, with a 32-bit header, 4 more (EDS sizes exclude both); fixed size if supported, else variable; scheduled, else high, else low priority, among those both directions support | not point-to-point, no shared priority, over 511 bytes with `Some(false)` |
| `*_requested_packet_interval` | a number or a `ParamN`'s default, in microseconds | none in the EDS |
| the size (`eds.data_size`) | a number or a `ParamN`'s default; when empty, the size of the format's `AssemN` | neither |
| `*_real_time_format` | 0 modeless, 1 zero length, 3 heartbeat, 4 32-bit header | 2, 5, 6, 7 |
| `connection_path` | the path, `[ParamN]` replaced by the param's default | segments other than logical ones, such as `SYMBOL_ANSI` |

The client/server bit of the trigger mask is ignored (vendors disagree on it; a Forward_Open's
originator is always the client), and configuration data the EDS declares is not read.

### io-hub-implicit

Opens the IO-HUB-4-E's connection from the hub's EDS, then decodes every input packet as an
`InputAssemblyHub4E` and prints one motor's status when it changes. Outputs are
`OutputAssemblyHub4E::default()` (nothing enabled), sent idle unless `--run`:

```
cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip> --motor 0
```

### flexi-net-implicit

The stages of `implicit-io` against a SICK flexi-net, with the device's connection fixed in
constants: configuration assembly 1278 (no configuration data, so a 16-bit instance segment),
outputs to connection point 151 and inputs from connection point 152, 4 data bytes each way, a
run/idle header on the outputs only. The connection path starts with the electronic key. The outputs
count slowly, with the run flag set: one byte at a time, byte `i` goes from `10 * i` to
`10 * i + 10` and holds it while the next byte counts, one step every `--count-delay`, while
packets keep going out every RPI. Every packet sent or received prints a line numbered by the
packet count, and every step of the count prints a `COUNT` line. After the last count has been
held for one delay, or on Ctrl+C, one packet sets every byte back to 0 (a `ZERO` line) and the
connection closes. A longer test is a longer `--count-delay`.

* `cargo run --example flexi-net-implicit -- --host 172.31.19.210`
* `cargo run --example flexi-net-implicit -- --count-delay 3000`

Flags: `--host` (`172.31.19.210` by default), `--rpi` (milliseconds, both directions, 20 by
default), `--count-delay` (milliseconds, rounded up to whole RPIs, 1000 by default).

### Writing a device's assemblies

Assemblies are plain `binrw` structs written by hand in `scanner/assemblies/<device>/`, so they
keep groups, named bits, enums and helpers. The same struct then decodes an explicit
Get_Attribute_Single reply (`decode_reply`) and the data of an implicit input packet
(`T::read_le` on the bytes `t2o::accept_t2o_packet` returns), and is sent as implicit outputs with
`o2t::build_o2t_packet(.., CipDataOpt::Typed(Box::new(outputs)), ..)`.

The members of an `AssemN` entry are `bits, ParamN` pairs, and each `ParamN` gives the member's
type, name and units (`eds.assemblies` and `eds.params` hold both, read). Write the structs from
them, or hand the EDS to Claude to write them; give reserved bytes a named `_reserved` field so
every byte shows up in the struct, and add a test against a capture of the real device.
