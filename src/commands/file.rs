use std::{
    collections::HashSet,
    fs::File,
    io::{ErrorKind, Write},
    path::PathBuf,
    process::exit,
    sync::Arc,
};

use anyhow::{Context as _, bail};
use azure_identity::DefaultAzureCredential;
use azure_storage_blob::{
    BlobClient,
    models::{BlobClientDownloadResultHeaders, BlockBlobClientUploadOptions},
};
use futures::{TryStreamExt, stream::FuturesUnordered};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::fs::File as AsyncFile;
use tracing::info;
use typespec_client_core::{
    fs::FileStreamBuilder,
    http::{StatusCode, response::ResponseBody},
};
use url::Url;

use crate::{
    cli::{GlobalOptions, SyncFileOptions, SyncMode},
    commands::Command,
    dotenv::DotenvFile,
    sync::{SyncAction, SyncType, confirm},
};

const MODIFIED_META: &str = "modified";

impl Command for SyncFileOptions {
    async fn execute(self, global_options: &GlobalOptions) -> anyhow::Result<()> {
        // Load dotenv file
        let dotenv = if global_options.no_env_file {
            None
        } else {
            DotenvFile::from_path_exists(&global_options.env_file)?
        };

        // De-dupe the input paths to better support shell-level globbing
        let paths: HashSet<_> = self
            .paths
            .into_iter()
            .map(|path| {
                match path.canonicalize() {
                    Ok(path) => Ok(path),
                    // File doesn't exist yet, so can't be canonicalized
                    Err(error) if error.kind() == ErrorKind::NotFound => Ok(path),
                    // Other type of I/O error
                    Err(error) => Err(error),
                }
            })
            .collect::<Result<_, _>>()?;

        // Convert to an ordered list so that we can track associated blob names
        let paths = Vec::from_iter(paths);

        // Ensure all blob names are unique
        let (blob_names, duplicate_names) = paths.iter().try_fold(
            (
                Vec::with_capacity(paths.len()),
                HashSet::with_capacity(paths.len()),
            ),
            |(mut blob_names, mut duplicates), path| -> anyhow::Result<_> {
                // Get path parts
                let mut name = path
                    .file_name()
                    .context("Expected path to file")
                    .and_then(|name| name.to_str().context("File name must be valid Unicode"));
                let mut stem = path
                    .file_stem()
                    .context("Expected path to file")
                    .and_then(|stem| stem.to_str().context("File stem must be valid Unicode"));
                let mut ext = path
                    .extension()
                    .context("No file extension")
                    .and_then(|ext| ext.to_str().context("File extension must be valid Unicode"));

                /// Tries to copy the `Ok` variant out of a result.
                ///
                /// This replaces the result with `Ok(value)`.
                macro_rules! copy_try {
                    ($result:ident) => {{
                        let value = $result?;
                        $result = Ok(value);
                        value
                    }};
                }

                // Format blob name
                let mut blob_name = String::with_capacity(path.as_os_str().len());
                let mut placeholder = false;
                for part in self.blob_name.split('#') {
                    if placeholder {
                        let inserted = match part {
                            "name" => copy_try!(name),
                            "stem" => copy_try!(stem),
                            "ext" => copy_try!(ext),
                            other => bail!("Invalid placeholder: {other:?}"),
                        };
                        blob_name.push_str(inserted);
                    } else {
                        blob_name.push_str(part);
                    }
                    placeholder = !placeholder;
                }

                // Make sure the right number of #s are found
                if !placeholder {
                    bail!("Blob name is malformed (invalid number of #s)");
                }

                // Check if it's a duplicate
                if blob_names.contains(&blob_name) {
                    // Duplicate name
                    duplicates.insert(blob_name);
                } else {
                    // Unique name
                    blob_names.push(blob_name);
                }

                Ok((blob_names, duplicates))
            },
        )?;

        // Check if we had duplicate names
        if !duplicate_names.is_empty() {
            // Format the names
            let duplicate_names = Vec::from_iter(duplicate_names).join(", ");
            bail!("Duplicate blob names: {duplicate_names}");
        }

        // Convert each input path to an action
        let credential =
            DefaultAzureCredential::new().context("Failed to get default Azure credential")?;
        let endpoint = self
            .azure_storage
            .storage_account_url
            .resolve(dotenv.as_ref())?;
        let container_name = self.azure_storage.container_name.resolve(dotenv.as_ref())?;
        let actions: FuturesUnordered<_> = paths
            .into_iter()
            .zip(blob_names)
            .map(|(path, blob_name)| {
                get_file_action(
                    path,
                    blob_name,
                    credential.clone(),
                    &endpoint,
                    &container_name,
                    self.sync.sync_mode,
                )
            })
            .collect();
        let mut actions: Vec<_> = actions.try_collect().await?;
        actions.sort();

        // Print actions to the user
        info!("Using:");
        info!("  Endpoint: {endpoint}");
        info!("  Container: {container_name}");
        info!("Actions:");
        for action in &actions {
            match action {
                SyncType::Push(inner) => info!(
                    "<- PUSH: {} <- {}",
                    inner.context.blob_name,
                    inner.context.local_path.display(),
                ),
                SyncType::Pull(inner) => info!(
                    "-> PULL: {} -> {}",
                    inner.context.blob_name,
                    inner.context.local_path.display(),
                ),
                SyncType::Skip { reason, data } => info!(
                    "   SKIP ({reason}): {} -- {}",
                    data.blob_name,
                    data.local_path.display(),
                ),
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

        // Execute the action
        let actions: FuturesUnordered<_> = actions.into_iter().map(SyncAction::execute).collect();
        actions.try_collect::<()>().await?;

        Ok(())
    }
}

async fn get_file_action(
    local_path: PathBuf,
    blob_name: String,
    credential: Arc<DefaultAzureCredential>,
    endpoint: &Url,
    container_name: &str,
    sync_mode: SyncMode,
) -> anyhow::Result<SyncType<PushFile, PullFile, Context>> {
    // Open the local file
    let file = match File::open(&local_path) {
        Ok(file) => Some(file),
        Err(error) => {
            if error.kind() == ErrorKind::NotFound {
                None
            } else {
                bail!(error);
            }
        }
    };

    // Get the local modified time
    let local_modified = file
        .as_ref()
        .map(|file| file.metadata()?.modified())
        .transpose()?
        .map(OffsetDateTime::from);

    // Open the remote blob
    let client = BlobClient::new(
        endpoint.as_str(),
        container_name.to_string(),
        blob_name.clone(),
        credential,
        None,
    )?;
    let (remote_blob, remote_modified) = match client.download(None).await {
        Ok(blob) => {
            // Get when the remote blob was last modified
            let remote_modified = blob
                .metadata()?
                .get(MODIFIED_META)
                .map(|time| OffsetDateTime::parse(time, &Rfc3339))
                .transpose()?;
            let remote_modified = match remote_modified {
                Some(time) => time,
                None => blob
                    .last_modified()?
                    .context("unable to determine when blob was modified")?,
            };

            (Some(blob), Some(remote_modified))
        }
        Err(error) => {
            // Only allow NotFound - fail otherwise
            if error.http_status() != Some(StatusCode::NotFound) {
                bail!(error);
            }

            (None, None)
        }
    };

    let context = Context {
        local_path: local_path.clone(),
        blob_name,
    };
    Ok(SyncType::from_modified(
        sync_mode,
        local_modified,
        remote_modified,
        remote_blob,
        |local_modified, remote_blob| PushFile {
            context: context.clone(),
            client,
            local_modified,
            remote_etag: remote_blob.and_then(|blob| blob.etag().ok().flatten()),
        },
        |remote_modified, remote_blob| PullFile {
            context: context.clone(),
            remote_blob: remote_blob
                .expect("remote blob should be Some")
                .into_raw_body(),
            remote_modified,
        },
        |_| context.clone(),
    ))
}

#[derive(Clone, Debug)]
struct Context {
    local_path: PathBuf,
    blob_name: String,
}

sortable_by_key!(Context, str, |context| &context.blob_name);

struct PullFile {
    context: Context,
    remote_blob: ResponseBody,
    remote_modified: OffsetDateTime,
}

sortable_by_key!(PullFile, Context, |action| &action.context);

impl SyncAction for PullFile {
    async fn execute(mut self) -> anyhow::Result<()> {
        // Save the file to disk
        let mut file = File::create(self.context.local_path)?;
        while let Some(chunk) = self.remote_blob.try_next().await? {
            file.write_all(&chunk)?;
        }
        file.set_modified(self.remote_modified.into())?;

        Ok(())
    }
}

struct PushFile {
    context: Context,
    client: BlobClient,
    local_modified: OffsetDateTime,
    remote_etag: Option<String>,
}

sortable_by_key!(PushFile, Context, |action| &action.context);

impl SyncAction for PushFile {
    async fn execute(self) -> anyhow::Result<()> {
        let local_file = AsyncFile::open(self.context.local_path).await?;
        let content_length = local_file.metadata().await?.len();
        let stream = FileStreamBuilder::new(local_file).build().await?;
        let metadata = [(
            MODIFIED_META.to_string(),
            self.local_modified.format(&Rfc3339)?,
        )]
        .into_iter()
        .collect();

        self.client
            .upload(
                stream.into(),
                true,
                content_length,
                Some(BlockBlobClientUploadOptions {
                    if_match: self.remote_etag,
                    metadata: Some(metadata),
                    ..Default::default()
                }),
            )
            .await?;

        Ok(())
    }
}
