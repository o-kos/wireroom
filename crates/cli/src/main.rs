mod lookahead;
mod output;
mod pacing;
mod runtime;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about = "Inspect and process recorded signals")]
struct Cli {
    /// Module and recording search directories, separated by semicolons.
    #[arg(short = 's', long, global = true, value_delimiter = ';')]
    search_path: Vec<PathBuf>,
    /// Write events to this path (not implemented).
    #[arg(long, global = true)]
    events: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Debug, Default, ValueEnum)]
enum MessageFormat {
    #[default]
    Human,
    Json,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Replay a recorded run.
    Run {
        #[arg(long, value_enum, default_value = "human")]
        message_format: MessageFormat,
    },
    /// Check results.
    Check,
    /// Inspect module information.
    Info,
    /// List modules.
    List,
    /// Inspect module masks.
    Mask,
    /// Convert a signal.
    Convert,
    /// Record a run.
    Record,
}
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if !cli.search_path.is_empty() {
        anyhow::bail!("--search-path: not implemented");
    }
    if cli.events.is_some() {
        anyhow::bail!("--events: not implemented");
    }
    match cli.command {
        Command::Run {
            message_format: MessageFormat::Json,
        } => runtime::run(),
        Command::Run { .. } => anyhow::bail!("run: not implemented for human output"),
        command => anyhow::bail!(
            "{}: not implemented",
            match command {
                Command::Check => "check",
                Command::Info => "info",
                Command::List => "list",
                Command::Mask => "mask",
                Command::Convert => "convert",
                Command::Record => "record",
                Command::Run { .. } => "run",
            }
        ),
    }
}
