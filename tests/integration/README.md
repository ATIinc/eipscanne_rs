# Integration Testing with OpENer

The examples run against [OpENer](https://github.com/EIPStackGroup/OpENer), an open source
EtherNet/IP adapter, in a Docker container. `OpENer/Dockerfile` builds its POSIX sample
application, which copies the outputs it receives on assembly 150 into the inputs it sends from
assembly 100, so every input packet echoes the last output packet.

## Start the adapter

On the host (the devcontainer has no Docker):

```
tests/integration/start-opener.sh
```

It builds the `eip-adapter` image and runs the adapter in the foreground at `172.28.0.10` on the
`eip-network` Docker network, creating the network when it is missing (Docker only assigns a fixed
address on a network of its own). The adapter prints nothing while it runs; Ctrl+C stops it.

The devcontainer runs with `--network=host`, and the host routes `172.28.0.0/16` to the network's
bridge, so the scanner reaches the adapter over TCP 44818 and UDP 2222 with nothing to configure.

## Run the examples

In the devcontainer; every example that can talk to OpENer defaults to `172.28.0.10`:

* `cargo run --example read-identity`: registers a session, prints the adapter's identity
  (`Product Name: "OpENer PC"`) and unregisters
* `cargo run --example read-identity-connected`: opens a class 3 connection to the Message
  Router, reads the identity over it five times (Send Unit Data, sequence counts 1 to 5) and
  closes it
* `cargo run --example implicit-io`: opens a class 1 connection (configuration assembly 151,
  output 150, input 100, 32 bytes each, a 1 s packet interval), prints a `SENT` line per cycle and
  a `RECEIVED` line per input packet carrying the same bytes, then closes the connection and
  unregisters. `--rpi 100 --cycles 50` for a faster exchange, `--large` for a Large_Forward_Open
* `cargo run --example eds-implicit-io`: the same connection, read from the `eds_parser` crate's
  `sample_adapter.eds` fixture. Without `--run` the outputs are sent idle (run flag cleared);
  OpENer still echoes them

In Wireshark: a Forward Open request and reply on TCP 44818, Connected Data Items on UDP 2222 in
both directions (decoded once Wireshark has seen the Forward Open), a Forward Close request and
reply, then Unregister Session.
