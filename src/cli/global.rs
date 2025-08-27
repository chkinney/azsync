use std::path::PathBuf;

use clap::{ArgAction, Args};

/// Global options that are always relevant.
#[derive(Clone, Debug, Args)]
#[command(version, next_help_heading = "Global")]
pub struct GlobalOptions {
    /// The dotenv file to load (if present).
    ///
    /// Some options can load values from your environment. If this dotenv file
    /// exists, then it will be loaded and used in addition to this program's
    /// environment variables.
    #[arg(global = true, long, short = 'e', default_value = ".env")]
    pub env_file: PathBuf,

    /// Disables loading options from dotenv files (with --env-file).
    ///
    /// If a dotenv file specified by --env-file exists, it will be ignored.
    /// This flag takes precedence over --env-file.
    ///
    /// Note that this only applies to loading options from your dotenv file.
    /// azsync dotenv still uses the value of --env-file to find which dotenv
    /// file to sync, and still loads that file when synchronizing.
    #[arg(global = true, long)]
    pub no_env_file: bool,

    /// Enable more verbose output (repeatable up to 3 times).
    ///
    /// Output is emitted via stderr.
    #[arg(global = true, long, short = 'v', action = ArgAction::Count)]
    pub verbose: u8,
}
