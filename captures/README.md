# Captures

## Viewing Wireshark Files
1. Install wireshark: https://www.wireshark.org/


## Capturing Network Traffic (in the devcontainer)

`tshark` is installed by the devcontainer:
`sudo tshark -i any -f "port 44818 or port 2222" -w ./captures/test.pcap`