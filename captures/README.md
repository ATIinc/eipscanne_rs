# Captures

## Viewing Wireshark Files
1. Install wireshark: https://www.wireshark.org/


## Capturing on macOS

1. Install `tshark`: `brew install wireshark`
1. Run it: `sudo tshark -i en0 -f "port 44818 or port 2222" -w ./captures/test.pcap`

## Capturing in the devcontainer

1. Install `tshark`: already installed by the devcontainer
1. Run it: `sudo tshark -i any -f "port 44818 or port 2222" -w ./captures/test.pcap`
