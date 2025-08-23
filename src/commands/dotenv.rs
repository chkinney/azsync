use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs::File,
    future::ready,
    io::{Write, stdin, stdout},
    process::exit,
    sync::Arc,
};

use anyhow::{Context, bail};
use azure_identity::DefaultAzureCredential;
use azure_security_keyvault_secrets::{SecretClient, models::SetSecretParameters};
use futures::{StreamExt, TryStreamExt, future::ok, stream::FuturesUnordered};
use time::OffsetDateTime;
use tracing::info;

use crate::{
    cli::{DotenvOptions, GlobalOptions, SyncMode},
    dotenv::DotenvFile,
};

impl DotenvOptions {
    /// Execute this subcommand.
    pub async fn execute(self, _global_options: &GlobalOptions) -> anyhow::Result<()> {
        // Load dotenv file
        let dotenv = DotenvFile::from_path_exists(&self.dotenv)?;
        let template = if self.no_template {
            None
        } else {
            DotenvFile::from_path_exists(&self.template)?
        };

        // Collect list of variables to synchronize
        let vars_to_sync: HashSet<_> = template
            .as_ref()
            .map(|template| template.parameters.keys())
            .or_else(|| Some(dotenv.as_ref()?.parameters.keys()))
            .context("Cannot synchronize without a dotenv or dotenv template file")?
            .map(String::as_str)
            .collect();
        info!(local_vars=?vars_to_sync.iter());

        // Create client
        let credential =
            DefaultAzureCredential::new().context("Failed to get default Azure credential")?;
        let key_vault_url = self.key_vault.resolve_url(dotenv.as_ref())?;
        let client = SecretClient::new(key_vault_url.as_str(), credential, None)
            .context("Failed to create Key Vault secrets client")?;

        // Get synchronized secrets from Key Vault
        let remote_vars =
            get_remote_vars(&client, self.sync_mode, vars_to_sync.iter().copied()).await?;
        info!(remote_vars=?remote_vars.keys());

        // Create a list of actions to execute
        let local_modified = dotenv
            .as_ref()
            .and_then(|dotenv| dotenv.last_modified)
            .map(OffsetDateTime::from);
        let mut actions: Vec<_> = vars_to_sync
            .into_iter()
            .map(|name| {
                let local_value = dotenv
                    .as_ref()
                    .and_then(|dotenv| dotenv.parameters.get(name))
                    .cloned();
                let (remote_value, remote_modified) = remote_vars
                    .get(name)
                    .map(|&(ref value, modified)| (value.clone(), modified))
                    .unzip();

                let name = name.to_string();
                match self.sync_mode {
                    SyncMode::Sync => VarAction::sync(
                        name,
                        local_value,
                        remote_value,
                        local_modified,
                        remote_modified.flatten(),
                    ),
                    SyncMode::Push => VarAction::sync(
                        name,
                        local_value,
                        remote_value,
                        local_modified,
                        remote_modified.flatten(),
                    )
                    .push_only(),
                    SyncMode::Pull => VarAction::sync(
                        name,
                        local_value,
                        remote_value,
                        local_modified,
                        remote_modified.flatten(),
                    )
                    .pull_only(),
                    SyncMode::PushAlways => match local_value {
                        Some(value) => VarAction::Push { name, value },
                        None => VarAction::Skip {
                            name,
                            reason: "No local value",
                        },
                    },
                    SyncMode::PullAlways => match remote_value {
                        Some(value) => VarAction::Pull { name, value },
                        None => VarAction::Skip {
                            name,
                            reason: "No remote value",
                        },
                    },
                }
            })
            .collect();
        actions.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        // Print actions to the user
        println!("Actions:");
        for action in &actions {
            match action {
                VarAction::Pull { name, .. } => println!("-> PULL: {name}"),
                VarAction::Push { name, .. } => println!("<- PUSH: {name}"),
                VarAction::Skip { name, reason } => println!("   SKIP: {name} ({reason})"),
            }
        }

        // If we're only checking, make no changes
        let unchanged = actions
            .iter()
            .all(|action| matches!(action, VarAction::Skip { .. }));
        if self.check_only || unchanged {
            exit(i32::from(!unchanged));
        }

        // Ask for confirmation
        println!();
        confirm()?;

        // Split into push and pull actions
        let push = FuturesUnordered::new();
        let mut pull = HashMap::with_capacity(actions.len());
        let client = Arc::new(client);
        for action in actions {
            match action {
                VarAction::Pull { name, value } => {
                    pull.insert(name, value);
                }
                VarAction::Push { name, value } => {
                    let params = SetSecretParameters {
                        content_type: Some("text/plain".into()),
                        value: Some(value),
                        ..Default::default()
                    };
                    let client = client.clone();

                    // `async move` is to give the future ownership of `name`
                    // (otherwise the future isn't `'static`)
                    let fut = async move {
                        let name = name.replace('_', "-");
                        client.set_secret(&name, params.try_into()?, None).await
                    };
                    push.push(fut);
                }
                VarAction::Skip { .. } => {}
            }
        }

        // Update local file
        let new_source = if let Some(dotenv) = dotenv {
            dotenv.replace(pull)
        } else {
            DotenvFile::default().replace(pull)
        };
        let mut file = File::create(&self.dotenv)?;
        write!(file, "{new_source}")?;

        // Update remote values
        push.map_ok(|_| {}).try_collect::<()>().await?;

        Ok(())
    }
}

