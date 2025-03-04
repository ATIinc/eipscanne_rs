use clap::Parser;

#[derive(Parser, Debug)]
pub struct OutputValue {
    /// Turns the output on
    #[arg(long, required = false, conflicts_with = "off")]
    pub on: bool,

    /// Turns the output off
    #[arg(long, required = false, conflicts_with = "on")]
    pub off: bool,
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = "Used to set the value of a digital output on a Teknic ClearLink controller"
)]
pub struct CliArgs {
    #[arg(short, long, default_value = "172.31.19.10")]
    pub address: String,

    #[arg(short, long, default_value_t = false)]
    pub custom: bool,

    #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..15), num_args = 1.., value_delimiter = ' ')]
    pub select: Vec<u8>,

    #[command(flatten)]
    pub output_value: OutputValue,
}
