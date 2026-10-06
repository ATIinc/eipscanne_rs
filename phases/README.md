# Implicit messaging phases (SW-4573)

Implicit (class 1 cyclic I/O) messaging is being added to `eipscanne_rs` as a stack of small pull
requests. Each phase has its own document in this directory so the work can be picked up again at any
point. Update the **Status** line of a phase when it changes.

| Phase | Document | Status |
|---|---|---|
| 1 | [Explicit-messaging groundwork](01-explicit-messaging-groundwork.md) | In review |
| 2 | [Connection Manager packets](02-connection-manager-packets.md) | Implemented, awaiting review |
| 3 | [Class 1 I/O packets](03-class1-io-packets.md) | Delivered with phase 2 (PR #4) |
| 4 | [`scanner` crate: open a connection and exchange I/O](04-scanner-crate.md) | Implemented, awaiting review |
| 5 | [EDS parser crate](05-eds-parser.md) | Not started |
| 6 | [Safety System validation](06-safety-system-validation.md) | Not started |

## How the stack works

* Integration branch: `feat/SW-4573-implicit-messaging`.
* One branch per phase (`feat/SW-4573-<n>-<name>`), each based on the previous phase's branch, each
  opened as its own pull request so reviews stay small.
* Every phase keeps `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test --all`,
  `cargo test --all --features adapter` and `cargo test --examples` green (from phase 4 on, the
  workspace equivalents listed in that phase).

## Ground rules

* **Packets only in the library.** `eipscanne_rs` stays a packet (de)serialization library built
  on `binrw` + `bilge`. Sessions, sockets, timers, connection state and the examples live in the
  `scanner` workspace crate (phase 4): the shared `session`, then `explicit` and `implicit`
  messaging kept apart, the latter organized by protocol stage, so it reads as a reference for
  production code.
* **Application data is declared by the caller.** Assemblies, configuration data and other
  device-specific payloads are plain `binrw` structs in the caller's code (see `examples/`) and are
  passed in as `CipData`; the library frames them but never models their content. Keep the protocol
  types to what the baseline actually sends and parses instead of covering the whole specification.
* **Wireshark naming.** Struct and field names follow the names Wireshark shows for the same bytes
  (`enip.*` and `cip.cm.*` fields), in `snake_case`.
* **Path-based module layout.** Modules are declared in `src/<name>.rs` with their submodules in
  `src/<name>/`; no `mod.rs` files.
* **Bitfields are built with their builders.** Every `bilge` bitfield derives `BuilderBits` and is
  assembled with `Type::builder().field(value)....build()`, each field set exactly once, reserved
  bits zero, in the library and in the tests alike; a bitfield is never assembled with
  `Type::default()` and the `set_*` setters, and the positional `new(...)` constructor is never
  called. bilge 0.5 gives the builder the visibility of that `new`, so a bitfield built outside its
  module carries `new = pub` for the sole purpose of exposing its builder. A bitfield only built
  inside its module, through a wrapper such as `ServiceContainer::new_request`, keeps `new` private.
* **EIPScanner is a loose reference only.** Behaviour follows the specification; code is
  structured for a human reader, not after EIPScanner's classes.
* **Tests are byte-exact.** Every packet type gets serialization and deserialization tests against
  byte arrays documented with Wireshark's dissection of those bytes: run `scripts/dissect.sh` (tshark)
  on the bytes and paste its output into the test comment, so the comment is never hand-written.
  Captures from real devices replace hand-assembled vectors as they become available.
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