async fn get_remote_vars(
    client: &SecretClient,
    mode: SyncMode,
    var_names: impl IntoIterator<Item = &str>,
) -> anyhow::Result<HashMap<String, (String, Option<OffsetDateTime>)>> {
    if let SyncMode::PushAlways = mode {
        // Don't pull any values
        return Ok(HashMap::new());
    }

    // Get synchronized secrets from Key Vault
    let remote_vars: Vec<_> = var_names
        .into_iter()
        .map(|name| name.replace('_', "-"))
        .collect();
    let remote_vars: FuturesUnordered<_> = remote_vars
        .iter()
        .map(|var_name| client.get_secret(var_name, "", None))
        .collect();

    #[expect(clippy::redundant_closure_for_method_calls, reason = "Opaque type")]
    let remote_vars: HashMap<_, _> = remote_vars
        .filter(|result| match result {
            Ok(_) => ready(true),
            Err(error) => ready(error.http_status() != Some(404.into())),
        })
        .and_then(|response| response.into_body())
        .map_ok(|secret| {
            let name = secret.id?.split('/').nth_back(1)?.replace('-', "_");
            let value = secret.value?;
            let modified = secret
                .attributes
                .and_then(|attributes| attributes.updated.or(attributes.created));
            Some((name, (value, modified)))
        })
        .try_filter_map(ok)
        .try_collect()
        .await
        .context("Failed to load secrets from Key Vault")?;

    Ok(remote_vars)
}

fn confirm() -> anyhow::Result<()> {
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

#[derive(Clone)]
enum VarAction {
    Pull { name: String, value: String },
    Push { name: String, value: String },
    Skip { name: String, reason: &'static str },
}

impl VarAction {
    /// Convert this action to skip if it's not a push action.
    pub fn push_only(self) -> Self {
        match self {
            VarAction::Pull { name, .. } => Self::Skip {
                name,
                reason: "pull disabled",
            },
            action => action,
        }
    }

    /// Convert this action to skip if it's not a pull action.
    pub fn pull_only(self) -> Self {
        match self {
            VarAction::Push { name, .. } => Self::Skip {
                name,
                reason: "push disabled",
            },
            action => action,
        }
    }

    pub fn sync(
        name: String,
        local_value: Option<String>,
        remote_value: Option<String>,
        local_modified: Option<OffsetDateTime>,
        remote_modified: Option<OffsetDateTime>,
    ) -> Self {
        match (local_value, remote_value) {
            // Unchanged
            (Some(local_value), Some(remote_value)) if local_value == remote_value => Self::Skip {
                name,
                reason: "unchanged",
            },

            // Conflict
            (Some(local_value), Some(remote_value)) => {
                // Determine newer version
                match (local_modified, remote_modified) {
                    // Remote is newer
                    (Some(local_modified), Some(remote_modified))
                        if remote_modified >= local_modified =>
                    {
                        Self::Pull {
                            name,
                            value: remote_value,
                        }
                    }

                    // Local is newer (or unknown when remote was modified)
                    (Some(_), _) => Self::Push {
                        name,
                        value: local_value,
                    },

                    // Unknown when local was modified
                    (None, Some(_)) => Self::Pull {
                        name,
                        value: remote_value,
                    },

                    // Unknown when either was modified
                    (None, None) => Self::Skip {
                        name,
                        reason: "unknown modified times",
                    },
                }
            }

            // Only available locally
            (Some(local_value), _) => Self::Push {
                name,
                value: local_value,
            },

            // Only available in remote
            (_, Some(remote_value)) => Self::Pull {
                name,
                value: remote_value,
            },

            // Not available in either (only in dotenv template for example)
            (None, None) => Self::Skip {
                name,
                reason: "no value found",
            },
        }
    }
}

impl PartialEq for VarAction {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd for VarAction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Push { name: a, .. }, Self::Push { name: b, .. })
            | (Self::Pull { name: a, .. }, Self::Pull { name: b, .. })
            | (Self::Skip { name: a, .. }, Self::Skip { name: b, .. }) => a.partial_cmp(b),

            (Self::Push { .. }, _) => Some(Ordering::Less),
            (_, Self::Push { .. }) => Some(Ordering::Greater),

            (Self::Pull { .. }, _) => Some(Ordering::Less),
            (_, Self::Pull { .. }) => Some(Ordering::Greater),
        }
    }
}
