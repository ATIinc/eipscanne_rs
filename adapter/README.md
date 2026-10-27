# adapter

The adapter side of EtherNet/IP, built on `eipscanne_rs`: an explicit messaging server over TCP 44818. A device implements `handler::Handler`, and `server::serve` takes care of the encapsulation session and the Message Router:

* `server`: accepts connections, registers sessions and hands Get_Attribute and Set_Attribute requests, Single and All, to the handler.
* `handler`: the trait a device implements, and `decode_request` for the data of a Set.
* `reply`: the Message Router replies, a success carrying data or a refusal.
* `packet`: reads and writes one encapsulation packet on a stream, and finds the request it carries.
* `error`: the one error type of every fallible call.

`examples/identity-device` serves an Identity object and one output assembly; the scanner's `read-identity` reads it:

```text
cargo run --example identity-device
cargo run --example read-identity -- --host 127.0.0.1
```
