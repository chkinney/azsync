use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    fs::File,
    future::ready,
    io::Write,
    process::exit,
    sync::{
        Arc,
        mpsc::{Sender, channel},
    },
};

use anyhow::Context;
use azure_identity::DefaultAzureCredential;
use azure_security_keyvault_secrets::{SecretClient, models::SetSecretParameters};
use futures::{StreamExt, TryStreamExt, future::ok, stream::FuturesUnordered};
use time::OffsetDateTime;
use tracing::{debug, info};

use crate::{
    cli::{GlobalOptions, SyncDotenvOptions, SyncMode},
    commands::Command,
    dotenv::DotenvFile,
    sync::{SyncAction, SyncType, confirm},
};

impl Command for SyncDotenvOptions {
    async fn execute(self, global_options: &GlobalOptions) -> anyhow::Result<()> {
        // Load dotenv file
        let dotenv = DotenvFile::from_path_exists(&global_options.env_file)?;
        let template = if self.no_template {
            None
        } else {
            DotenvFile::from_path_exists(&self.template_file)?
        };

        // Collect list of variables to synchronize
        let vars_to_sync: HashSet<_> = template
            .as_ref()
            .map(|template| template.parameters.keys())
            .or_else(|| Some(dotenv.as_ref()?.parameters.keys()))
            .context("Cannot synchronize without a dotenv or dotenv template file")?
            .map(String::as_str)
            .collect();
        debug!(local_vars=?vars_to_sync.iter());

        // Create client
        let credential =
            DefaultAzureCredential::new().context("Failed to get default Azure credential")?;
        let key_vault_url = self
            .key_vault
            .key_vault_url
            .resolve(dotenv.as_ref().filter(|_| global_options.no_env_file))?;
        info!("Using:");
        info!("  Key Vault: {key_vault_url}");
        let client = SecretClient::new(key_vault_url.as_str(), credential, None)
            .context("Failed to create Key Vault secrets client")?;

        // Get synchronized secrets from Key Vault
        let remote_vars =
            get_remote_vars(&client, self.sync.sync_mode, vars_to_sync.iter().copied()).await?;
        debug!(remote_vars=?remote_vars.keys());

        // Create a list of actions to execute
        let client = Arc::new(client);
        let (pairs_tx, pairs_rx) = channel();
        let local_modified = dotenv.as_ref().and_then(|dotenv| dotenv.last_modified);
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

                // Check if values are equal
                if local_value
                    .as_ref()
                    .zip(remote_value.as_ref())
                    .is_some_and(|(a, b)| a == b)
                {
                    return SyncType::Skip {
                        reason: "unchanged",
                        data: name.to_string(),
                    };
                }

                SyncType::from_modified(
                    self.sync.sync_mode,
                    local_value.as_ref().and(local_modified),
                    remote_modified.flatten(),
                    name,
                    |_, name| PushVar {
                        name: name.to_string(),
                        value: local_value.expect("local value should be Some"),
                        client: client.clone(),
                    },
                    |remote_modified, name| PullVar {
                        name: name.to_string(),
                        value: remote_value.expect("remote value should be Some"),
                        remote_modified,
                        pairs_tx: pairs_tx.clone(),
                    },
                    ToString::to_string,
                )
            })
            .collect();
        actions.sort_unstable();

        // Print actions to the user
        info!("Actions:");
        for action in &actions {
            match action {
                SyncType::Pull(PullVar { name, .. }) => info!("-> PULL: {name}"),
                SyncType::Push(PushVar { name, .. }) => info!("<- PUSH: {name}"),
                SyncType::Skip { reason, data } => info!("   SKIP: {data} ({reason})"),
            }
        }

        // If we're only checking, make no changes
        let unchanged = actions
            .iter()
            .all(|action| matches!(action, SyncType::Skip { .. }));
        if self.sync.check_only || unchanged {
            exit(i32::from(!unchanged));
        }

        // Ask for confirmation
        if !self.sync.no_confirm {
            confirm()?;
        }

        // Get the latest that the remote was modified for the dotenv
        let new_modified = actions
            .iter()
            .filter_map(|action| {
                if let SyncType::Pull(PullVar {
                    remote_modified, ..
                }) = action
                {
                    Some(*remote_modified)
                } else {
                    None
                }
            })
            .max();

        // Execute the actions
        let actions: FuturesUnordered<_> = actions.into_iter().map(SyncAction::execute).collect();
        actions.try_collect::<()>().await?;

        // Update local file
        drop(pairs_tx); // to allow the channel to close after actions complete
        let replacements: HashMap<_, _> = pairs_rx.into_iter().collect();
        if !replacements.is_empty() {
            let new_source = if let Some(dotenv) = dotenv {
                dotenv.replace(replacements)
            } else {
                DotenvFile::default().replace(replacements)
            };
            let mut file = File::create(&global_options.env_file)?;
            write!(file, "{new_source}")?;
            file.flush()?;

            // Track the new modified time if it's later than the current modified time
            let new_modified = match (local_modified, new_modified) {
                (None, None) => None,
                (None, Some(time)) | (Some(time), None) => Some(time),
                (Some(a), Some(b)) => Some(max(a, b)),
            };
            if let Some(new_modified) = new_modified {
                file.set_modified(new_modified.into())?;
            }
        }

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

pub struct PullVar {
    name: String,
    value: String,
    remote_modified: OffsetDateTime,
    pairs_tx: Sender<(String, String)>,
}

sortable_by_key!(PullVar, str, |action| &action.name);

impl SyncAction for PullVar {
    async fn execute(self) -> anyhow::Result<()> {
        self.pairs_tx.send((self.name, self.value))?;
        Ok(())
    }
}

pub struct PushVar {
    name: String,
    value: String,
    client: Arc<SecretClient>,
}

sortable_by_key!(PushVar, str, |action| &action.name);

impl SyncAction for PushVar {
    async fn execute(self) -> anyhow::Result<()> {
        let params = SetSecretParameters {
            content_type: Some("text/plain".into()),
            value: Some(self.value),
            ..Default::default()
        };

        let name = self.name.replace('_', "-");
        self.client
            .set_secret(&name, params.try_into()?, None)
            .await?;

        Ok(())
    }
}
