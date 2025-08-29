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

#[cfg(test)]
mod tests {
    use test_case::test_case;
    use time::macros::datetime;

    use super::*;

    const DT_2024: OffsetDateTime = datetime!(2024-01-01 00:00 +00:00);
    const DT_2025: OffsetDateTime = datetime!(2025-01-01 00:00 +00:00);

    // SyncMode::Sync
    #[test_case(SyncMode::Sync, None, None => matches SyncType::Skip { .. }; "sync not-found")]
    #[test_case(SyncMode::Sync, Some(DT_2025), None => SyncType::Push(DT_2025); "sync local-only")]
    #[test_case(SyncMode::Sync, None, Some(DT_2025) => SyncType::Pull(DT_2025); "sync remote-only")]
    #[test_case(SyncMode::Sync, Some(DT_2025), Some(DT_2024) => SyncType::Push(DT_2025); "sync conflict local-newer")]
    #[test_case(SyncMode::Sync, Some(DT_2024), Some(DT_2025) => SyncType::Pull(DT_2025); "sync conflict remote-newer")]
    #[test_case(SyncMode::Sync, Some(DT_2025), Some(DT_2025) => matches SyncType::Skip { .. }; "sync conflict same-time")]
    // SyncMode::Push
    #[test_case(SyncMode::Push, None, None => matches SyncType::Skip { .. }; "push not-found")]
    #[test_case(SyncMode::Push, Some(DT_2025), None => SyncType::Push(DT_2025); "push local-only")]
    #[test_case(SyncMode::Push, None, Some(DT_2025) => matches SyncType::Skip { .. }; "push remote-only")]
    #[test_case(SyncMode::Push, Some(DT_2025), Some(DT_2024) => SyncType::Push(DT_2025); "push conflict local-newer")]
    #[test_case(SyncMode::Push, Some(DT_2024), Some(DT_2025) => matches SyncType::Skip { .. }; "push conflict remote-newer")]
    #[test_case(SyncMode::Push, Some(DT_2025), Some(DT_2025) => matches SyncType::Skip { .. }; "push conflict same-time")]
    // SyncMode::Pull
    #[test_case(SyncMode::Pull, None, None => matches SyncType::Skip { .. }; "pull not-found")]
    #[test_case(SyncMode::Pull, Some(DT_2025), None => matches SyncType::Skip { .. }; "pull local-only")]
    #[test_case(SyncMode::Pull, None, Some(DT_2025) => SyncType::Pull(DT_2025); "pull remote-only")]
    #[test_case(SyncMode::Pull, Some(DT_2025), Some(DT_2024) => matches SyncType::Skip { .. }; "pull conflict local-newer")]
    #[test_case(SyncMode::Pull, Some(DT_2024), Some(DT_2025) => SyncType::Pull(DT_2025); "pull conflict remote-newer")]
    #[test_case(SyncMode::Pull, Some(DT_2025), Some(DT_2025) => matches SyncType::Skip { .. }; "pull conflict same-time")]
    // SyncMode::PushAlways
    #[test_case(SyncMode::PushAlways, None, None => matches SyncType::Skip { .. }; "push-always not-found")]
    #[test_case(SyncMode::PushAlways, Some(DT_2025), None => SyncType::Push(DT_2025); "push-always local-only")]
    #[test_case(SyncMode::PushAlways, None, Some(DT_2025) => matches SyncType::Skip { .. }; "push-always remote-only")]
    #[test_case(SyncMode::PushAlways, Some(DT_2025), Some(DT_2024) => SyncType::Push(DT_2025); "push-always conflict local-newer")]
    #[test_case(SyncMode::PushAlways, Some(DT_2024), Some(DT_2025) => SyncType::Push(DT_2024); "push-always conflict remote-newer")]
    #[test_case(SyncMode::PushAlways, Some(DT_2025), Some(DT_2025) => SyncType::Push(DT_2025); "push-always conflict same-time")]
    // SyncMode::PullAlways
    #[test_case(SyncMode::PullAlways, None, None => matches SyncType::Skip { .. }; "pull-always not-found")]
    #[test_case(SyncMode::PullAlways, Some(DT_2025), None => matches SyncType::Skip { .. }; "pull-always local-only")]
    #[test_case(SyncMode::PullAlways, None, Some(DT_2025) => SyncType::Pull(DT_2025); "pull-always remote-only")]
    #[test_case(SyncMode::PullAlways, Some(DT_2025), Some(DT_2024) => SyncType::Pull(DT_2024); "pull-always conflict local-newer")]
    #[test_case(SyncMode::PullAlways, Some(DT_2024), Some(DT_2025) => SyncType::Pull(DT_2025); "pull-always conflict remote-newer")]
    #[test_case(SyncMode::PullAlways, Some(DT_2025), Some(DT_2025) => SyncType::Pull(DT_2025); "pull-always conflict same-time")]
    fn from_modified_correct_variant(
        sync_mode: SyncMode,
        local: Option<OffsetDateTime>,
        remote: Option<OffsetDateTime>,
    ) -> SyncType<OffsetDateTime, OffsetDateTime, ()> {
        SyncType::from_modified(
            sync_mode,
            local,
            remote,
            (),
            |time, ()| time,
            |time, ()| time,
            |()| (),
        )
    }
}
