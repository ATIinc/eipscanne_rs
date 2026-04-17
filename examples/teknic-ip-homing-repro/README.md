# teknic-ip-homing-repro

Reproduces a bug in ClearPath-IP servo motors where sending repeated homing commands before the previous homing has completed leaves the motor in an unexpected state.

## The Bug

A single homing command (`HomingMove`) should:
1. Drive the motor to its home sensor
2. Set `homing = 1` while in progress
3. Set `has_homed = 1` when done

When a second (or third) homing command arrives **while homing is still in progress**, the motor may cancel the active homing move, fail to assert `has_homed`, or enter a fault state — depending on firmware version. This repro makes that easy to observe.

## Hardware Required

- Teknic **IO-HUB-4-E** (EtherNet/IP, 4-motor variant)
- ClearPath-IP motor connected to **port M0**
- A home sensor configured on the motor
- Network access to the hub

## Configuration

All tuneable knobs are at the top of [`main.rs`](main.rs):

| Constant | Default | Description |
|---|---|---|
| `DEVICE_IP` | `172.31.19.18` | IP address of the IO-HUB-4-E |
| `HOMING_COUNT` | `3` | Number of homing commands to send (set > 1 to trigger the bug) |
| `DELAY_MS` | `100` | Milliseconds between successive homing commands |
| `POST_HOMING_OBSERVE_SECS` | `10` | Timeout waiting for homing to complete after all commands are sent |

## How to Run

```sh
cargo run --bin teknic-ip-homing-repro
```

## What to Observe

The program prints motor status every `OBSERVE_POLL_MS` (100 ms) during the wait phase. Watch the key bits:

| Field | Meaning |
|---|---|
| `homing` | `1` while homing is in progress |
| `has_homed` | `1` once homing completes successfully |
| `move_cancelled` | `1` if the motor rejected or cancelled the move |
| `shutdown` | `1` if the motor faulted |

**Normal (single command):** `homing` goes high, then `has_homed` goes high. Done.

**Bug (repeated commands):** One or more commands may be rejected (`move_cancelled` high, `move_type_ack` returns an AOI error code), or homing restarts and takes longer than expected, or the motor faults.

If homing does not complete within `POST_HOMING_OBSERVE_SECS`, the program prints a timeout message and exits cleanly (session is unregistered).
