use clap::{Args, ValueEnum};

/// Options for synchronizing between local and remote.
#[derive(Clone, Debug, Args)]
pub struct SyncOptions {
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
}

/// Mode for synchronizing between local and remote.
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
