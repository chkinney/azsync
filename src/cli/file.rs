use std::path::PathBuf;

use clap::Args;

use crate::cli::{AzureStorageOptions, SyncOptions};

/// Options for synchronizing files.
#[derive(Clone, Debug, Args)]
pub struct SyncFileOptions {
    /// The files to sync.
    ///
    /// NOTE ON GLOBBING (*.json):
    ///
    /// On most shells, globs are expanded by the shell. For example, the path
    /// *.json will usually be converted by your shell into a list of all files
    /// matching that pattern (like foo.json bar.json). When shells expand these
    /// patterns, THEY ONLY INCLUDE FILES ON YOUR SYSTEM. Applications do not
    /// receive the pattern. They only receive the list of paths that matched it
    /// so they have no way of knowing what the pattern was.
    ///
    /// As a result, if you want to synchronize a file that does not exist on
    /// your system, you MUST specify it literally. For example, if you want to
    /// pull the file foo.json, you MUST specify foo.json because *.json will
    /// not be expanded by your shell to include it.
    ///
    /// There is currently no way to pull all files matching a pattern from the
    /// remote storage. IF YOU WANT TO SYNCHRONIZE A DIRECTORY, ARCHIVE IT
    /// FIRST. You can synchronize foos.zip easily because it is only one file.
    #[arg(required = true, num_args = 1..)]
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
