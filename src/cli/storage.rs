use clap::Args;
use url::Url;

use crate::cli::MaybeEnv;

/// Options for configuring the Azure Storage instance.
#[derive(Clone, Debug, Args)]
#[command(next_help_heading = "Blob Storage")]
pub struct AzureStorageOptions {
    /// The storage account's endpoint.
    ///
    /// This is usually in the format `https://<name>.blob.core.windows.net/`.
    ///
    /// To use an environment variable instead, use the `env:` scheme. For
    /// example, `env:STORAGE_ACCOUNT_URL` will use the value of the environment
    /// variable `STORAGE_ACCOUNT_URL` to determine the URL.
    ///
    /// If a local dotenv file is present, the `env:` scheme will first search
    /// that file for a value. If it's not found in that file, or if no dotenv
    /// file is present, then the program's environment variables will be
    /// searched instead.
    #[arg(long, default_value = "env:STORAGE_ACCOUNT_URL")]
    pub storage_account_url: MaybeEnv<Url>,

    /// The name of the container in the storage account.
    ///
    /// To use an environment variable instead, use the `env:` scheme. For
    /// example, `env:STORAGE_ACCOUNT_CONTAINER` will use the value of the
    /// environment variable `STORAGE_ACCOUNT_CONTAINER` to determine the URL.
    ///
    /// If a local dotenv file is present, the `env:` scheme will first search
    /// that file for a value. If it's not found in that file, or if no dotenv
    /// file is present, then the program's environment variables will be
    /// searched instead.
    #[arg(long, default_value = "env:STORAGE_ACCOUNT_CONTAINER")]
    pub container_name: MaybeEnv<String>,
}
