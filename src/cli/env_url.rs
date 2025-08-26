use std::{
    borrow::Cow,
    env::var,
    fmt::{Display, Formatter},
    str::FromStr,
};

use anyhow::{Context, bail};
use url::Url;

use crate::dotenv::DotenvFile;

/// A [`Url`] that supports `env` schemes.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct EnvUrl(pub Url);

impl EnvUrl {
    /// Gets the URL as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Resolves this url.
    ///
    /// If the scheme is `env`, that value is looked up from either the provided
    /// dotenv file, or the process environment (in order, first match).
    pub fn resolve(&self, env_file: Option<&DotenvFile>) -> anyhow::Result<Cow<'_, Self>> {
        match self.0.scheme() {
            // Standard HTTP/S URL
            "http" | "https" => Ok(Cow::Borrowed(self)),
            // Environment variable
            "env" => {
                // Get value from environment
                let var_name = self
                    .0
                    .host_str()
                    .context("Missing Key Vault URL variable name (format: env://VAR_NAME)")?;
                let url = env_file
                    .and_then(|env_file| env_file.parameters.get(var_name))
                    .cloned();
                let url = url.or_else(|| var(var_name).ok());
                let Some(url) = url else {
                    bail!("'{}' not found in environment", self.0.path());
                };

                // Parse URL
                let url = Url::parse(&url).context("Failed to parse Key Vault URL")?;
                Ok(Cow::Owned(EnvUrl(url)))
            }
            _ => bail!("Unsupported scheme: '{}'", self.0.scheme()),
        }
    }
}

impl FromStr for EnvUrl {
    type Err = <Url as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::from_str(s).map(EnvUrl)
    }
}

impl Display for EnvUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
