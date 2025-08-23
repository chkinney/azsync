use std::{borrow::Cow, env::var};

use anyhow::{Context, bail};
use url::Url;

use crate::{cli::KeyVaultOptions, dotenv::DotenvFile};

impl KeyVaultOptions {
    pub fn resolve_url(&self, dotenv: Option<&DotenvFile>) -> anyhow::Result<Cow<'_, Url>> {
        match self.key_vault_url.scheme() {
            // Standard HTTP/S URL
            "http" | "https" => Ok(Cow::Borrowed(&self.key_vault_url)),
            // Environment variable
            "env" => {
                // Get value from environment
                let var_name = self
                    .key_vault_url
                    .host_str()
                    .context("Missing Key Vault URL variable name (format: env://VAR_NAME)")?;
                let url = dotenv
                    .and_then(|dotenv| dotenv.parameters.get(var_name))
                    .cloned();
                let url = url.or_else(|| var(var_name).ok());
                let Some(url) = url else {
                    bail!("'{}' not found in environment", self.key_vault_url.path());
                };

                // Parse URL
                let url = Url::parse(&url).context("Failed to parse Key Vault URL")?;
                Ok(Cow::Owned(url))
            }
            _ => bail!("Unsupported scheme: '{}'", self.key_vault_url.scheme()),
        }
    }
}
