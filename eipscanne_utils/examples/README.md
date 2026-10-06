# Examples

The examples live in the `eipscanne_utils` crate, which also holds the session, connection and
I/O helpers they are built on (see `eipscanne_utils/src/lib.rs` for the stages).

## Run Examples

From the repository root: `cargo run -p eipscanne-utils --example <name> -- <arguments>`

## Test Examples

`cargo test -p eipscanne-utils --examples` runs the byte-exact tests of the ClearLink assemblies
in `write-teknic-io` (they also run as part of `cargo test --workspace`).

## Examples Explained

### read-identity

Requests the Identity object from the connected device.

i.e. `cargo run -p eipscanne-utils --example read-identity -- 172.28.0.10`

1. Registers a session (`Session::register`)
1. Sends a request for the Identity object
1. Reads the Identity object reply (`Session::read_typed_reply`)
1. Unregisters the session

### write-teknic-io

Reads from and writes to a Teknic ClearLink motor controller board using the assembly objects
defined in Teknic's EtherNet/IP Object Reference:
https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=18

i.e. `cargo run -p eipscanne-utils --example write-teknic-io -- --help`
* `cargo run -p eipscanne-utils --example write-teknic-io -- --index 4 --on`
* `cargo run -p eipscanne-utils --example write-teknic-io -- --index 4 --off`
* `cargo run -p eipscanne-utils --example write-teknic-io -- --index 4 --pwm 100`
* `--host <ip>` picks another ClearLink than the default one

1. Parses the desired digital output to be modified from the command line
1. Registers a session
1. Writes the ConfigAssembly object and reads the success reply
1. Requests the OutputAssembly object and reads its data
1. Modifies the value of the appropriate digital output (from the command line)
1. Writes the modified OutputAssembly object and reads the success reply
1. Unregisters the session

### implicit-io

Opens a class 1 (implicit messaging) connection, exchanges cyclic I/O for a number of cycles and
closes it again. `main` is the five stages of implicit messaging in order; the defaults match the
OpENer sample application of `tests/integration`, which copies the outputs it receives (assembly
150) into the inputs it sends (assembly 100).

i.e. `cargo run -p eipscanne-utils --example implicit-io -- --help`
* `cargo run -p eipscanne-utils --example implicit-io -- --host 172.28.0.10`
* `cargo run -p eipscanne-utils --example implicit-io -- --host 172.28.0.10 --rpi 100 --cycles 50`
* `cargo run -p eipscanne-utils --example implicit-io -- --host 172.28.0.10 --large`

Flags: `--host`, `--configuration-instance`, `--output-instance`, `--input-instance`,
`--output-size`, `--input-size`, `--rpi` (milliseconds, both directions), `--cycles`, `--large`.

1. **Session**: registers a session over TCP 44818 (`Session::register`)
1. **Open**: binds the UDP I/O socket on port 2222 first, because the adapter starts sending as
   soon as it has replied; then sends the Forward_Open built from a `ConnectionConfig`
   (`forward_open`) and keeps the reply as an `OpenConnection`
1. **Exchange**: one `tokio::select!` loop over three events
    * the send timer fires every O->T packet interval: `Producer::next_packet` frames the outputs
      (sequence numbers, run/idle header) and `send_io_packet` sends them to the O->T endpoint
    * a datagram arrives: `Consumer::accept` checks the connection ID, the sender, the sequence
      number and the size, decodes the inputs and prints them, or says why the packet was
      discarded
    * `Consumer::deadline` passes: no input packet arrived within timeout multiplier × packet
      interval, the connection is considered timed out and the loop ends
1. **Close**: sends the Forward_Close matching the Forward_Open (`forward_close`)
1. **End session**: unregisters the session

Production code may run the producer on a task of its own so a slow input handler can never
delay an output packet; the example keeps one loop to stay readable.

Limits: one connection per host (only one socket can bind UDP 2222) and point-to-point in both
directions (no multicast).
