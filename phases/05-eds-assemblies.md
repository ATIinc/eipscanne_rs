# Phase 5 — Device assemblies checked against the EDS

## Goal

Exchange class 1 I/O with the same hand-written `binrw` assemblies the explicit examples use
(`scanner/assemblies/`), and prove that an assembly matches the device's EDS before the connection
is opened. The assemblies are written by a person or by Claude from a layout printout; the EDS
parser does not generate code.

Why not generate code: a generator can only emit a flat struct (`motor0_statusword`,
`motor1_statusword`, ...). The hand-written assemblies have per-motor groups, named bits, enums
such as `MoveType`, and helpers such as `motor_input(n)` and `command_move`. Checking is cheaper
and keeps the ergonomic types.

## Scope

### 1. Typed implicit I/O (phase 3's `scanner` crate)

The implicit side takes the same structs through phase 3's stage 3 modules, which keep no state.
`scanner::implicit::o2t::build_o2t_packet` takes the outputs as
`CipDataOpt::Typed(Box::new(outputs))`, and `scanner::implicit::t2o::accept_t2o_packet` returns the
inputs as an `IoData` whose data is the bytes on the wire (`CipDataOpt::Raw`), which the struct
reads with `T::read_le` (the implicit counterpart of `scanner::explicit::decode_reply`). Both
belong to the `scanner` crate of phase 3; phase 5 has no code in `scanner/src/`.

### 2. `eds_parser`: assembly members (step 3b)

* `Param { keyword, name, data_type, data_size, units, help, min, max, default, enum_names }`:
  `data_type` is a `DataType` (`Bool`, `Sint` ... `Lreal`, the bit strings `Byte` ... `Lword`,
  and `Other(code)`) read from the CIP type code; `enum_names` are the `(value, name)` pairs of the
  `EnumN` with the param's number, bit numbers for a bit string and values otherwise.
* `Assembly { keyword, name, path, size, members }`: `path` is the bytes with `[ParamN]` resolved,
  and `members: Vec<Member>`, with `Member { offset_bits, size_bits, param: Option<Param> }`, are
  read from the `bits, ParamN` pairs after the descriptor field, each placed right after the
  previous one:
  * `bits,` with no param: an opaque member (the fixture's `256,;`);
  * `bits, AssemN` (nested assembly): refused with a `BadField` error;
  * members that are not byte-aligned are allowed (bit offsets); the checks work on the bytes they
    cover (`Member::byte_range`).
* The member bits must add up to `size * 8` when a size is given. An empty size is the sum of the
  members, which a connection with an empty size then also takes from that `AssemN`.
* `Assembly::instance()`: the instance from the assembly's path (`20 04 24 64 30 03` -> 100; 8-bit
  and 16-bit instance segments), or `None` without an instance segment.
* `Eds::assembly(keyword)`: the assembly a connection direction's `format` names, case ignored.

### 3. `eds_parser`: layout printout — the input for writing an assembly

`impl Display for Assembly`: a heading with keyword, name, instance and size, then one line per
member with byte offset (`byte.bit` when not byte-aligned), bit size, CIP type name, param keyword
and name, units and help, plus the enum names indented under their member. Example
`eds-assemblies --eds <file> [--assembly Assem100]` prints it for one assembly, or lists which
assemblies each connection carries and then prints every assembly. Paste this output into the
prompt when asking Claude to write `scanner/assemblies/<device>/`.

### 4. `eds_parser`: `check_assembly::<T>(&Assembly) -> Result<Vec<Finding>, AssemblyMismatch>`

In `src/check.rs`, with `binrw` a dependency of `eds_parser`. `T: BinRead + BinWrite + Debug`,
read and written without arguments; the checks run in this order:

1. **Size.** Decoding zero bytes as `T` consumes exactly `size` bytes, and re-encoding gives
   `size` bytes back. This catches a missing, extra or wrong-width field. A size mismatch stops
   the check there.
2. **Coverage.** For each member, a buffer that is zero except for that member's bits (all set)
   decodes and re-encodes unchanged. This catches a struct that pads over bytes the EDS
   calls data. Members whose param name starts with "Reserved" may be either a field or padding
   (the IO-HUB output assembly pads over its `Reserved Byte` members with `pad_after`). The
   convention for an assembly is a named `_reserved` field, so every byte of the EDS layout shows
   up in the struct.
3. **Values.** For each integer or real member (not BOOL, nor BYTE/WORD/DWORD/LWORD bit strings,
   which are usually bilge bitfields, nor a reserved member), the member alone is set to a
   distinctive value of its type, and the `Debug` output of the decoded `T` must contain that
   value. This proves that one field covers exactly those bytes with that width and signedness,
   which catches reordered fields and a `u16` + `u16` written where a `u32` belongs.

