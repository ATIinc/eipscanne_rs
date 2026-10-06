# eipScanne-RS (Ethernet/IP Scanner - rust)

This repository is an implementation of the Ethernet/IP **Explicit Messaging** protocol.

This was created by using the [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) library to communicate with an Ethernet/IP Adapter while monitoring the network traffic with Wireshark.

The struct definitions/names heavily correlate to their Wireshark counterparts. See the [captures](./captures/) directory for some examples of Ethernet/IP traffic. 

See the [examples](./examples/) directory for ideas on how to implement an Ethernet/IP Explicit Messaging Scanner

## Reading PDF references

Reference PDFs (e.g. protocol specifications) can be kept in the git-ignored [docs](./docs/) directory. The devcontainer installs `poppler-utils` for working with them:

* `scripts/pdf-to-text.sh <input.pdf> [output.txt]` extracts the text layer into a text file with a `=====PAGE n=====` marker per page, which is easier to search with `grep` than the PDF itself. PDFs whose text is drawn with embedded glyph fonts are decoded through the fonts' Unicode tables; scanned PDFs only give text if they have an OCR layer.
* `pdftoppm` also lets Claude Code render PDF pages directly, which helps with tables and diagrams that don't survive text extraction.

## Implicit messaging

Class 1 implicit messaging (cyclic I/O) is being added in stages; the plan, the ground rules and the status of each stage live in [phases/](./phases/).

## Related projects

Other implementations that were reviewed while planning the implicit messaging work. None of their code is used here, but they are useful references and possible interoperability test targets:

* [EIPScanner](https://github.com/nimbuscontrols/EIPScanner) (C++, [ATI fork](https://github.com/ATIinc/EIPScanner)) — the scanner this library was originally modelled on; its `ImplicitMessagingExample` is the behavioural reference for class 1 connections.
* [OpENer](https://github.com/EIPStackGroup/OpENer) (C) — open source adapter used for the integration tests in [tests/integration](./tests/integration/).
* [EthernetIpRust](https://github.com/CristianMori/EthernetIpRust) (Rust, Apache-2.0) — async scanner/adapter crates (`ethernetip-core`, `ethernetip-connections`) with class 1 I/O. Its `echo-adapter` sample could serve as a future loopback interoperability target.
* [rseip](https://github.com/Joylei/eip-rs) (Rust, MIT) — explicit messaging client with class 3 connected messaging.
