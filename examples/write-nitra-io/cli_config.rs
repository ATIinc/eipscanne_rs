use clap::Parser;

#[derive(Parser)]
pub struct OutputValue {
    /// Turns the output on
    #[arg(long, required = true, conflicts_with = "off")]
    pub on: bool,

    /// Turns the output off
    #[arg(long, required = true, conflicts_with = "on")]
    pub off: bool,
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