A member the check cannot probe is returned as `Finding::NotChecked` on success rather than
failing: the struct rejects the probe (an enum without that value), the member does not hold a
whole value of its type, or no field shows the value as a number (an enum or a bitfield). Every
other finding (`NoSize`, `ZerosRejected`, `ReadSize`, `WriteSize`, `Dropped`, `WrongValue`) is a
mismatch, and `AssemblyMismatch` lists every finding with the member's param, name and byte
offset.

Limits: two adjacent fields of the same type that are swapped pass every check. They are caught only
by a byte-exact test against captured data.

### 5. Example — `eds_parser/examples/io-hub-implicit.rs`

The implicit counterpart of `io-hub-homing`, one linear file:

1. Parse `--eds` (the local IO-HUB file), pick the first exclusive-owner connection and build its
   Forward_Open with `to_forward_open`; print the connection path as hex.
2. Check that the connection path ends with `2C 65 2C 64`, the connection points
   `OUTPUT_ASSEMBLY_INSTANCE` and `INPUT_ASSEMBLY_INSTANCE`, then
   `check_assembly::<InputAssemblyHub4E>` and `::<OutputAssemblyHub4E>` against the connection's
   T->O and O->T assemblies, printing the members not checked; refuse to open the connection on
   a mismatch.
3. Open with `scanner::implicit::connection::forward_open`; each cycle send
   `CipDataOpt::Typed(Box::new(OutputAssemblyHub4E::default()))` through `build_o2t_packet` and
   `send_io_packet` (idle unless `--run`; nothing is enabled either way; the outputs never change,
   so the CIP sequence count stays at 1). Each packet that `accept_t2o_packet` accepts is read with
   `InputAssemblyHub4E::read_le`, and a status line for the chosen motor (`--motor`) is printed
   when the printed values change (measured values such as torque move all the time); a packet it
   discards (`Error::UnexpectedPacket`) is printed as `DISCARDED`. After `--cycles` packets, on the
   input timeout or on Ctrl+C, close and unregister, as in the other examples.

It includes the assemblies with `#[path = "../../scanner/assemblies"]` and reports errors through
`anyhow` (a dev-dependency of `eds_parser`, for the examples), so they read as messages with their
causes. `--host` has no default.

### 6. IO-HUB assemblies follow the EDS

The hand-written IO-HUB structs use the EDS's integer types. The ClearPath-IP Software Reference
linked at the top of each file is their source, but where it and the EDS disagree the EDS is
followed and the doc comment says the documentation may be wrong:

| Field | EDS | Software Reference | Struct |
|---|---|---|---|
| Analog inputs I/O-0…12, analog output I/O-12 | UINT | INT (tags) | `CipUint`, with the callout |
| Read/Write Parameter ID | INT | INT | `CipInt` |
| Read Parameter ID Echo, Move Number, Move Number Ack | INT | no type | `CipInt` |

### Documentation

* `eds_parser/README.md`: "Writing a device's assemblies": print the layout, have the structs
  written (reserved bytes as named `_reserved` fields), add the check test, then a capture test.
* `phases/README.md` lists phase 5's pull request.

## Tests

* Params: an inline snippet with data types, units, help and `EnumN` names.
* Members: inline snippets for `bits,ParamN`, opaque, nested (refused), non-byte-aligned and
  a sum that does not match the size; a `[ParamN]` in an assembly path; `instance()` with 8-bit
  and 16-bit instance segments; the layout printout.
* Fixture: its two opaque 32-byte assemblies pass `check_assembly::<[u8; 32]>`, and the input
  assembly fails with `[u8; 31]`. The fixture has no typed members; those are covered by inline
  snippets.
* `check_assembly`: a correct struct passes, and so does one that pads over a reserved member; a
  missing field, a padded data member, two `u16` halves in place of a DINT, a signed field for an
  unsigned member and two reordered fields of different types each fail with the right finding; an
  enum that refuses the probe is reported as not checked; an assembly without a size cannot be
  checked; a mismatch lists every finding.
* Ignored, with the local EDS (`tests/io_hub_assemblies.rs`): `scanner/assemblies/io_hub` input
  (100) and output (101) have the instances of the connection's assemblies and pass
  `check_assembly` against `docs/IO-HUB-4-E_EDS_File.eds`.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test && cargo test --features adapter
EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored
cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100
```

Against the hub:

```
cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip>
```

opens the connection, exchanges I/O every 10 ms and closes cleanly; this is verified against an
IO-HUB-4-E.
