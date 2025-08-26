use std::path::PathBuf;

use clap::Args;

use crate::cli::{AzureStorageOptions, SyncOptions};

/// Options for synchronizing files.
#[derive(Clone, Debug, Args)]
pub struct SyncFileOptions {
    /// The file or directory to sync.
    pub path: PathBuf,

    /// The name of the remote blob.
    ///
    /// If not provided, the name of the file or directory being synchronized is
    /// used as the blob name instead. Directories will be suffixed depending on
    /// the archive type chosen.
    #[arg(long)]
    pub blob_name: Option<String>,

    /// Options for configuring how to synchronize with Azure.
    #[command(flatten)]
    pub sync: SyncOptions,

    /// Options for configuring the Storage Account.
    #[command(flatten)]
    pub azure_storage: AzureStorageOptions,
}
