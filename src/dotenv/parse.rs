use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    str::FromStr,
};

use anyhow::{Context, bail, ensure};
use pest::{Parser, Span, iterators::Pair};
use pest_derive::Parser;

use crate::dotenv::{DotenvFile, expand::expand, unescape::unescape};

#[derive(Parser)]
#[grammar = "grammars/dotenv.pest"]
struct DotenvParser;

impl DotenvFile {
    /// Parses a string as a dotenv file.
    pub fn parse(source: impl ToString) -> anyhow::Result<Self> {
        // Parse the contents
        let source = source.to_string();
        let pairs = DotenvParser::parse(Rule::dotenv, &source)?;
        let mut parameters = HashMap::new();
        let mut value_spans = HashMap::new();
        let mut referenced = HashSet::new(); // names that are expanded later in the file
        for pair in pairs {
            match pair.as_rule() {
                Rule::var_definition => {
                    // Parse a variable definition
                    let (name, value) = var_definition(pair, &parameters, &mut referenced)?;

                    // Overwrite previous definition if needed
                    referenced.remove(&name); // New definition (even if self-referencing)
                    parameters.insert(name.clone(), value.value);
                    value_spans.insert(name, value.span);
                }
                Rule::EOI => {
                    // Done
                }
                rule => bail!("Unexpected rule: {rule:?}"),
            }
        }

        Ok(DotenvFile {
            source,
            parameters,
            value_spans,
            referenced,
            last_modified: None,
        })
    }
}

impl FromStr for DotenvFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn var_definition(
    pair: Pair<'_, Rule>,
    parameters: &HashMap<String, String>,
    referenced: &mut HashSet<String>,
) -> anyhow::Result<(String, Spanned<String>)> {
    let mut name = None;
    let mut value = None;
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::var_name => {
                ensure!(
                    name.is_none(),
                    "Variable name defined multiple times (this is a bug)"
                );
                name = Some(pair.as_str().to_owned());
            }
            Rule::var_value_uq => {
                ensure!(
                    value.is_none(),
                    "Variable value defined multiple times (this is a bug)"
                );
                let mut processed = expand(pair.as_str().chars(), parameters);
                processed.on_expand(|name| {
                    referenced.insert(name.to_string());
                });
                let processed = unescape(processed);
                value = Some(Spanned::new(processed.collect(), pair.as_span()));
            }
            Rule::var_value_sq => {
                ensure!(
                    value.is_none(),
                    "Variable value defined multiple times (this is a bug)"
                );
                let processed = unquote(pair.as_str(), '\'')
                    .context("Single-quoted value missing one or more quotes (this is a bug)")?;
                value = Some(Spanned::new(processed.to_owned(), pair.as_span()));
            }
            Rule::var_value_dq => {
                ensure!(
                    value.is_none(),
                    "Variable value defined multiple times (this is a bug)"
                );
                let processed = unquote(pair.as_str(), '"')
                    .context("Double-quoted value missing one or more quotes (this is a bug)")?;
                let mut processed = expand(processed.chars(), parameters);
                processed.on_expand(|name| {
                    referenced.insert(name.to_string());
                });
                let processed = unescape(processed);
                value = Some(Spanned::new(processed.collect(), pair.as_span()));
            }
            rule => bail!("Unexpected rule: {rule:?} (this is a bug)"),
        }
    }

    let name = name.context("Missing variable name (this is a bug)")?;
    let value = value.context("Missing variable value (this is a bug)")?;
    Ok((name, value))
}

/// Removes a leading and trailing quote character from the string.
fn unquote(s: &str, quote: char) -> Option<&str> {
    s.strip_prefix(quote)?.strip_suffix(quote)
}

/// A value with an associated source location.
#[derive(Clone, Debug)]
struct Spanned<T> {
    /// The value.
    pub value: T,

    /// The source location of the value.
    pub span: Range<usize>,
}

