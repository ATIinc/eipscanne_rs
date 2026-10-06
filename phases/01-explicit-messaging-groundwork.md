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
* **Common Packet Format** — `src/eip/description.rs`, `src/eip/command.rs`, `src/eip/packet.rs`,
  `src/object_assembly.rs`. Every address and data item of a packet is modelled the same way, as an
  item of one list:
  * `CommonPacketItemId` keeps unknown IDs (`Unknown(u16)`) so unexpected items are skipped by
    length instead of failing the packet;
  * `CommonPacketItem`: one enum for every item — Null Address Item, Unconnected Data Item
    (carrying a `CipMessage`), Socket Address Info O->T and T->O, and a raw `Unknown` fallback;
    variant and Type ID names follow Wireshark. The Type ID and Length are derived from the variant
    on write; an item whose data does not fit its variant (including an unparsable CIP message) is
    read as `Unknown` and re-serialized unchanged;
  * `CipMessage` (`src/cip/message.rs`): `Request(MessageRouterRequest)` or
    `Response(MessageRouterResponse)`, chosen on read by the Request/Response bit of the service
    code byte, so no type is generic over the message;
  * `src/eip/sockaddr.rs` (implicit-messaging only): `SockaddrInfo` (family, port, address in big
    endian; zero padding) with conversions from and to `SocketAddrV4`, and the Sockaddr Info item
    constructors;
  * `RRPacketData` holds the interface handle, the timeout and `items`; the item count is read
    from the wire and written from `items.len()`. `CommonPacketDescriptor`, `BASE_ITEM_COUNT` and
    the length write arguments are gone;
  * `EnIpPacket` (was `EnIpPacketDescription`) is the whole packet: header plus command specific
    data; the header length is computed on write. `src/eip/packet.rs` starts with a map from the
    Wireshark tree to the Rust fields. `read_request` / `read_response` fail when a SendRRData
    packet does not carry the expected message, while plain `read` keeps it as an `Unknown` item.
    `RequestObjectAssembly` / `ResponseObjectAssembly` are both aliases of `EnIpPacket` that only
    document the direction, with `cip_message()`, `request()`, `response()`, `items()`,
    `sockaddr_info_items()` and `with_item()`.
* **README** — "Related projects" section.

## Tests

* Existing suites keep their expected bytes; struct literals now build the item list
  (`RRPacketData::new_unconnected`, `RequestObjectAssembly::new_send_rr_data`). Tests that
  serialized or read only the header and command specific data now include the Unconnected Data
  Item data from the same capture, since it is part of the packet.
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
