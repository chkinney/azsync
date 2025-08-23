use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use clap_cargo::style::CLAP_STYLING;
use url::Url;

/// Quickly synchronize local files with Azure.
///
/// This requires you to be authenticated to Azure already as it uses the
/// default Azure credential for this environment. If needed, use the Azure CLI
/// to login and select the subscription you want to use.
#[derive(Clone, Debug, Parser)]
#[command(styles = CLAP_STYLING)]
pub struct Cli {
    /// The subcommand to execute.
    #[command(subcommand)]
    pub subcommand: CliCommand,

    /// Global options relevant to all subcommands.
    #[command(flatten)]
    pub global: GlobalOptions,
}

/// Global options that are always relevant.
#[derive(Clone, Debug, Parser)]
#[command(version, next_help_heading = "Global")]
pub struct GlobalOptions {
    /// Enable more verbose output (repeatable up to 3 times).
    ///
    /// Output is emitted via stderr.
    #[arg(global = true, long, short = 'v', action = ArgAction::Count)]
    pub verbose: u8,
}

/// A subcommand to execute.
#[derive(Clone, Debug, Parser)]
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
    Dotenv(DotenvOptions),
}

/// Options for generating shell completions.
#[derive(Clone, Debug, Parser)]
#[command(hide = true)] // Not relevant except during installation
pub struct CompletionsOptions {
    /// The shell to generate completions for.
    #[arg(value_enum)]
    #[cfg_attr(
        any(target_os = "windows", target_os = "macos", target_os = "linux"),
        arg(default_value_t),
        doc = "",
        doc = " If not provided, a default shell will be selected for your platform."
    )]
    pub shell: Shell,
}

/// A shell that completions can be generated for.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, ValueEnum)]
#[cfg_attr(
    any(target_os = "windows", target_os = "macos", target_os = "linux"),
    derive(Default)
)]
pub enum Shell {
    #[value(name = "bash")]
    #[cfg_attr(target_os = "linux", default)]
    Bash,

    #[cfg_attr(target_os = "windows", default)]
    #[value(name = "pwsh", alias = "powershell")]
    PowerShell,

    #[value(name = "zsh")]
    #[cfg_attr(target_os = "macos", default)]
    Zsh,

    #[value(name = "elvish")]
    Elvish,

    #[value(name = "fish")]
    Fish,

    #[value(name = "nushell", alias = "nu")]
    Nushell,
}

/// Options for configuring syncing a dotenv file.
#[derive(Clone, Debug, Parser)]
pub struct DotenvOptions {
    /// The dotenv file to synchronize.
    #[arg(default_value = ".env")]
    pub dotenv: PathBuf,

    /// The dotenv template file.
    ///
    /// If present, variable names defined in it will be the ONLY variables
    /// synchronized in the (non-template) dotenv file. Variables that are
    /// missing from the dotenv file and present in Azure will be added to the
    /// dotenv file.
    ///
    /// Note that values defined in the template file will not be used, nor
    /// will that file be modified in any manner.
    ///
    /// If the file does not exist, this option is ignored.
    #[arg(long, short = 't', default_value = ".env.example")]
    pub template: PathBuf,

    /// How to synchronize values.
    ///
    /// By default, the newest values are stored both locally and in Azure. If
    /// a newer value is available locally, it's pushed. Otherwise, if a newer
    /// value is present in Azure, it's pulled.
    ///
    /// This behavior can be changed. For example, if you do not have permission
    /// to push variables to Azure, you can still pull them as long as you have
    /// permission to read their values. Both `pull` and `pull-always` will do
    /// this, though the latter will always overwrite any locally stored values.
    ///
    /// `pull` always indicates reading from Azure and storing locally. `push`
    /// always indicates reading locally and sending to Azure. Values that end
    /// in `-always` will always push/pull relevant values.
    #[arg(long, short = 'm', value_enum, default_value_t)]
    pub sync_mode: SyncMode,

    /// Only check if anything needs to be synchronized.
    ///
    /// Any changes that need to be made are printed to stdout.
    ///
    /// The application returns an error status if the local dotenv is out
    /// of sync.
    #[arg(long, short = 'c')]
    pub check_only: bool,

    /// Don't ask for confirmation before synchronizing.
    ///
    /// Normally, you will be asked before any changes are made locally or in
    /// Azure. Passing this flag skips that step. You will still be informed of
    /// changes being made, but you will not be asked to confirm them.
    ///
    /// This is a potentially destructive action. Use with caution.
    #[arg(long, short = 'y')]
    pub no_confirm: bool,

    /// Don't use a template file.
    ///
    /// If the template file exists, it will be ignored. Instead, all variables
    /// defined in your dotenv file will be synchronized instead. No additional
    /// variables will be pulled from Azure, nor will any value saved locally be
    /// ignored when synchronizing.
    ///
    /// This is equivalent to synchronizing without a template file. If the
    /// template file doesn't exist, this flag does nothing.
    #[arg(long)]
    pub no_template: bool,

    /// Options for configuring the Key Vault.
    #[command(flatten)]
    pub key_vault: KeyVaultOptions,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default, ValueEnum)]
pub enum SyncMode {
    /// Push if local is newer, pull if remote is newer.
    ///
    /// This ensures the latest value is stored both locally and remotely.
    #[default]
    #[value(name = "sync")]
    Sync,

    /// Only push, and only if newer.
    ///
    /// This ensures the latest value is stored remotely.
    #[value(name = "push")]
    Push,

    /// Only pull, and only if newer.
    ///
    /// This ensures the latest value is stored locally.
    #[value(name = "pull")]
    Pull,

    /// Always push.
    ///
    /// This overwrites the remote value (or creates it).
    #[value(name = "push-always")]
    PushAlways,

    /// Always pull.
    ///
    /// This overwrites the remote value (or creates it).
    #[value(name = "pull-always")]
    PullAlways,
}

/// Options for configuring the Key Vault instance.
#[derive(Clone, Debug, Parser)]
#[command(next_help_heading = "Key Vault")]
pub struct KeyVaultOptions {
    /// The URL to the Key Vault instance.
    ///
    /// To use an environment variable instead, use the `env://` scheme. For
    /// example, `env://KEY_VAULT_URL` will use the value of the environment
    /// variable `KEY_VAULT_URL` to determine the URL.
    ///
    /// If a local dotenv file is present, the `env://` scheme will first search
    /// that file for a value. If it's not found in that file, or if no dotenv
    /// file is present, then the program's environment variables will be
    /// searched instead.
    #[arg(long, default_value = "env://KEY_VAULT_URL")]
    pub key_vault_url: Url,
}
