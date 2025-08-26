use clap::Args;
use url::Url;

use crate::cli::MaybeEnv;

/// Options for configuring the Key Vault instance.
#[derive(Clone, Debug, Args)]
#[command(next_help_heading = "Key Vault")]
pub struct KeyVaultOptions {
    /// The URL to the Key Vault instance.
    ///
    /// To use an environment variable instead, use the `env:` scheme. For
    /// example, `env:KEY_VAULT_URL` will use the value of the environment
    /// variable `KEY_VAULT_URL` to determine the URL.
    ///
    /// If a local dotenv file is present, the `env:` scheme will first search
    /// that file for a value. If it's not found in that file, or if no dotenv
    /// file is present, then the program's environment variables will be
    /// searched instead.
    #[arg(long, default_value = "env:KEY_VAULT_URL")]
    pub key_vault_url: MaybeEnv<Url>,
}
