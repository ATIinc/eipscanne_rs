# scanner

The scanner side of EtherNet/IP, built on `eipscanne_rs`, written to be read top to bottom. The library only (de)serializes packets; sockets, timers and state live here:

* `session`: the encapsulation session over TCP 44818, shared by both kinds of messaging.
* `explicit`: unconnected messaging, one request and one reply; `explicit::connected` a class 3 connection.
* `implicit`: a class 1 I/O connection, `connection` plus one module per direction (`o2t`, `t2o`); `implicit::eds` opens one from an EDS file read by `eds_parser`.
* `error`: the one error type of every fallible call.

The [examples](./examples/) run from the repository root with `cargo run --example <name>`; [tests/integration](../tests/integration/) runs them against an OpENer adapter.
