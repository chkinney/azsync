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
    let result = match options.subcommand {
        CliCommand::Completions(command) => command.execute(&options.global).await,
        CliCommand::Dotenv(command) => command.execute(&options.global).await,
        CliCommand::File(command) => command.execute(&options.global).await,
    };

    // Report errors
    if let Err(error) = result {
        for cause in error.chain() {
            tracing::error!("{cause}");
        }
    }

    Ok(())
}

/// Setup the tracing subscriber based on the provided CLI options.
fn init_tracing(options: &Cli) {
    // Set level filter based on verbosity
    let filter = match options.global.verbose {
        0 | 1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        3.. => LevelFilter::TRACE,
    };

    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_max_level(filter)
        .with_writer(stderr)
        .with_target(options.global.verbose > 1);

    if options.global.verbose == 0 {
        // Exclude timestamps for non-verbose output
        subscriber.without_time().init();
    } else {
        subscriber.init();
    }
}
