# Phase 1 — explicit-messaging groundwork

## Goal

The explicit-messaging packet types that the Connection Manager traffic of the later phases is
built on: a path of any number of logical segments, a Common Packet Format modelled as one list of
items, a CIP message that is either a request or a response, rejected requests as plain data, named
constants, and bitfields built with their builders. The phase contains no Connection Manager
packet and no other protocol feature.

## Scope

* **Dependencies** — `bilge` 0.5 (bitfield builders through `BuilderBits`; generated `new`
  constructors are private unless declared `new = pub`), `binrw` 0.15; dev-dependencies `tokio`
  1.53, `clap` 4.6 and `pretty_assertions` 1.4; the `hex_test_macros` crate depends on
  `pretty-hex` 0.4.2. Both crates use Rust edition 2024; `eipscanne_rs` is version 0.3.0.
* **Module layout** — `src/cip.rs`, `src/cip/message.rs` and `src/eip.rs` declare their submodules,
  which live in `src/cip/`, `src/cip/message/` and `src/eip/`; there are no `mod.rs` files.
* **Named constants** — `src/cip/object_ids.rs` holds the Identity, Assembly and Connection Manager
  class, instance and attribute IDs. `src/eip/constants.rs` holds the TCP port 44818, the I/O UDP
  port 2222 and the encapsulation values the packet constructors use (`EMPTY_SENDER_CONTEXT`,
  `DEFAULT_ENCAPSULATION_OPTIONS`, `CIP_INTERFACE_HANDLE`, `NO_ENCAPSULATION_TIMEOUT`,
  `UNREGISTERED_SESSION_HANDLE`, the Register Session protocol version and option flags).
