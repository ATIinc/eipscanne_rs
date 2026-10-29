# adapter

The adapter side of EtherNet/IP, built on `eipscanne_rs`: explicit messaging over TCP 44818, one class 3 connection per session, and class 1 I/O connections over UDP (one at a time unless the device allows more). A device implements `message_router::MessageRouter`, and `server::serve` takes care of the encapsulation session and of decoding each request. With `message_router::ExplicitOptions::accepts_explicit_connections` set, `serve` also takes class 3 connections. A device that also implements `implicit::ImplicitDevice` is served by `server::serve_implicit`, which adds class 1 Forward_Open, Forward_Close and the I/O:

* `server`: accepts connections, registers sessions, hands Send RR Data to `explicit::unconnected` and Send Unit Data to `explicit::connected`, and closes a session without encapsulation activity for its inactivity timeout.
* `message_router`: the `MessageRouter` trait, the services a scanner's requests reach, and the paths, connection parameters and responses of those services.
* `explicit`: the decoding and answering of each request; `unconnected` answers Send RR Data, `connected` the session's class 3 connection.
* `implicit`: the `ImplicitDevice` trait, and the opening, exchange and closing of the class 1 connections.
* `packet`: reads and writes one encapsulation packet on a stream; `decode_data` decodes the data of a Set or a connection's outputs.
* `error`: the one error type of every fallible call.

[src/README.md](src/README.md) follows a packet through these modules.

## Sample IO-HUB

`examples/sample-io-hub-adapter` serves a sample Teknic IO-HUB-4-E on `127.0.0.1:44818`: an Identity object and the hub's output (101) and input (100) assemblies, over explicit messaging, a class 3 connection or a class 1 connection with I/O on UDP 2223. Its motors enable when told, home about 2 s after a homing move, and turn in short spurts while a class 1 connection runs. Start it in one terminal and run the scanner's examples against it from another:

```text
cargo run --example sample-io-hub-adapter
cargo run --example read-identity -- --host 127.0.0.1
cargo run --example read-identity-connected -- --host 127.0.0.1
cargo run --example io-hub-explicit-homing -- --host 127.0.0.1 --motor 0
cargo run --example io-hub-eds-implicit -- --host 127.0.0.1 --motor 0
```

`read-identity` prints `Product Name: "Sample IO-HUB"`; `io-hub-explicit-homing` prints `HOMED`; `io-hub-eds-implicit` prints a burst of `position=` lines every 4 s. `--host 0.0.0.0` serves other hosts too.

## Sample implicit adapter

`examples/sample-implicit-adapter` serves an Identity object and the OpENer sample's assemblies on `127.0.0.1:44818`: configuration 151, output 150 and input 100, 32 bytes each. Its inputs echo the last run outputs, and it accepts a class 3 connection. The scanner binds UDP 2222, so the adapter takes its outputs on 2223 and names that port in the Forward_Open reply:

```text
cargo run --example sample-implicit-adapter
cargo run --example read-identity -- --host 127.0.0.1
cargo run --example read-identity-connected -- --host 127.0.0.1
cargo run --example opener-implicit -- --host 127.0.0.1
cargo run --example opener-eds-implicit -- --host 127.0.0.1 --run
```

Both OpENer examples print their outputs echoed back as `RECEIVED` lines. Without `--run`, `opener-eds-implicit` sends its outputs idle and the adapter does not apply them. `opener-implicit --toggle-every 3` switches between run and idle every three cycles: while idle the echo holds the last run outputs and the adapter logs `outputs idle`.
