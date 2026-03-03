# teknic_homing_diag_rs

Standalone diagnostic tool for reproducing and investigating the ClearLink homing bug.

## What it does

Connects directly to a ClearLink over EtherNet/IP and executes a single homing move,
logging every `MotorStatus` poll response to stdout so the full state transition sequence
is visible. The homing move parameters (velocity, acceleration, sensor connector, etc.)
are constants defined at the top of `src/main.rs`.

## Prerequisites

- Rust toolchain (stable, 1.80+): <https://rustup.rs>
- Network access to the ClearLink device

## Build

```bash
cargo build
```

## Run

```bash
cargo run -- --ip <CLEARLINK_IP>
# Example:
cargo run -- --ip 172.31.19.21
```

## Adjusting parameters

All homing parameters are constants at the top of `src/main.rs`:

| Constant | Description |
| --- | --- |
| `MOTOR_INDEX` | Which motor connector to home (0–3) |
| `HOME_SENSOR_CONNECTOR` | I/O pin index for the home sensor (-1 = hard-stop homing) |
| `HOMING_VELOCITY_STEPS` | Jog velocity for the homing move (steps/s, must be negative) |
| `HOMING_ACCELERATION_STEPS` | Acceleration limit (steps/s²) |
| `HOMING_DECELERATION_STEPS` | Deceleration limit (steps/s²) |
| `POLL_INTERVAL_MS` | How often to poll the input assembly (milliseconds) |
| `HOMING_TIMEOUT_SECS` | Maximum time to wait for `has_homed` before aborting |
