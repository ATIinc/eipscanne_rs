# Phase 1 — explicit-messaging groundwork

**Status:** In review (branch `feat/SW-4573-1-explicit-groundwork`)

## Goal

Generalize the explicit-messaging code so the Connection Manager traffic of the next phases fits
without rewrites, while keeping every existing byte-exact test green. No new protocol features.

## Scope

* **Dependencies** — bump `bilge` 0.2 → 0.5 (the `Number` prelude import is gone and generated
  `new` constructors are private by default), `tokio` 1.43 → 1.53, `clap` 4.5 → 4.6,
  `pretty-hex` 0.4.2.
* **Named bitfield construction** — all bitfields (`ServiceContainer`, `LogicalPathDefinition`,
  `IdentityStatusBits`, the example's `DigitalOutputs` and `ConfigRegisterData`) derive
  `BuilderBits`, and `DefaultBits` where every field defaults to zero; call sites use
  `Type::builder().field(value)....build()` or `Type::default()` instead of the positional `new`.
  `ServiceContainer::new_request(code)` / `new_response(code)` wrap the builder for the common case.
* **Service codes** — `ServiceCode` gains the Connection Manager services: `ForwardClose` (0x4E),
  `UnconnectedSend` (0x52), `ForwardOpen` (0x54), `GetConnectionData` (0x56),
  `SearchConnectionData` (0x57), `GetConnectionOwner` (0x5A), `LargeForwardOpen` (0x5B).
* **General status** — `ResponseStatusCode` lists every general status code (0x00–0x2B) and keeps
  unknown codes as `Unknown(u8)`. `ResponseData` now parses the additional status words
  (`additional_status: Vec<u16>`, one per `additional_status_size`) that error replies carry, e.g.
  the extended status of a failed `Forward_Open`.
* **EPATH** — `src/cip/path.rs`:
  * all segment types (`PortSegment`, `LogicalSegment`, `NetworkSegment`, `SymbolicSegment`,
    `DataSegment`) and all logical segment types (`ClassId`, `InstanceId`, `MemberId`,
    `ConnectionPoint`, `AttributeId`, `Special`, `ServiceId`, `Reserved`);
  * `CipPath` is now a list of logical segments (any length) instead of a fixed
    class/instance/attribute shape: it reads with the path size in words, keeps the
    `new` / `new_full` constructors, adds `new_assembly_connection` (the usual
    `config instance / O->T connection point / T->O connection point` path) and
    `class_id()` / `instance_id()` / `attribute_id()` accessors. Only logical segments are
    modelled; application-defined content (such as configuration data) is not part of the path
    type, following the same rule as assemblies: the caller declares it and passes it in;
  * `write_path_with_word_size`, a reusable `write_with` function that prefixes a path with its
    size in 16-bit words (used by the request path today, by `Forward_Open` / `Forward_Close`
    next);
  * `LogicalPathSegment` rejects bytes whose segment type is not "logical" instead of
    misinterpreting them.
* **Common Packet Format** — `src/eip/description.rs`, `src/eip/command.rs`,
  `src/object_assembly.rs`:
  * `CommonPacketItemId` keeps unknown IDs (`Unknown(u16)`) so unexpected items are skipped by
    length instead of failing the packet;
  * `CommonPacketItem` / `CommonPacketItemData`: a typed item (descriptor + data) for the items
    that follow the address and data items — O->T and T->O Sockaddr Info plus a raw fallback;
  * `src/eip/sockaddr.rs` (implicit-messaging only): `SockaddrInfo` (family, port, address in big
    endian; zero padding) with conversions from and to `SocketAddrV4`, and the Sockaddr Info item
    constructors;
  * `RRPacketData::item_count` is a real field now; on write it is derived from the number of items
    actually serialized;
  * `RequestObjectAssembly` / `ResponseObjectAssembly` carry `additional_items` after the CIP
    message, and the write path serializes the CIP message and the trailing items separately so the
    encapsulation length and the Unconnected Data Item length stay correct (the write arguments
    threaded through `EnIpPacketDescription` → `CommandSpecificData` → `RRPacketData` are now
    `(unconnected_data_length, trailing_items_length, item_count)`).
* **README** — "Related projects" section.

## Tests

* Existing suites unchanged apart from the new struct literal fields.
* `tests/test_common_packet.rs` — Sockaddr Info byte order, Sockaddr Info items, unknown items,
  a reply with three items round-tripped byte-for-byte.
* `tests/test_epath.rs` — assembly connection path, 16-bit class/instance path, data segment,
  rejection of unsupported segment types and of segments overrunning the declared length.
* `tests/test_general_status.rs` — additional status words, unknown general status, service codes.

## Verification

```
cargo fmt --check && cargo clippy --all-targets && cargo test --all && cargo test --examples && cargo build --features async
```

Pre-existing clippy style warnings (`Into` impls, `-1 *`, `if let Err` blocks) are left untouched.
