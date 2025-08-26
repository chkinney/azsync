use std::io::{Write, stdin, stdout};

use anyhow::bail;
use time::{Duration, OffsetDateTime};

use crate::cli::SyncMode;

/// An action that can be taken on a synchronized resource.
pub trait SyncAction {
    /// Execute this action.
    async fn execute(self) -> anyhow::Result<()>;
}

/// A kind of synchronization operation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum SyncType<Push, Pull, Skip> {
    /// Push local data to remote storage.
    Push(Push),

    /// Pull remote data to local storage.
    Pull(Pull),

    /// Do nothing.
    Skip {
        /// The reason for skipping.
        reason: &'static str,

        /// Data associated with skipping.
        data: Skip,
    },
}

impl<Push, Pull, Skip> SyncType<Push, Pull, Skip> {
    /// Sync based on the last modified times of the local and remote value.
    #[must_use]
    pub fn from_modified<T>(
        sync_mode: SyncMode,
        local_modified: Option<OffsetDateTime>,
        remote_modified: Option<OffsetDateTime>,
        seed: T,
        push: impl FnOnce(OffsetDateTime, T) -> Push,
        pull: impl FnOnce(OffsetDateTime, T) -> Pull,
        skip: impl FnOnce(T) -> Skip,
    ) -> Self {
        match (local_modified, remote_modified) {
            // Both present but modified very close to each other
            (Some(local), Some(remote)) if (local - remote).abs() < Duration::minutes(1) => {
                match sync_mode {
                    SyncMode::Sync | SyncMode::Push | SyncMode::Pull => SyncType::Skip {
                        reason: "unchanged",
                        data: skip(seed),
                    },
                    SyncMode::PushAlways => Self::Push(push(local, seed)),
                    SyncMode::PullAlways => Self::Pull(pull(local, seed)),
                }
            }

            // Local newer
            (Some(local), Some(remote)) if local > remote => match sync_mode {
                SyncMode::Sync | SyncMode::Push | SyncMode::PushAlways => {
                    Self::Push(push(local, seed))
                }
                SyncMode::Pull => Self::Skip {
                    reason: "pull disabled",
                    data: skip(seed),
                },
                SyncMode::PullAlways => Self::Pull(pull(remote, seed)),
            },
            (Some(local), None) => match sync_mode {
                SyncMode::Sync | SyncMode::Push | SyncMode::PushAlways => {
                    Self::Push(push(local, seed))
                }
                SyncMode::Pull => Self::Skip {
                    reason: "pull disabled",
                    data: skip(seed),
                },
                SyncMode::PullAlways => Self::Skip {
                    reason: "nothing to pull",
                    data: skip(seed),
                },
            },

            // Remote newer
            (Some(local), Some(remote)) => match sync_mode {
                SyncMode::Sync | SyncMode::Pull | SyncMode::PullAlways => {
                    Self::Pull(pull(remote, seed))
                }
                SyncMode::Push => Self::Skip {
                    reason: "push disabled",
                    data: skip(seed),
                },
                SyncMode::PushAlways => Self::Push(push(local, seed)),
            },
            (None, Some(remote)) => match sync_mode {
                SyncMode::Sync | SyncMode::Pull | SyncMode::PullAlways => {
                    Self::Pull(pull(remote, seed))
                }
                SyncMode::Push => Self::Skip {
                    reason: "push disabled",
                    data: skip(seed),
                },
                SyncMode::PushAlways => Self::Skip {
                    reason: "nothing to push",
                    data: skip(seed),
                },
            },

            // Neither present
            (None, None) => Self::Skip {
                reason: "not found",
                data: skip(seed),
            },
        }
    }
}

impl<Push, Pull, Skip> SyncAction for SyncType<Push, Pull, Skip>
where
    Push: SyncAction,
    Pull: SyncAction,
{
    async fn execute(self) -> anyhow::Result<()> {
        match self {
            SyncType::Push(inner) => inner.execute().await,
            SyncType::Pull(inner) => inner.execute().await,
            SyncType::Skip { .. } => Ok(()),
        }
    }
}

/// Ask the user for confirmation on a set of actions.
pub fn confirm() -> anyhow::Result<()> {
    let mut input = String::new();
    loop {
        print!("Confirm (yes/no)? ");
        stdout().flush()?;
        input.clear();
        stdin().read_line(&mut input)?;

        match input.as_str().trim_end() {
            "y" | "yes" => return Ok(()),
            "n" | "no" => {
                bail!("Aborted");
            }
            _ => {
                // Ask again
            }
        }
    }
}
