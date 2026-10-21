# Captures

## Viewing Wireshark Files
1. Install wireshark: https://www.wireshark.org/


## Capturing Network Traffic (on macOS in adevcontainer)

1. Install tcpdump: `sudo apt-get update && apt-get install -y tcpdump`
1. Take a recording on the eth0 interface: `sudo tcpdump -i lo port 44818 -w ./captures/test.pcap`