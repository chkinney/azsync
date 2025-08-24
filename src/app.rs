use std::io::stderr;

use clap::Parser;
use tracing::level_filters::LevelFilter;

use crate::{
    cli::{Cli, CliCommand},
    commands::Command,
};

pub async fn run() -> anyhow::Result<()> {
    // Parse CLI options
    let options = Cli::parse();
    init_tracing(&options);

    // Run command
    match options.subcommand {
        CliCommand::Completions(command) => command.execute(&options.global).await?,
        CliCommand::Dotenv(command) => command.execute(&options.global).await?,
    }

    Ok(())
}

/// Setup the tracing subscriber based on the provided CLI options.
fn init_tracing(options: &Cli) {
    let filter = match options.global.verbose {
        0 => LevelFilter::OFF,
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        3.. => LevelFilter::TRACE,
    };
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(filter)
        .with_writer(stderr)
        .init();
}
