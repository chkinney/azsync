use clap::{Args, ValueEnum};

/// Options for generating shell completions.
#[derive(Clone, Debug, Args)]
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
