# adapter

The adapter side of EtherNet/IP, built on `eipscanne_rs`: explicit messaging over TCP 44818. A device implements `message_router::MessageRouter`, and `server::serve` takes care of the encapsulation session and of decoding each request:

* `server`: accepts connections, registers sessions, hands Send RR Data to `explicit::unconnected`, and closes a session without encapsulation activity for its inactivity timeout.
* `message_router`: the `MessageRouter` trait, the services a scanner's requests reach, and the paths and responses of those services.
* `explicit`: the decoding and answering of each request; `unconnected` answers Send RR Data.
* `packet`: reads and writes one encapsulation packet on a stream; `error_reply` refuses one; `decode_data` decodes the data of a Set.
* `error`: the one error type of every fallible call.

[src/README.md](src/README.md) follows a packet through these modules.

## Sample ClearLink

`examples/sample-clearlink-adapter` serves a sample Teknic ClearLink on `127.0.0.1:44818`: an Identity object and the configuration, output and input assemblies. Its inputs report an output as driven while it is on or has a PWM duty cycle. Start it in one terminal and run the scanner's examples against it from another:

```text
cargo run --example sample-clearlink-adapter
cargo run --example read-identity -- --host 127.0.0.1
cargo run --example clearlink-explicit-outputs -- --host 127.0.0.1 --index 4 --on
```

`read-identity` prints `Product Name: "Sample Clearlink"`; `clearlink-explicit-outputs` prints `IO-4: driven true (expected true), overloaded false`. `--host 0.0.0.0` serves other hosts too. The adapter serves unconnected messaging only, so `read-identity-connected` is refused.
