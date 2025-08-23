use std::{
    collections::{HashMap, VecDeque},
    iter::Peekable,
};

/// Performs bash-style parameter expansion on a string.
pub fn expand<Chars>(
    chars: Chars,
    parameters: &HashMap<String, String>,
) -> Expand<'_, Chars::IntoIter>
where
    Chars: IntoIterator<Item = char>,
{
    Expand {
        inner: chars.into_iter().peekable(),
        parameters,
        state: State::default(),
        on_expand: None,
    }
}

/// Bash-style parameter expansion.
pub struct Expand<'i, Chars>
where
    Chars: Iterator<Item = char>,
{
    inner: Peekable<Chars>,
    parameters: &'i HashMap<String, String>,
    state: State,
    on_expand: Option<Box<dyn for<'s> FnMut(&'s str) + 'i>>,
}

impl<'i, Chars> Expand<'i, Chars>
where
    Chars: Iterator<Item = char>,
{
    /// Call the provided function whenever a name is expanded.
    pub fn on_expand(&mut self, f: impl for<'s> FnMut(&'s str) + 'i) {
        self.on_expand = Some(Box::new(f));
    }
}

impl<Chars> Iterator for Expand<'_, Chars>
where
    Chars: Iterator<Item = char>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let c = match &mut self.state {
                State::NotExpanding => {
                    // Get the next char and check if we need to start an expansion
                    let c = self.inner.next()?;
                    if c == '$' {
                        self.state = State::StartExpansion;
                        continue;
                    } else if c == '\\' {
                        self.state = State::Escape;
                        continue;
                    }

                    c
                }
                State::Escape => {
                    // Keep the backslashes so they can be removed by the caller
                    let mut value = VecDeque::with_capacity(2);
                    value.push_back('\\');
                    if let Some(c) = self.inner.next() {
                        value.push_back(c);
                    }

                    self.state = State::Buffered { value };
                    continue;
                }
                State::Buffered { value } => {
                    // Take the next char from the buffer
                    if let Some(c) = value.pop_front() {
                        c
                    } else {
                        self.state = State::NotExpanding;
                        continue;
                    }
                }
                State::StartExpansion => {
                    // Braced expansion
                    if self.inner.next_if(|&c| c == '{').is_some() {
                        self.state = State::BracedExpansion {
                            name: String::new(),
                            invalid: false,
                        };
                        continue;
                    }

                    // Unbraced expansion
                    if let Some(c) = self.inner.next_if(is_name_start) {
                        let mut name = String::new();
                        name.push(c);
                        self.state = State::UnbracedExpansion { name };
                        continue;
                    }

                    // Lone '$' - not expanding
                    self.state = State::NotExpanding;
                    '$'
                }
                State::BracedExpansion { name, invalid } => {
                    // Variable name
                    if let Some(c) = self.inner.next_if(is_name_start) {
                        name.push(c);
                        continue;
                    }
                    if let Some(c) = self.inner.next_if(char::is_ascii_digit) {
                        name.push(c);
                        continue;
                    }

                    // Done expanding
                    if self.inner.next_if(|&c| c == '}').is_some() {
                        // Get the value of this parameter
                        if !*invalid && let Some(value) = self.parameters.get(name) {
                            // Call on_expand if needed
                            if let Some(on_expand) = &mut self.on_expand {
                                on_expand(name);
                            }

                            self.state = State::Buffered {
                                value: value.chars().collect(),
                            };
                            continue;
                        }

                        // Parameter not defined (or valid) - skip it
                        // Note: we can't return errors for invalid parameter names like bash can
                        self.state = State::NotExpanding;
                        continue;
                    }

                    // Invalid character
                    if self.inner.next().is_some() {
                        *invalid = true;
                        continue;
                    }

                    // End of input - return everything we matched
                    // NOTE: using .len() may overestimate the number of chars,
                    // but it's okay to slightly overallocate
                    let mut value = VecDeque::with_capacity(name.len() + 2);
                    value.push_back('$');
                    value.push_back('{');
                    value.extend(name.chars());
                    self.state = State::Buffered { value };
                    continue;
                }
                State::UnbracedExpansion { name } => {
                    // Check if we're still reading the parameter's name
                    if let Some(c) = self
                        .inner
                        .next_if(|c| is_name_start(c) || c.is_ascii_digit())
                    {
                        name.push(c);
                        continue;
                    }

                    // Get the value of this parameter
                    if let Some(value) = self.parameters.get(name) {
                        // Call on_expand if needed
                        if let Some(on_expand) = &mut self.on_expand {
                            on_expand(name);
                        }

                        self.state = State::Buffered {
                            value: value.chars().collect(),
                        };
                        continue;
                    }

                    // Parameter not defined - skip it
                    self.state = State::NotExpanding;
                    continue;
                }
            };

            break Some(c);
        }
    }
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "next_if passes by ref")]
fn is_name_start(c: &char) -> bool {
    *c == '_' || c.is_alphabetic()
}

#[derive(Clone, Debug, Default)]
enum State {
    /// Normal characters outside of an expansion.
    #[default]
    NotExpanding,

    /// Escaping the next character (but not expanding).
    Escape,

    /// A string is buffered and needs to be returned.
    Buffered { value: VecDeque<char> },

    /// Started expanding due to `'$'`.
    StartExpansion,

    /// Expanding a braced parameter like `"${ etc }"`.
    BracedExpansion { name: String, invalid: bool },

    /// Expanding an unbraced parameter like `"$etc"`.
    UnbracedExpansion { name: String },
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case("abc def" => "abc def"; "simple")]
    #[test_case(r"ghi \$jkl" => r"ghi \$jkl"; "escaped start")]
    fn no_expansion(s: &str) -> String {
        expand(s.chars(), &HashMap::new()).collect()
    }

    #[test_case("$abc $abc" => "a a"; "simple")]
    #[test_case("$def2 $3abc" => "b $3abc"; "digits")]
    #[test_case("$_ghi$_ghi" => "cc"; "underscores")]
    #[test_case("$_j3k_l3_ $_aaa" => "d "; "complex")]
    #[test_case(r"\$abc \\$def2" => r"\$abc \\b"; "escaped")]
    fn unbraced_expansion(s: &str) -> String {
        let parameters: HashMap<_, _> = [
            ("abc", "a"),
            ("def2", "b"),
            ("_ghi", "c"),
            ("_j3k_l3_", "d"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        expand(s.chars(), &parameters).collect()
    }

    #[test_case("${abc} ${abc}" => "a a"; "simple")]
    #[test_case("${def2} ${3abc}" => "b "; "digits")]
    #[test_case("${_ghi}${_ghi}" => "cc"; "underscores")]
    #[test_case("${_j3k_l3_} ${_aaa}" => "d "; "complex")]
    #[test_case(r"\${abc} \\${def2}" => r"\${abc} \\b"; "escaped")]
    #[test_case(r"}}{abc}{{abc${abc{}}$}" => r"}}{abc}{{abc}$}"; "extra braces")]
    fn braced_expansion(s: &str) -> String {
        let parameters: HashMap<_, _> = [
            ("abc", "a"),
            ("def2", "b"),
            ("_ghi", "c"),
            ("_j3k_l3_", "d"),
            ("_", "e"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        expand(s.chars(), &parameters).collect()
    }
}
