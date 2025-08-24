use crate::cli::GlobalOptions;

/// An executable CLI subcommand.
pub trait Command: Sized {
    /// Execute this command.
    async fn execute(self, global_options: &GlobalOptions) -> anyhow::Result<()>;
}
