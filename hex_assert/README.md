# hex_assert

Hex assertions for the byte-exact packet tests, imported with `use hex_assert::prelude::*;`.

## assert_eq_hex!

`assert_eq_hex!(left, right)` compares two byte buffers and, when they differ, prints both as hex
dumps (`pretty_hex`), 30 bytes per line with ASCII.

## assert_eq_hex! with a config

`assert_eq_hex!(left, right, config)` prints the dumps with the given `HexConfig` instead.