impl<T> Spanned<T> {
    /// Create a new spanned value from a value and span.
    pub fn new(value: T, span: Span<'_>) -> Self {
        let (start, end) = span.split();
        Self {
            value,
            span: start.pos()..end.pos(),
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    // Load test dotenv files
    const SIMPLE: &str = include_str!("tests/simple.env");
    const EXPORT: &str = include_str!("tests/export.env");
    const EXPANSION: &str = include_str!("tests/expansion.env");
    const COMMENTS: &str = include_str!("tests/comments.env");

    const SIMPLE_VALUES: &[(&str, &str)] =
        &[("A", "123"), ("B", "four five six"), ("C", "seven 8 nine")];
    const EXPORT_VALUES: &[(&str, &str)] =
        &[("A", "123"), ("B", "four five six"), ("C", "seven 8 nine")];
    const COMMENTS_VALUES: &[(&str, &str)] =
        &[("A", "123#456"), ("B", "123#456"), ("C", "123#456")];
    const EXPANSION_VALUES: &[(&str, &str)] = &[
        ("A", "456"),
        ("B", "123 456"),
        ("C", "$A 456"),
        ("D", "123 456"),
        ("E", "123123"),
        ("F", "${A}${A}"),
        ("G", "123123"),
        ("H", "aa456456aa"),
        ("I", "aa$A${A}aa"),
        ("J", "aa456456aa"),
    ];

    const SIMPLE_SPANS: &[(&str, Range<usize>)] = &[("A", 2..5), ("B", 8..23), ("C", 26..40)];
    const EXPORT_SPANS: &[(&str, Range<usize>)] = &[("A", 4..7), ("B", 17..32), ("C", 49..63)];
    const COMMENTS_SPANS: &[(&str, Range<usize>)] = &[("A", 19..26), ("B", 34..43), ("C", 50..59)];
    const EXPANSION_SPANS: &[(&str, Range<usize>)] = &[
        ("A", 103..106),
        ("B", 9..15),
        ("C", 18..26),
        ("D", 29..37),
        ("E", 41..49),
        ("F", 60..70),
        ("G", 81..91),
        ("H", 110..120),
        ("I", 123..135),
        ("J", 138..150),
    ];

    #[test_case(SIMPLE, SIMPLE_VALUES; "simple")]
    #[test_case(EXPORT, EXPORT_VALUES; "export")]
    #[test_case(COMMENTS, COMMENTS_VALUES; "comments")]
    #[test_case(EXPANSION, EXPANSION_VALUES; "expansion")]
    fn values(s: &str, expected: &[(&str, &str)]) {
        let mut dotenv = DotenvFile::parse(s).unwrap();

        // Check that all the defined parameters match
        for &(k, expected) in expected {
            let actual = dotenv
                .parameters
                .remove(k)
                .unwrap_or_else(|| panic!("missing {k:?}"));
            assert_eq!(expected, actual, "for parameter {k:?}");
        }

        assert!(dotenv.parameters.is_empty());
    }

    #[test_case(SIMPLE, SIMPLE_SPANS; "simple")]
    #[test_case(EXPORT, EXPORT_SPANS; "export")]
    #[test_case(COMMENTS, COMMENTS_SPANS; "comments")]
    #[test_case(EXPANSION, EXPANSION_SPANS; "expansion")]
    fn spans(s: &str, expected: &[(&str, Range<usize>)]) {
        let s = s.replace("\r\n", "\n");
        let mut dotenv = DotenvFile::parse(s).unwrap();

        // Check that all the spans match
        for (k, expected) in expected {
            let actual = dotenv
                .value_spans
                .remove(*k)
                .unwrap_or_else(|| panic!("missing {k:?}"));
            assert_eq!(expected, &actual, "for parameter {k:?}");
        }

        assert!(dotenv.value_spans.is_empty());
    }

    #[test_case(""; "empty file")]
    #[test_case("\n"; "single newline")]
    #[test_case("# foo\n# bar"; "only comments")]
    #[test_case("# foo\n# bar\n"; "only comments and newline")]
    fn empty(s: &str) {
        let dotenv = DotenvFile::parse(s).unwrap();
        assert!(dotenv.parameters.is_empty());
        assert!(dotenv.value_spans.is_empty());
    }
}
