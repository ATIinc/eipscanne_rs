# Tests

`cargo test` from the repository root runs the byte-exact packet tests here, one file per packet
type. [integration](./integration/) runs the examples against an OpENer adapter.

## Packet tests

Each test holds one packet four ways: its Wireshark dissection and hex dump (a comment), its
bytes, and the struct they decode to. It serializes the struct and compares the bytes with
`assert_eq_hex!`, then deserializes the bytes and compares the struct. Values shared between
files, such as session handles and connection IDs, live in `common.rs`.

The bytes are known packet bytes, from one of:

* sniffed with Wireshark (the Identity packets come from EIPScanner's Identity example): copy the
  packet as a hex dump. In the devcontainer: `sudo tshark -i any -f "port 44818 or port 2222"`
* `EIP_DUMP=1 cargo run --example <name>`: every encapsulation packet the scanner sends or
  receives is printed to stderr as hex
* written by hand

`scripts/dissect.sh` prints the dissection with `tshark`, which the devcontainer installs:

```
scripts/dissect.sh packet.hex                        # TCP 44818 encapsulation packet
scripts/dissect.sh --udp packet.hex                  # UDP 2222 I/O packet
scripts/dissect.sh --request request.hex reply.hex   # a reply, decoded by its request's service
```

It reads Wireshark hex dump lines, Rust byte vectors or plain hex. A `Malformed` dissection means
the bytes are wrong.
