# eipScanne-RS (Ethernet/IP Scanner - rust)

`eipscanne_rs` reads and writes Ethernet/IP packets with [`binrw`](https://docs.rs/binrw) and [`bilge`](https://docs.rs/bilge): the encapsulation session and **explicit messaging**. Sockets and state belong to the caller.

This was created by using the [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) library to communicate with an Ethernet/IP Adapter while monitoring the network traffic with Wireshark.

The struct definitions/names heavily correlate to their Wireshark counterparts. The [tests](./tests/) hold the captured packets as hex dumps.

See the [examples](./examples/) directory for ideas on how to implement an Ethernet/IP Explicit Messaging Scanner

## Related projects

Other implementations that were reviewed while planning the implicit messaging work. None of their code is used here, but they are useful references and possible interoperability test targets:

* [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) (C++, [ATI fork](https://github.com/ATIinc/EIPScanner)) — used to capture the Wireshark traffic the packet types follow.
* [OpENer](https://github.com/EIPStackGroup/OpENer) (C) — open source adapter used for the integration tests in [tests/integration](./tests/integration/).
* [EthernetIpRust](https://github.com/CristianMori/EthernetIpRust) (Rust, Apache-2.0) — async scanner/adapter crates (`ethernetip-core`, `ethernetip-connections`) with class 1 I/O. Its `echo-adapter` sample could serve as a future loopback interoperability target.
* [rseip](https://github.com/Joylei/eip-rs) (Rust, MIT) — explicit messaging client with class 3 connected messaging.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
* MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
