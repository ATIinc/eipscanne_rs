#!/usr/bin/env bash
# Dissects a single EtherNet/IP packet with Wireshark's command-line tools and prints the
# dissection tree, the same text the tests carry in their comments.
#
# The packet bytes are read as hex from a file or from stdin. Any of these layouts work:
#   Wireshark "Hex Dump" lines:  0000   6f 00 1a 00 06 00 ...
#   Rust byte vectors:           0x6f, 0x00, 0x1a, 0x00, ...
#   plain hex:                   6f001a0006...
#
# Requires tshark and text2pcap (the `tshark` package), installed by the devcontainer.
#
# Usage: scripts/dissect.sh [--udp] [--frame] [--request hexfile] [hexfile]
#   --udp      the bytes are a class 0/1 I/O packet (UDP port 2222) instead of an
#              encapsulation packet (TCP port 44818)
#   --frame    also print the Frame / Ethernet / IP / TCP (UDP) layers
#   --request  the packet is the reply to the request in this file. Wireshark only knows
#              which service a reply answers when the request precedes it in the capture,
#              so the request is placed in front of the reply, whose dissection is printed
set -euo pipefail

udp=0
keep_frame=0
request=""
input=/dev/stdin
while [ $# -gt 0 ]; do
    case "$1" in
        --udp) udp=1 ;;
        --frame) keep_frame=1 ;;
        --request) shift; request="${1:?--request needs a hex file}" ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        *) input="$1" ;;
    esac
    shift
done

# Normalises an input into plain hex
hex_of() {
    local hex
    hex=$(sed -E 's/^[[:space:]]*[0-9a-fA-F]{4}[[:space:]]{2,}//; s/0x//g; s/[,;]/ /g' "$1" \
        | tr -d ' \n\r\t' | tr 'A-F' 'a-f')
    if [ -z "$hex" ] || [ $(( ${#hex} % 2 )) -ne 0 ]; then
        echo "no (or an odd number of) hex digits found in $1" >&2
        exit 1
    fi
    echo "$hex"
}

# Prints plain hex as the "offset bytes" lines text2pcap expects, the first one behind the
# direction indicator (I or O) when one is given
lines_of() {
    echo "$1" | fold -w 32 | awk -v direction="${2:-}" \
        '{ printf "%s%06x", (NR == 1 ? direction : ""), (NR-1)*16; for (i = 1; i <= length($0); i += 2) printf " %s", substr($0, i, 2); print "" }'
}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

packet_hex=$(hex_of "$input")
if [ -z "$request" ]; then
    # A packet on its own: both ports are the EtherNet/IP one, so either direction fits
    if [ "$udp" -eq 1 ]; then transport=(-u 2222,2222); else transport=(-T 44818,44818); fi
    lines_of "$packet_hex" > "$workdir/packets.hex"
    frame=1
else
    # The request goes out to the EtherNet/IP port (O), the reply comes back from it (I)
    request_hex=$(hex_of "$request")
    transport=(-D -T 44818,50000)
    { lines_of "$request_hex" "O "; lines_of "$packet_hex" "I "; } > "$workdir/packets.hex"
    frame=2
fi
text2pcap -q "${transport[@]}" "$workdir/packets.hex" "$workdir/packets.pcap"

dissection=$(tshark -r "$workdir/packets.pcap" -V -Y "frame.number == $frame")
if [ "$keep_frame" -eq 1 ]; then
    echo "$dissection"
else
    # Drop the generated lower layers; the packet starts at the EtherNet/IP layer
    echo "$dissection" | sed -n '/^EtherNet\/IP/,$p'
fi
