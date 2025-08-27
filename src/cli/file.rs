use std::path::PathBuf;

use clap::Args;

use crate::cli::{AzureStorageOptions, SyncOptions};

/// Options for synchronizing files.
#[derive(Clone, Debug, Args)]
pub struct SyncFileOptions {
    /// The files to sync.
    pub paths: Vec<PathBuf>,

    // NOTE: clap doesn't format doc comments correctly for long help yet:
    // https://github.com/clap-rs/clap/issues/5900
    #[doc = include_str!("file.blob_name.txt")]
    #[arg(
        long,
        default_value = "#name#",
        help = "The name of the remote blob.",
        long_help = include_str!("file.blob_name.txt"),
    )]
    pub blob_name: String,

    /// Options for configuring how to synchronize with Azure.
    #[command(flatten)]
    pub sync: SyncOptions,

    /// Options for configuring the Storage Account.
    #[command(flatten)]
    pub azure_storage: AzureStorageOptions,
}
