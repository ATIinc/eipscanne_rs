use clap::Parser;

use crate::clearlink_output::IOOutputData;

#[derive(Parser)]
pub struct OutputValue {
    /// Turns the output on
    #[arg(
        long,
        required = true,
        conflicts_with = "off",
        conflicts_with = "pwm_value"
    )]
    on: bool,

    /// Turns the output off
    #[arg(
        long,
        required = true,
        conflicts_with = "on",
        conflicts_with = "pwm_value"
    )]
    off: bool,

    /// Sets the output value to the specified number between 0 and 255 (inclusive)
    #[arg(
        long = "pwm",
        required = true,
        conflicts_with = "on",
        conflicts_with = "off"
    )]
    pwm_value: Option<u8>,
}

/// Simple program to greet a person
#[derive(Parser)]
#[command(
    version,
    about,
    long_about = "Used to set the value of a digital output on a Teknic ClearLink controller"
)]
pub struct CliArgs {
    #[arg(short, long, default_value = "172.31.19.10")]
    pub address: String,

    #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..5))]
    pub index: u8,

    #[command(flatten)]
    pub output_value: OutputValue,
}

pub fn set_io_data(io_output_data: &mut IOOutputData, index: usize, output_value: OutputValue) {
    match output_value {
        // On and Off are mutually exclusive so only one needs to be checked
        OutputValue {
            on: is_on,
            off: _,
            pwm_value: None,
        } => {
            io_output_data.set_digital_output(index, is_on);
        }
        OutputValue {
            on: _,
            off: _,
            pwm_value: Some(pwm),
        } => {
            io_output_data.set_digital_pwm(index, pwm);
        }
    }
}
