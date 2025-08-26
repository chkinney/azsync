use std::path::PathBuf;

use clap::Args;

use crate::cli::{KeyVaultOptions, SyncOptions};

/// Options for configuring syncing a dotenv file.
#[derive(Clone, Debug, Args)]
pub struct SyncDotenvOptions {
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
    pub template_file: PathBuf,

    /// Disable --template-file.
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

    /// Options for configuring how to synchronize with Azure.
    #[command(flatten)]
    pub sync: SyncOptions,

    /// Options for configuring the Key Vault.
    #[command(flatten)]
    pub key_vault: KeyVaultOptions,
}
