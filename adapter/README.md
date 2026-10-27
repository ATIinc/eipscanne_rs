# adapter

The adapter side of EtherNet/IP, built on `eipscanne_rs`: an explicit messaging server over TCP 44818. A device implements `device::Device`, and `server::serve` takes care of the encapsulation session and the Message Router:

* `server`: accepts connections, registers sessions and hands Get_Attribute and Set_Attribute requests, Single and All, to the device.
* `device`: the trait a device implements, and `decode_request` for the data of a Set.
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
