# Phase 4 — I/O connection state

**Status:** Not started

## Goal

A socket-free `IoConnection` (in `src/io_connection.rs`) that owns everything a scanner must remember
about one open connection, mirroring EIPScanner's `IOConnection` and the receive path of its
`ConnectionManager`.

## Scope

* Built from the `ForwardOpenRequest`, the `ForwardOpenResponse` (from
  `ConnectionManagerResponse::from_message_router_response`) and any Sockaddr Info items of the reply
  (`EnIpPacket::sockaddr_info_items()`); resolves the target UDP endpoint (the O->T
  `SockaddrInfo::socket_address()` if present, `0.0.0.0` meaning "the session's IP", else the
  session IP on `ETHERNET_IP_IO_UDP_PORT`).
* Timing: O->T period from the actual packet interval (`o2t_api`), receive timeout =
  `ConnectionTimeoutMultiplier::multiplier()` × `t2o_api` with a 10 s grace before the first
  packet; the first packet is due immediately after the connection is established.
* `next_output_packet`: size check, encapsulation sequence number (random start) and CIP sequence
  count increments, run/idle header, encoding into an `IoPacket`.
* `accept_input_packet`: screening by connection ID and sender IP, discarding old/duplicate or
  too-far-ahead encapsulation sequence numbers (modular arithmetic), decoding `IoData`, flagging
  duplicate CIP sequence counts as "no new data", refreshing the timeout only for accepted packets.
* `forward_close_request()` builds the matching `ForwardCloseRequest` (same connection serial
  number, originator vendor ID and serial number, and connection path).

## Tests

Unit tests for each screening rule and the timeout arithmetic.
