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

### 1. Typed implicit I/O (delivered in phase 3)

The implicit side takes the same structs through phase 3's `Input::decode::<T>()` (the counterpart
of `decode_reply`; every byte must belong to `T`) and `Producer::next_packet_from(&outputs, run)`.
They live in the `scanner` crate, so they were delivered with it.

### 2. `eds_parser`: assembly members (step 3b grows)

* `Param` gains `data_type` (the CIP type code), `help` and its `EnumN` names (bit or value names).
* `Assembly` gains `members: Vec<Member>`, with `Member { offset_bits, size_bits, param:
  Option<Param> }`, read from the `bits, ParamN` pairs after the descriptor field:
  * `bits,` with no param: an opaque member (the fixture's `256,;`);
  * `bits, AssemN` (nested assembly): refused with a `BadField` error;
  * members that are not byte-aligned are allowed (bit offsets); the checks work on the bytes they
    cover.
* The member bits must add up to `size * 8` when a size is given. An empty size is the sum of the
  members, which the connection bridge then also accepts for an `AssemN` without a size.
* `Assembly::instance()`: the instance from the assembly's path (`20 04 24 64 30 03` -> 100).

### 3. `eds_parser`: layout printout — the input for writing an assembly

`impl Display for Assembly`: one line per member with byte offset, bit size, CIP type name, param
name, units and help, plus the enum names indented under their member. Example
`eds-assemblies --eds <file> [--assembly Assem100]` prints it for every assembly, or for one. Paste
this output into the prompt when asking Claude to write `scanner/assemblies/<device>/`.

### 4. `eds_parser`: `check_assembly::<T>(&Assembly) -> Result<Vec<Finding>, AssemblyMismatch>`

`T: BinRead + BinWrite + Debug`; the checks run in this order:

1. **Size.** Decoding `size` zero bytes as `T` consumes exactly `size` bytes, and re-encoding gives
   `size` bytes back. This catches a missing, extra or wrong-width field.
2. **Coverage.** For each member, a buffer that is zero except for that member's bytes (non-zero
   pattern) decodes and re-encodes unchanged. This catches a struct that pads over bytes the EDS
   calls data. Members whose param name starts with "Reserved" may be either a field or padding
   (the IO-HUB output assembly pads them with `pad_after`). New assemblies prefer a named
   `_reserved` field, so every byte of the EDS layout shows up in the struct.
3. **Values.** For each integer or real member (not BYTE/WORD/DWORD bit strings, which are usually
   bilge bitfields), the member alone is set to a distinctive value of its type, and the `Debug`
   output of the decoded `T` must contain that value. This proves that one field covers exactly
   those bytes with that width and signedness, which catches reordered fields and a `u16` + `u16`
   written where a `u32` belongs.

A member whose probe the struct rejects (an enum with `TryFromBits`) is returned as
`Finding::NotChecked` on success rather than failing. `AssemblyMismatch` lists every finding with
the member's offset and param name.

Limits: two adjacent fields of the same type that are swapped pass every check. They are caught only
by a byte-exact test against captured data.

### 5. Example — `eds_parser/examples/io-hub-implicit.rs`

The implicit counterpart of `io-hub-homing`, one linear file:

1. Parse `--eds` (the local IO-HUB file) and pick the connection.
2. `check_assembly::<InputAssemblyHub4E>` and `::<OutputAssemblyHub4E>` against the connection's
   T->O and O->T assemblies, and check that the instance constants equal the connection points;
   refuse to open the connection on a mismatch.
3. Open; each cycle send `OutputAssemblyHub4E::default()` (idle unless `--run`; nothing is enabled
   either way), decode the inputs with `input.decode::<InputAssemblyHub4E>()` and print a status
   line for the chosen motor when the printed values change (measured values such as torque move
   all the time). Ctrl+C, close and unregister, as in the other examples.

It includes the assemblies with `#[path = "../../scanner/assemblies"]` and reports errors through
`anyhow` (a dev-dependency), so they read as messages with their causes.

### 6. IO-HUB assemblies follow the EDS

The check found the hand-written IO-HUB structs off in signedness only. The ClearPath-IP Software
Reference linked at the top of each file is their source, but where it and the EDS disagree the
EDS is followed and the doc comment says the documentation may be wrong:

| Field | EDS | Software Reference | Now |
|---|---|---|---|
| Analog inputs I/O-0…12, analog output I/O-12 | UINT | INT (tags) | `CipUint`, with the callout |
| Read/Write Parameter ID | INT | INT | `CipInt` |
| Read Parameter ID Echo, Move Number, Move Number Ack | INT | no type | `CipInt` |

### Documentation

* `eds_parser/README.md`: "Writing a device's assemblies": print the layout, have the structs
  written (reserved bytes as named `_reserved` fields), add the check test, then a capture test.
* `phases/README.md`: phase 5's row points here. The Safety System validation phase is removed
  (`05-safety-system-validation.md` deleted): testing against the device is done by hand, outside
  the plan.

## Tests

* Members: inline snippets for `bits,ParamN`, opaque, nested (refused), non-byte-aligned and
  a sum that does not match the size; `instance()`.
* Fixture: its two opaque 32-byte assemblies pass `check_assembly::<[u8; 32]>` and fail with
  `[u8; 31]`; the fixture itself is unchanged (typed members are covered by inline snippets).
* `check_assembly`: a correct struct passes; a missing field, a padded data member, a `u16` in place
  of a `u32` and two reordered fields of different types each fail with the right finding; a
  `TryFromBits` enum is reported as not checked.
* Ignored, with the local EDS: `scanner/assemblies/io_hub` input (100) and output (101) pass
  `check_assembly` against `docs/IO-HUB-4-E_EDS_File.eds`.

## Verification

```
cargo fmt --all --check && cargo clippy --all-targets && cargo test
EDS_FILE=docs/IO-HUB-4-E_EDS_File.eds cargo test -- --ignored
cargo run --example eds-assemblies -- --eds docs/IO-HUB-4-E_EDS_File.eds --assembly Assem100
```

Against the hub: `cargo run --example io-hub-implicit -- --eds docs/IO-HUB-4-E_EDS_File.eds --host <ip>`
opens the connection, exchanges I/O every 10 ms and closes cleanly (done against an IO-HUB-4-E).
Getting there needed library and scanner fixes, which went into the phases that own those files:
the 8-bit Connection Manager path (phase 2) and readable rejections (phases 2 and 3).
