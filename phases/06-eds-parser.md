# Phase 6 — EDS parser crate

**Status:** Not started

## Goal

Derive `Forward_Open` parameters from a device's EDS file instead of typing them by hand.

## Scope

New workspace crate `eds_parser/` (deps `pest`, `pest_derive`):

* `src/eds.pest` — the EDS syntax as a PEG grammar: `[Section]` headers (case-insensitive, single
  internal spaces allowed), `Keyword = field, field, ...;` entries spanning lines, empty and omitted
  trailing fields, `{ ... }` nested fields, quoted strings with escapes (and the `L"..."` prefix),
  decimal / `0x` hex / `0b` binary numbers, dates, times, identifiers (`Param1`, `Assem150`,
  `SYMBOL_ANSI`), bracketed path references (`[Param1]`); `$` comments and whitespace are silent
  rules.
* `parser.rs` — walks the parse tree into `EdsFile { sections → entries → fields }` with line/column
  errors from pest.
* Typed views: `[File]`, `[Device]`, `[Params]` (enough to resolve `ParamN` defaults, e.g. an RPI),
  `[Assembly]` (`AssemN`: name, path, size, descriptor), `[Connection Manager]` (`ConnectionN`:
  trigger/transport mask, connection parameter mask, O->T / T->O RPI, size and format, config
  entries, name, help, path). Empty sizes/RPIs resolve through the referenced `AssemN` / `ParamN`.
* `bridge.rs` (depends on `eipscanne_rs`): `ConnectionN` → `ConnectionParameters` (class, trigger,
  connection types, priorities, fixed/variable, real-time formats, sizes, RPIs, `CipPath` from the path
  string with `[ParamN]` substitution).

## Tests

* Committed fixture: OpENer's BSD-licensed `opener_sample_app.eds` (`eds_parser/tests/fixtures/`).
* Grammar edge cases as inline snippets; the bridge must produce the EIPScanner example parameters
  for OpENer's `Connection1`.
* An ignored test reads `EDS_FILE` so the Safety System's EDS (kept locally, not committed) can be
  exercised: `EDS_FILE=docs/<device>.eds cargo test -p eds-parser -- --ignored`.