* **Named bitfield construction** — every bitfield (`ServiceContainer`, `LogicalPathDefinition`,
  `IdentityStatusBits`, and the example's `DigitalOutputs` and `ConfigRegisterData`) derives
  `BuilderBits`, and `DefaultBits` where every field defaults to zero. A bitfield is built with
  `Type::builder().field(value)....build()`, or `Type::default()` for an all-zero value; the
  positional `new` is never called. bilge 0.5 gives the builder the visibility of `new`, which is
  private for all of these, so `builder()` is only called in the module that defines the bitfield;
  elsewhere (tests, other modules) a bitfield is either `Type::default()` or comes from a
  constructor of its module. `ServiceContainer::new_request(code)` / `new_response(code)` wrap the
  builder for the service byte of a request or a response.
* **Service codes** — `ServiceCode` lists the CIP common services and the Connection Manager
  services: `ForwardClose` (0x4E), `UnconnectedSend` (0x52), `ForwardOpen` (0x54),
  `GetConnectionData` (0x56), `SearchConnectionData` (0x57), `GetConnectionOwner` (0x5A),
  `LargeForwardOpen` (0x5B), with `Unknown(u7)` for any other value.
* **General status** — `ResponseStatusCode` lists every general status code (0x00–0x2B) and keeps
  any other code as `Unknown(u8)`; its derived `Debug` names the status and it has no `Display`.
  `ResponseData` reads the Additional Status words (`additional_status: Vec<u16>`, one per
  `additional_status_size`) that error replies carry, such as the extended status of a failed
  `Forward_Open`, and gives the reply data what is left after them.
* **Rejection** — `Rejection { service, general_status, additional_status }`
  (`src/cip/message/response.rs`): a refused request as plain data (`Debug`, `PartialEq`,
  `Clone`; no `Display`, no `std::error::Error`), the service the adapter answered, its general
  status and the Additional Status words, whose meaning depends on the object that refused it.
  `Rejection::from_response(&MessageRouterResponse)` returns it, or `None` when the general status
  is success.
* **Typed data** — `CipData` (`src/cip/message/data.rs`) requires `Send + Sync`, so packets
  carrying typed data can be held across `.await` points. Its blanket impl only requires
  `BinWrite` with empty arguments, since `write_to` only writes, so a type that reads with
  arguments can be typed data as well as one that reads without. `adapter` (below) is the crate's
  only feature.
* **EPATH** — `src/cip/path.rs`:
  * `SegmentType` names every segment type (`PortSegment`, `LogicalSegment`, `NetworkSegment`,
    `SymbolicSegment`, `DataSegment`) and `LogicalSegmentType` every logical segment type
    (`ClassId`, `InstanceId`, `MemberId`, `ConnectionPoint`, `AttributeId`, `Special`,
    `ServiceId`, `Reserved`);
  * `LogicalPathSegment` reads only logical segments: a segment of any other type fails the read.
    `new_u8` and `new_u16` build an 8-bit or a 16-bit segment;
  * `CipPath` is a list of logical segments (`segments`, any length, any mix of widths). It reads
    with the path size in 16-bit words and fails when a segment overruns that size. Its
    constructors are `from_segments`, `new` (class and instance as 16-bit segments), `new_u8`
    (class and instance as 8-bit segments; both widths are valid for a value that fits, but the
    Teknic IO-HUB refuses a 16-bit request path with a path segment error), `new_full` (class,
    instance and attribute as 8-bit segments) and `new_assembly_connection` (the usual
    `config instance / O->T connection point / T->O connection point` path to the Assembly
    object); `word_len()` gives its size in words. Only logical segments are modelled;
    application-defined content (such as configuration data) is not part of the path type,
    following the same rule as assemblies: the caller declares it and passes it in;
  * `write_path_with_word_size`, a `write_with` function that prefixes a path with its size in
    16-bit words, writes the Request Path Size and Request Path of `RequestData`.
* **Common Packet Format** — `src/eip/description.rs`, `src/eip/command.rs`, `src/eip/packet.rs`,
  `src/object_assembly.rs`. Every address and data item of a packet is modelled the same way, as an
  item of one list:
  * `CommonPacketItemId` names the item Type IDs and keeps any other ID as `Unknown(u16)`;
  * `CommonPacketItem`: one enum for every item — Null Address Item, Unconnected Data Item
    (carrying a `CipMessage`), Socket Address Info O->T and T->O, and an `Unknown { type_id, data }`
    fallback; variant and Type ID names follow Wireshark. The Type ID and Length are derived from
    the variant on write. An item of an unknown type, or whose data does not fit its variant
    (including an unparsable CIP message), is read as `Unknown`, skipped by its Length and
    re-serialized unchanged;
  * `CipMessage` (`src/cip/message.rs`): `Request(MessageRouterRequest)` or
    `Response(MessageRouterResponse)`, chosen on read by the Request/Response bit of the service
    code byte, so no type is generic over the message;
  * `src/eip/sockaddr.rs`, used when opening an I/O connection: `SockaddrInfo` (family, port and
    address in big endian; zero padding) with conversions from and to `SocketAddrV4`, and
    `CommonPacketItem::sockaddr_info()`;
  * `RRPacketData` holds the interface handle, the timeout and `items`; the item count is read
    from the wire and written from `items.len()`. `RRPacketData::new_unconnected` builds the Null
    Address Item followed by the Unconnected Data Item;
  * `EnIpPacket` is the whole packet: `EncapsulationHeader` plus `CommandSpecificData`. The
    header's `length` is written from the size of the command specific data when it is `None`.
    `src/eip/packet.rs` starts with a map from the Wireshark tree to the Rust fields. Constructors:
    `new_registration`, `new_unregistration`, `new_send_rr_data`. `read_request` / `read_response`
    fail when a SendRRData packet does not carry a Message Router request / response, while plain
    `read` accepts either and keeps a message it cannot parse as an `Unknown` item. `read_request`
    is only compiled with the `adapter` feature (adapter-side helpers). `cip_message()`,
    `response()` and `sockaddr_info_items()` give the carried message, the Message Router
    response and the Sockaddr Info items;
  * `RequestObjectAssembly` / `ResponseObjectAssembly` are both aliases of `EnIpPacket` that only
    document the direction; `RequestObjectAssembly::new_identity` and `new_service_request` build
    a request to an object.
* **README** — what the library covers, and the "Related projects" section.

## Tests

* The packet suites (`test_encapsulated_packet`, `test_identity_object`, `test_message_router`,
  `test_object_assembly`, `test_request_object`, `test_response_object`, `test_session_object`)
  compare Register Session, Unregister Session, Identity and assembly packets, and the Message
  Router messages inside them, byte for byte; a packet is compared whole: the encapsulation header,
  the command specific data and its items, CIP message included. Their expected values build the
  item list with `RRPacketData::new_unconnected` or `RequestObjectAssembly::new_send_rr_data`.
* `tests/test_common_packet.rs` — Sockaddr Info byte order, a T->O Sockaddr Info item, and
  `read_response` rejecting a request that plain `read` keeps. Replies carrying Sockaddr Info items
  are tested in phase 2 with Forward_Open reply bodies.
* `tests/test_cip_path.rs` — the assembly connection path written and read, and the rejection of a
  non-logical segment and of a segment overrunning the declared length.
  `tests/test_path_segment.rs` covers 16-bit segments, class/instance and class/instance/attribute
  paths, and the rejection of a data segment; `src/cip/path.rs` unit-tests the `PathData`
  conversions and the path sizes.
* `tests/common.rs` — session handles, assembly instances and connection points shared by the
  captures.
* `tests/test_general_status.rs` — a reply with Additional Status words and data, an unknown
  general status read and written back unchanged, and the Connection Manager service codes.

## Verification

```
cargo fmt --check && cargo clippy --all-targets && cargo test --all && cargo test --all --features adapter && cargo test --examples
```

`cargo clippy --all-targets` reports style warnings (`Into` impls, an elidable lifetime, a `-1 *`
multiplication, useless conversions in tests); they are outside the scope of this phase.
