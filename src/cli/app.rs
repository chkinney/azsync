use clap::{Parser, Subcommand};
use clap_cargo::style::CLAP_STYLING;

use crate::cli::{CompletionsOptions, GlobalOptions, SyncDotenvOptions, SyncFileOptions};

/// Quickly synchronize local files with Azure.
///
/// This requires you to be authenticated to Azure already as it uses the
/// default Azure credential for this environment. If needed, use the Azure CLI
/// to login and select the subscription you want to use.
#[derive(Clone, Debug, Parser)]
#[command(
    styles = CLAP_STYLING,
    disable_help_subcommand = true,
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// The subcommand to execute.
    #[command(subcommand)]
    pub subcommand: CliCommand,

    /// Global options relevant to all subcommands.
    #[command(flatten)]
    pub global: GlobalOptions,
}

/// A subcommand to execute.
#[derive(Clone, Debug, Subcommand)]
pub enum CliCommand {
    /// Generate shell completions.
    ///
    /// Completions are written to stdout. Save them to the appropriate place
    /// for your shell.
    Completions(CompletionsOptions),

    /// Synchronize variables defined in your local dotenv file with Azure.
    ///
    /// This only synchronizes variables defined in your dotenv file (or dotenv
    /// template, if available).
    ///
    /// When synchronizing with Key Vault, local variable names should use '_'
    /// (underscores) to represent '-' (hyphens) in secret names stored in Key
    /// Vault. The conversion between the two will be done automatically for
    /// you when either pushing or pulling variables.
    Dotenv(SyncDotenvOptions),

    /// Synchronize a file with Azure.
    File(SyncFileOptions),
}

const AFTER_HELP: &str = concat!(
    "Please submit all issues and feature requests on GitHub.\n",
    "\n",
    env!("CARGO_PKG_REPOSITORY"),
    "\n",
    "License: ",
    env!("CARGO_PKG_LICENSE"),
);
