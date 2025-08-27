use std::path::PathBuf;

use clap::Args;

use crate::cli::{AzureStorageOptions, SyncOptions};

/// Options for synchronizing files.
#[derive(Clone, Debug, Args)]
pub struct SyncFileOptions {
    /// The file to sync.
    pub path: PathBuf,

    /// The name of the remote blob.
    ///
    /// If not provided, the name of the file being synchronized is used as the
    /// blob name instead.
    #[arg(long)]
    pub blob_name: Option<String>,

    /// Options for configuring how to synchronize with Azure.
    #[command(flatten)]
    pub sync: SyncOptions,

    /// Options for configuring the Storage Account.
    #[command(flatten)]
    pub azure_storage: AzureStorageOptions,
}
