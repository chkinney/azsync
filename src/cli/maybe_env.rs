use std::{
    borrow::Cow,
    env::var,
    error::Error,
    ffi::OsStr,
    fmt::{Display, Formatter},
    marker::PhantomData,
    str::FromStr,
};

use anyhow::Context;
use clap::{
    Arg, Command,
    builder::{NonEmptyStringValueParser, TypedValueParser, ValueParserFactory},
};
use url::Url;

use crate::dotenv::DotenvFile;

/// A value that may be loaded from the environment or a dotenv file.
///
/// To load from an environment, the value must be a URL in the format
/// `env:VAR_NAME`.
///
/// `T` must be [`FromStr`] for this to be used as a [`clap`] value type.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum MaybeEnv<T> {
    /// Value refers to an environment variable.
    EnvVar(String),

    /// Value was provided directly.
    Value(T),
}

impl<T> MaybeEnv<T>
where
    T: ToOwned<Owned = T> + FromStr<Err: Error + Send + Sync + 'static>,
{
    /// Resolves this value.
    ///
    /// If the value comes from the environment, that value is looked up from
    /// either the provided dotenv file or the process environment (in order,
    /// first match).
    pub fn resolve(&self, env_file: Option<&DotenvFile>) -> anyhow::Result<Cow<'_, T>> {
        match self {
            MaybeEnv::EnvVar(var_name) => {
                // Get value of variable
                let value = env_file
                    .and_then(|file| file.parameters.get(var_name))
                    .map(Cow::Borrowed)
                    .or_else(|| var(var_name).ok().map(Cow::Owned))
                    .with_context(|| format!("'{var_name}' not found in environment"))?;

                // Parse variable
                let value = value
                    .parse()
                    .with_context(|| format!("failed to parse {var_name}"))?;

                Ok(Cow::Owned(value))
            }
            MaybeEnv::Value(value) => Ok(Cow::Borrowed(value)),
        }
    }
}

impl<T> Display for MaybeEnv<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MaybeEnv::EnvVar(var_name) => write!(f, "env:{var_name}"),
            MaybeEnv::Value(value) => value.fmt(f),
        }
    }
}

impl<T> Default for MaybeEnv<T>
where
    T: Default,
{
    fn default() -> Self {
        MaybeEnv::Value(T::default())
    }
}

impl<T> ValueParserFactory for MaybeEnv<T> {
    type Parser = MaybeEnvParser<T>;

    fn value_parser() -> Self::Parser {
        MaybeEnvParser(PhantomData)
    }
}

/// Value parser for [`MaybeEnv`]s.
#[derive(Clone, Debug)]
pub struct MaybeEnvParser<T>(PhantomData<fn() -> T>);

impl<T> TypedValueParser for MaybeEnvParser<T>
where
    T: Clone + Send + Sync + 'static,
    T: FromStr<Err: Into<Box<dyn Error + Send + Sync>>>,
{
    type Value = MaybeEnv<T>;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        // Parse as a string
        let inner = NonEmptyStringValueParser::default();
        let value2 = inner.parse_ref(cmd, arg, value)?;

        // Parse the string as `env:VAR_NAME` if possible
        if let Ok(value) = Url::from_str(&value2)
            && value.scheme() == "env"
            && value.cannot_be_a_base()
            && !value.path().is_empty()
        {
            let var_name = value.path();
            return Ok(MaybeEnv::EnvVar(var_name.to_string()));
        }

        TypedValueParser::parse_ref(&T::from_str, cmd, arg, value).map(MaybeEnv::Value)
    }
}
