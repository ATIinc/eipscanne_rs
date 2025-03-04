# Examples

## Run Exampeles

Can run the examples with `cargo run --example <file_name>`

## Test Examples

Can test the examples with `cargo test --examples`

Also possible to declare the exact test using the using cargo test flags


## Examples Explained

### Read-Identity

Requests the identity CIP object from the connected device

i.e. `cargo run --example read-identity`


1. Requests a session registration
1. Reads the session registration and extracts the session_id
1. Sends a request for the the Identity object
1. Reads the Identity Object response
1. Requests an unregistration for the session_id 

## Write-Teknic-IO

Reads from and Writes to a Teknic ClearLink motor controller board using the assembly objects defined in Teknic's Ethernet/IP Object Reference: https://www.teknic.com/files/downloads/clearlink_ethernet-ip_object_reference.pdf#page=18

i.e. `cargo run --example write-teknic-io -- --help`
* `cargo run --example write-teknic-io -- --index 4 --on`
* `cargo run --example write-teknic-io -- --index 4 --off`
* `cargo run --example write-teknic-io -- --index 4 --pwm 100`

1. Parses the desired digital output to be modified from the commandline
1. Requests a session registration
1. Reads the session registration and extracts the session_id
1. Writes the ConfigAssembly object
1. Reads the ConfigAssembly object success response
1. Requests the OutputAssembly object
1. Reads the OutputAssembly object data
1. Modifies the value of the appropriate digital output (from commandline)
1. Writes the modified OutputAssembly object
1. Reads the modified OutputAssembly object success response
1. Requests an unregistration for the session_id 

## Write-Nitro-IO

Sets valves on the Nitra Ethernet/IP Pneumatic Regulator

i.e. `cargo run --example write-nitra-io -- --help`
* `cargo run --example write-nitra-io -- --address 172.31.19.60 custom`
* `cargo run --example write-nitra-io -- --address 172.31.19.60 selection --valves 0 2 --on`