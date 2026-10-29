# adapter

The adapter side of EtherNet/IP, built on `eipscanne_rs`: explicit messaging over TCP 44818, and one class 1 I/O connection at a time over UDP. A device implements `message_router::MessageRouter`, and `server::serve` takes care of the encapsulation session and of decoding each request. A device that also implements `implicit::ImplicitDevice` is served by `server::serve_implicit`, which adds Forward_Open, Forward_Close and the I/O:

* `server`: accepts connections, registers sessions and hands each Send RR Data request to the Message Router.
* `message_router`: the `MessageRouter` trait, the services a scanner's requests reach.
* `explicit`: `decode_data` for the data of a Set or a connection's outputs, and the decoding and answering of each request.
* `implicit`: the `ImplicitDevice` trait, and the opening, exchange and closing of the class 1 connection.
* `packet`: reads and writes one encapsulation packet on a stream, and finds the request it carries.
* `error`: the one error type of every fallible call.

## Sample ClearLink

`examples/sample-clearlink-adapter` serves a sample Teknic ClearLink on `127.0.0.1:44818`: an Identity object and the configuration, output and input assemblies. Its inputs report an output as driven while it is on or has a PWM duty cycle. Start it in one terminal and run the scanner's examples against it from another:

```text
cargo run --example sample-clearlink-adapter
cargo run --example read-identity -- --host 127.0.0.1
cargo run --example clearlink-explicit-outputs -- --host 127.0.0.1 --index 4 --on
```

`read-identity` prints `Product Name: "Sample Clearlink"`; `clearlink-explicit-outputs` prints `IO-4: driven true (expected true), overloaded false`. `--host 0.0.0.0` serves other hosts too. The adapter serves unconnected messaging only, so `read-identity-connected` is refused.

## Sample implicit adapter

`examples/sample-implicit-adapter` serves an Identity object and the OpENer sample's assemblies on `127.0.0.1:44818`: configuration 151, output 150 and input 100, 32 bytes each. Its inputs echo the last outputs. The scanner binds UDP 2222, so the adapter takes its outputs on 2223 and names that port in the Forward_Open reply:

```text
cargo run --example sample-implicit-adapter
cargo run --example read-identity -- --host 127.0.0.1
cargo run --example opener-implicit -- --host 127.0.0.1
```
