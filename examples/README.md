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

## Teknic-SD-Homing-Diag

Runs a full sensor-based homing sequence on a single Teknic ClearLink motor connector and is designed to help diagnose homing-related bugs (e.g. unexpected behaviour with specific deceleration values).

Before running, edit the constants at the top of `examples/teknic-sd-homing-diag/main.rs` to match your hardware:

| Constant | Description |
|---|---|
| `CLEARLINK_IP` | IPv4 address of the ClearLink device |
| `MOTOR_INDEX` | Motor connector to home (0–3) |
| `HOME_SENSOR_CONNECTOR` | I/O pin for the home sensor, or `-1` for hard-stop homing |
| `HOMING_VELOCITY_STEPS` | Jog velocity during the homing move (steps/s) |
| `HOMING_ACCELERATION_STEPS` | Acceleration limit (steps/s²) |
| `HOMING_DECELERATION_STEPS` | Deceleration limit (steps/s²) — set this to a negative value to reproduce the bug |

Run with:

```
cargo run --example teknic-sd-homing-diag
```

Press **Ctrl+C** at any time to abort the homing sequence. The example will always attempt to disable the motor and unregister the EtherNet/IP session before exiting.

Sequence of operations:
1. Connects to the ClearLink device over TCP
1. Registers an EtherNet/IP session
1. Clears any active shutdowns or motor faults
1. Writes the homing configuration (enables homing, sets the home sensor connector and deceleration limit)
1. Enables the motor and waits for it to report ready
1. Issues a velocity homing move and waits for the device to acknowledge it
1. Clears the move flags and waits for the `has_homed` status bit to be set
1. Disables the motor
1. Unregisters the EtherNet/IP session
