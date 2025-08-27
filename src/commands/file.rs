use std::{
    fs::File,
    io::{ErrorKind, Write},
    path::PathBuf,
    process::exit,
};

use anyhow::{Context, bail};
use azure_identity::DefaultAzureCredential;
use azure_storage_blob::{
    BlobClient,
    models::{BlobClientDownloadResultHeaders, BlockBlobClientUploadOptions},
};
use futures::TryStreamExt;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::fs::File as AsyncFile;
use tracing::info;
use typespec_client_core::{
    fs::FileStreamBuilder,
    http::{StatusCode, response::ResponseBody},
};

use crate::{
    cli::{GlobalOptions, SyncFileOptions},
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
        let local_path = match self.path.canonicalize() {
            Ok(path) => path,
            // File doesn't exist yet, so can't be canonicalized
            Err(error) if error.kind() == ErrorKind::NotFound => self.path,
            // Other type of I/O error
            Err(error) => bail!(error),
        };
        let blob_name = self
            .blob_name
            .as_ref()
            .map(AsRef::as_ref)
            .or_else(|| local_path.file_name().and_then(|s| s.to_str()))
            .context("Blob name cannot be created from paths that are not UTF-8. Please provide a blob name.")?;

        // Create client
        let credential =
            DefaultAzureCredential::new().context("Failed to get default Azure credential")?;
        let endpoint = self
            .azure_storage
            .storage_account_url
            .resolve(dotenv.as_ref())?;
        let container_name = self.azure_storage.container_name.resolve(dotenv.as_ref())?;
        info!("Using:");
        info!("  Endpoint: {endpoint}");
        info!("  Container: {container_name}");
        info!("  Blob: {blob_name}");
        let client = BlobClient::new(
            endpoint.as_str(),
            container_name.into_owned(),
            blob_name.to_owned(),
            credential,
            None,
        )?;

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

        // Get the remote blob
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

        let action = SyncType::from_modified(
            self.sync.sync_mode,
            local_modified,
            remote_modified,
            remote_blob,
            |local_modified, remote_blob| PushFile {
                client,
                local_path: local_path.clone(),
                local_modified,
                remote_etag: remote_blob.and_then(|blob| blob.etag().ok().flatten()),
            },
            |remote_modified, remote_blob| PullFile {
                remote_blob: remote_blob
                    .expect("remote blob should be Some")
                    .into_raw_body(),
                remote_modified,
                local_path: local_path.clone(),
            },
            |_| {},
        );

        // Print actions to the user
        info!("Action:");
        match action {
            SyncType::Push(_) => info!("<- PUSH"),
            SyncType::Pull(_) => info!("-> PULL"),
            SyncType::Skip { reason, .. } => info!("   SKIP ({reason})"),
        }

        if matches!(action, SyncType::Skip { .. }) {
            // Nothing to do
            exit(0);
        } else if self.sync.check_only {
            // Do nothing, but report that we're out of sync
            exit(1);
        }

        // Ask for confirmation
        if !self.sync.no_confirm {
            confirm()?;
        }

        // Execute the action
        action.execute().await?;

        Ok(())
    }
}

struct PullFile {
    remote_blob: ResponseBody,
    remote_modified: OffsetDateTime,
    local_path: PathBuf,
}

impl SyncAction for PullFile {
    async fn execute(mut self) -> anyhow::Result<()> {
        // Save the file to disk
        let mut file = File::create(self.local_path)?;
        while let Some(chunk) = self.remote_blob.try_next().await? {
            file.write_all(&chunk)?;
        }
        file.set_modified(self.remote_modified.into())?;

        Ok(())
    }
}

struct PushFile {
    client: BlobClient,
    local_path: PathBuf,
    local_modified: OffsetDateTime,
    remote_etag: Option<String>,
}

impl SyncAction for PushFile {
    async fn execute(self) -> anyhow::Result<()> {
        let local_file = AsyncFile::open(self.local_path).await?;
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
