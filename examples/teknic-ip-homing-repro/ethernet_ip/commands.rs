use super::io_hub_output::{MotorOutputData, MoveType};

/// High-level motor commands mapped to CIP output fields.
pub enum MotorCommand {
    Enable,
    Disable,
    ShutdownReset,
    ClearShutdownReset,
    HomingMove,
}

/// Apply a high-level motor command to the given per-motor output data.
pub fn apply_motor_command(command: MotorCommand, data: &mut MotorOutputData) {
    match command {
        MotorCommand::Enable => {
            data.controlword.set_enable(true);
        }
        MotorCommand::Disable => {
            data.controlword.set_enable(false);
        }
        MotorCommand::ShutdownReset => {
            data.controlword.set_shutdown_reset(true);
        }
        MotorCommand::ClearShutdownReset => {
            data.controlword.set_shutdown_reset(false);
        }
        MotorCommand::HomingMove => {
            data.move_type = MoveType::HomingMove.to_u8();
        }
    }
}
