# Examples

The examples live in the `scanner` crate, which also holds what they are built on: the
encapsulation `session` (shared), `explicit` messaging (one request, one reply) and `implicit`
messaging (a class 1 I/O connection, one submodule per stage). `read-identity` and
`write-clearlink-io` use `session` and `explicit`; `implicit-io` uses `session` and `implicit`.

## Run Examples

From the repository root: `cargo run --example <name> -- <arguments>` (the root `Cargo.toml`
lists the scanner among the workspace's default members, so no `-p scanner` is needed).

## Test Examples

`cargo test --examples` runs the byte-exact tests of the ClearLink assemblies
in `write-clearlink-io` (they also run as part of `cargo test --workspace`).

## Examples Explained

| Example | Device | Messaging | Moves hardware |
|---|---|---|---|
| `read-identity` | any adapter | explicit | no |
| `write-clearlink-io` | Teknic ClearLink | explicit | digital outputs |
| `write-nitra-io` | Nitra pneumatic valve manifold | explicit | solenoid valves |
| `clearlink-homing` | Teknic ClearLink | explicit (polling) | a motor |
| `io-hub-homing` | Teknic IO-HUB-4-E / ClearPath-IP | explicit (polling) | a motor |
| `implicit-io` | OpENer or any class 1 adapter | implicit | outputs of the adapter |

Every example is a single file. The device assemblies they use live outside the library in
`scanner/assemblies/`: `clearlink.rs` (config, input, output), `io_hub.rs` (input, output), each
with a directory of the same name, and `nitra.rs`. An example that needs them declares
`#[path = "../assemblies"] mod assemblies { pub mod clearlink; }` and imports
`assemblies::clearlink::...`. Application data is always declared by the caller as plain `binrw`
structs; the library never models it. Each request is written out where it happens: the assembly
paths are built once at the top of `main` and passed to `send_request`.

### read-identity

Requests the Identity object from the connected device.

i.e. `cargo run --example read-identity -- --host 172.28.0.10`

1. Registers a session (`Session::register`)
1. Reads the Identity object (`explicit::read_identity`: Get_Attributes_All, reply decoded as
   `IdentityResponse`)
1. Unregisters the session

### write-clearlink-io

Reads from and writes to a Teknic ClearLink motor controller board using the assembly objects
defined in Teknic's EtherNet/IP Object Reference:
https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=18

i.e. `cargo run --example write-clearlink-io -- --help`
* `cargo run --example write-clearlink-io -- --index 4 --on`
* `cargo run --example write-clearlink-io -- --index 4 --off`
* `cargo run --example write-clearlink-io -- --index 4 --pwm 100`
* `--host <ip>` picks another ClearLink than the default one

1. Parses the desired digital output to be modified from the command line
1. Registers a session
1. Writes the ConfigAssembly object (`explicit::send_request`, Set_Attribute_Single; a reply with
   a general status other than success is an error)
1. Requests the OutputAssembly object and decodes its data (`explicit::decode_reply`)
1. Modifies the value of the appropriate digital output (from the command line)
1. Writes the modified OutputAssembly object
1. Unregisters the session

### write-nitra-io

Energizes or releases solenoid valves on a Nitra EtherNet/IP pneumatic valve manifold
(https://cdn.automationdirect.com/static/manuals/nitrainserts/nitra_ump_ethernetip.pdf#page=6).
`--host` is required.

* `cargo run --example write-nitra-io -- --host 172.31.19.60 --valves 0 2 --on`
* `cargo run --example write-nitra-io -- --host 172.31.19.60 --valves 0 2 --off`
* `cargo run --example write-nitra-io -- --host 172.31.19.60 --valves 7 --pulse 800`

1. Registers a session
1. Reads the status byte (assembly 101)
1. Writes the 16 valve bits (assembly 100); `--pulse <ms>` writes them on, waits, and writes
   them all off again (Ctrl+C during the wait releases them early)
1. Unregisters the session

### clearlink-homing

Homes one motor of a Teknic ClearLink over explicit messaging, top to bottom like the other
explicit examples. This moves a real motor, so `--host` is required.

* `cargo run --example clearlink-homing -- --host 172.31.19.14 --motor 1 --home-sensor 6`
* `--home-sensor -1` (the default) homes against a hard stop; `--velocity` and `--acceleration`
  set the move, `--timeout-s` how long the whole homing may take

1. Registers a session and reads the output assembly, so writes change only the motor being homed
1. Reads the inputs and, if shutdowns or a motor fault are present, raises Clear Alerts and Clear
   Motor Fault briefly and lowers them again
1. Writes the configuration assembly with homing enabled and the home sensor connector
1. Enables the motor and waits for `ready_to_home`
1. Starts the homing move (homing + load velocity move flags, jog velocity, limits), waits for
   the acknowledgement, clears the flags, waits for `has_homed`
1. Disables the motor, however steps 4 and 5 ended (done, failed or Ctrl+C), and unregisters the
   session

### io-hub-homing

Homes one ClearPath-IP motor on a Teknic IO-HUB-4-E over explicit messaging, top to bottom like
the other explicit examples. This moves a real motor, so `--host` is required. `--repeat N` sends
N homing commands back to back, each before the previous one has finished, which reproduces a
firmware bug (the move is cancelled, `has_homed` never asserts, or the motor faults, depending on
the version); the status line printed while waiting shows the relevant bits and decodes a
rejected command's AOI error code.

* `cargo run --example io-hub-homing -- --host 172.31.19.18 --motor 0`
* `cargo run --example io-hub-homing -- --host 172.31.19.18 --motor 0 --repeat 4 --delay-ms 10`

1. Registers a session
1. Reads the inputs and, if a shutdown is present, raises Shutdown Reset briefly and lowers it again
1. Enables the motor
1. Sends the homing command(s), each with the next move number
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
   soon as it has replied; then sends the Forward_Open built from a `ConnectionConfig`
   (`implicit::forward_open`) and keeps the reply as an `OpenConnection`
1. **Exchange**: one `tokio::select!` loop over three events
    * the send timer fires every O->T packet interval: `Producer::next_packet` frames the outputs
      (sequence numbers, run/idle header) and `send_io_packet` sends them to the O->T endpoint
    * a datagram arrives: `Consumer::accept` checks the connection ID, the sender, the sequence
      number and the size, decodes the inputs and prints them, or says why the packet was
      discarded
    * `Consumer::deadline` passes: no input packet arrived within timeout multiplier × packet
      interval, the connection is considered timed out and the loop ends
    * Ctrl+C: the loop ends early; the connection is still closed and the session unregistered
1. **Close**: sends the Forward_Close matching the Forward_Open (`forward_close`)
1. **End session**: unregisters the session

Production code may run the producer on a task of its own so a slow input handler can never
delay an output packet; the example keeps one loop to stay readable.

Limits: one connection per host (only one socket can bind UDP 2222) and point-to-point in both
directions (no multicast).
