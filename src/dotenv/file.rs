use std::{
    borrow::Cow,
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fmt::Write,
    fs::File,
    io::{ErrorKind, Read},
    ops::Range,
    path::Path,
    time::SystemTime,
};

/// A loaded dotenv file.
#[derive(Clone, Debug, Default)]
pub struct DotenvFile {
    /// The original source contents of the file.
    pub(super) source: String,

    /// The variables defined in the file.
    pub parameters: HashMap<String, String>,

    /// The source locations for the values of variables defined in this file.
    pub(super) value_spans: HashMap<String, Range<usize>>,

    /// Parameters that are expanded after being defined.
    ///
    /// These parameters cannot be replaced in-place because doing so would
    /// affect other parameters defined later in the file.
    pub(super) referenced: HashSet<String>,

    /// The last modified date, if available.
    pub last_modified: Option<SystemTime>,
}

impl DotenvFile {
    /// Load this dotenv file from the given file path (if it exists)
    pub fn from_path_exists(path: &Path) -> anyhow::Result<Option<Self>> {
        // Open file
        let file = File::open(path);
        if let Err(error) = &file
            && error.kind() == ErrorKind::NotFound
        {
            // File not found
            return Ok(None);
        }

        // Read the file
        let mut file = file?;
        let mut source = String::new();
        file.read_to_string(&mut source)?;

        // Parse it
        let dotenv = Self::parse(source)?;

        // Attach last modified time if available
        Ok(Some(Self {
            last_modified: file
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok(),
            ..dotenv
        }))
    }

    /// Replaces the parameter values in this file, returning the modified
    /// contents.
    ///
    /// New parameters are appended to the end of the file. Existing parameters
    /// are replaced in-place. Any parameters not provided to this function that
    /// exist in the file will be left as-is.
    pub fn replace(&self, replacements: HashMap<String, String>) -> String {
        // Split up replacements and additions
        let mut replaced = Vec::with_capacity(replacements.len());
        let mut added = Vec::with_capacity(replacements.len());
        for (name, new_value) in replacements {
            if !self.referenced.contains(&name)
                && let Some(span) = self.value_spans.get(&name)
            {
                // Replace the value in-place
                replaced.push((span.clone(), new_value));
            } else {
                // Add the value to the end of the file
                added.push((name, new_value));
            }
        }

        // Replace values in reverse order to avoid shifting later indexes
        replaced.sort_by_key(|(span, _)| Reverse(span.end));
        let mut content = self.source.clone();
        for (span, value) in replaced {
            let escaped = escape(&value);
            content.replace_range(span, &escaped);
        }

        // Append new values to the end
        if !added.is_empty() {
            // Add newline to the end if needed
            if content.chars().last().is_some_and(|c| c != '\n') {
                content.push('\n');
            }

            for (name, value) in added {
                let value = escape(&value);
                let _ = writeln!(content, "{name}={value}");
            }
        }

        content
    }
}

/// Escapes a value so that it's valid in a dotenv file.
fn escape(value: &str) -> Cow<'_, str> {
    const ESCAPED: &[char] = &['\\', '$', '"', '\''];
    if value.contains(ESCAPED) || value != value.trim() {
        let value = ESCAPED.iter().fold(value.to_owned(), |value, &c| {
            value.replace(c, &format!("\\{c}"))
        });
        format!("\"{value}\"").into()
    } else {
        value.into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const SIMPLE: &str = include_str!("tests/simple.env");
    const SIMPLE_REPLACED: &str = include_str!("tests/simple.replaced.env");

    const EXPANSION: &str = include_str!("tests/expansion.env");
    const EXPANSION_REPLACED: &str = include_str!("tests/expansion.replaced.env");

    #[test]
    fn replace_simple() {
        let dotenv = DotenvFile::parse(SIMPLE).unwrap();
        let replacements = [("A", "456"), ("C", "seven eighty nine"), ("D", "new value")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let replaced = dotenv.replace(replacements);

        assert_eq!(SIMPLE_REPLACED, replaced);
    }

    #[test]
    fn replace_expansion() {
        let dotenv = DotenvFile::parse(EXPANSION).unwrap();
        let replacements = [
            ("A", "$789"),
            ("D", "d${e}e"),
            ("E", "\"eee\""),
            ("I", "'aii'"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        let replaced = dotenv.replace(replacements);

        assert_eq!(EXPANSION_REPLACED, replaced);
    }

    #[test]
    fn replace_empty() {
        let dotenv = DotenvFile::default();
        let replacements = [("A", "aaa"), ("B", "bbb"), ("C", "ccc")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let expected: HashSet<_> = ["A=aaa", "B=bbb", "C=ccc"].into_iter().collect();

        let replaced = dotenv.replace(replacements);

        let lines: HashSet<_> = replaced.lines().collect();
        assert_eq!(lines, expected);
    }
}
