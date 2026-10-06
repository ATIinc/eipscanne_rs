# Implicit messaging phases (SW-4573)

Implicit (class 1 cyclic I/O) messaging is being added to `eipscanne_rs` as a stack of small pull
requests. Each phase has its own document in this directory so the work can be picked up again at any
point. Update the **Status** line of a phase when it changes.

| Phase | Document | Status |
|---|---|---|
| 1 | [Explicit-messaging groundwork](01-explicit-messaging-groundwork.md) | In review |
| 2 | [Connection Manager packets](02-connection-manager-packets.md) | Implemented, awaiting review |
| 3 | [Class 1 I/O packets](03-class1-io-packets.md) | Not started |
| 4 | [I/O connection state](04-io-connection-state.md) | Not started |
| 5 | [Socket utilities and implicit-io example](05-socket-utilities-and-example.md) | Not started |
| 6 | [EDS parser crate](06-eds-parser.md) | Not started |
| 7 | [Safety System validation](07-safety-system-validation.md) | Not started |

## How the stack works

* Integration branch: `feat/SW-4573-implicit-messaging`.
* One branch per phase (`feat/SW-4573-<n>-<name>`), each based on the previous phase's branch, each
  opened as its own pull request so reviews stay small.
* Every phase keeps `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test --all`,
  `cargo test --all --features adapter`, `cargo test --examples` and
  `cargo build --features async` green.

## Ground rules

* **Packets only in the library.** The crate stays a packet (de)serialization library built on
  `binrw` + `bilge`; sockets, timers and the cyclic loop live in an example and a small utility crate.
* **Application data is declared by the caller.** Assemblies, configuration data and other
  device-specific payloads are plain `binrw` structs in the caller's code (see `examples/`) and are
  passed in as `CipData`; the library frames them but never models their content. Keep the protocol
  types to what the baseline actually sends and parses instead of covering the whole specification.
* **Wireshark naming.** Struct and field names follow the names Wireshark shows for the same bytes
  (`enip.*` and `cip.cm.*` fields), in `snake_case`.
* **Path-based module layout.** Modules are declared in `src/<name>.rs` with their submodules in
  `src/<name>/`; no `mod.rs` files.
* **Bitfields are built by name.** Every `bilge` bitfield derives `BuilderBits`
  (`Type::builder().field(value)....build()`, each field set exactly once, reserved bits zero) and,
  when every field has a zero default, `DefaultBits` (`Type::default()` plus `set_*` setters). The
  positional `new(...)` constructor stays private, and with it the builder (bilge 0.5), so outside
  the defining module bitfields are built with `Type::default()` and the setters, or through a
  wrapper such as `ServiceContainer::new_request` or `NetworkConnectionParameters::new`.
* **EIPScanner parity.** Behaviour mirrors the C++ [EIPScanner](https://github.com/nimbuscontrols/EIPScanner)
  `ConnectionManager` / `IOConnection` logic unless the specification says otherwise.
* **Tests are byte-exact.** Every packet type gets serialization and deserialization tests against
  hand-assembled byte arrays, documented with Wireshark-style dissection comments like the existing
  tests. Captures from real devices replace hand-assembled vectors as they become available.
* **Nothing in the repository references the specification documents.** The specification is only
  consulted locally; code comments describe behaviour in plain words.

## Scope decisions

* Target device: a standard EtherNet/IP adapter (the Safety System) using plain class 1 I/O.
  CIP Safety is out of scope.
* Baseline features: `Forward_Open` and `Large_Forward_Open`, class 1 cyclic, point-to-point in both
  directions, exclusive owner, 32-bit run/idle header on O->T data, `Forward_Close`.
* Not planned: multicast, configuration data segments in the connection path, class 3 connected
  explicit messaging (`SendUnitData`). The types are left extensible for them.
* The EDS parser is a separate workspace crate built on a `pest` grammar.
* Existing Rust implementations were reviewed and not adopted; see "Related projects" in the
  top-level README.
