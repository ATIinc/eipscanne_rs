# eipScanne-RS (Ethernet/IP Scanner - rust)

`eipscanne_rs` reads and writes EtherNet/IP packets with [`binrw`](https://docs.rs/binrw) and [`bilge`](https://docs.rs/bilge); its types follow Wireshark's names for the same bytes, and its [tests](./tests/) check them against known packet bytes. [captures](./captures/) holds Wireshark captures of explicit messaging and how to take more.

* [`scanner`](./scanner/): sessions, explicit and implicit messaging on top of the library, and the examples.
* [`adapter`](./adapter/): an explicit messaging (class 3 included) and class 1 I/O server for a device's handler, and sample adapters.
* [`eds_parser`](./eds_parser/): reads a device's EDS file into its params, assemblies and connections.
* [`eds_parser_derive`](./eds_parser_derive/): the derives of `eds_parser`'s entry structs and fields.
* [`hex_assert`](./hex_assert/): hex assertions for the byte-exact tests.

## Related projects

None of their code is used here:

* [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) (C++, [ATI fork](https://github.com/ATIinc/EIPScanner)): the scanner whose Wireshark captures the packet types follow.
* [OpENer](https://github.com/EIPStackGroup/OpENer) (C): the adapter of the integration tests in [tests/integration](./tests/integration/).
* [EthernetIpRust](https://github.com/CristianMori/EthernetIpRust) (Rust): async scanner and adapter crates with class 1 I/O.
* [rseip](https://github.com/Joylei/eip-rs) (Rust): an explicit messaging client with class 3 connected messaging.

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT) at your option.
