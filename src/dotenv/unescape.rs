use std::mem::take;

/// Unescapes a character sequence using bash escaping rules.
pub fn unescape<Chars>(chars: Chars) -> Unescape<Chars::IntoIter>
where
    Chars: IntoIterator<Item = char>,
{
    Unescape {
        inner: chars.into_iter(),
        escaped: false,
    }
}

/// Bash-style unescaping.
#[derive(Clone, Debug)]
pub struct Unescape<Chars> {
    inner: Chars,
    escaped: bool,
}

impl<Chars> Iterator for Unescape<Chars>
where
    Chars: Iterator<Item = char>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let escaped = take(&mut self.escaped);
            let c = self.inner.next()?;

            // Check if we need to escape the next char
            if !escaped && c == '\\' {
                self.escaped = true;
                continue;
            }

            break Some(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case("abc def$!#$^!*$%!@ " => "abc def$!#$^!*$%!@ "; "no escapes")]
    #[test_case(r#"a\b\c \"de\ f\""# => r#"abc "de f""#; "simple")]
    #[test_case(r"abc\d \" => r"abcd "; "drop trailing slash")] // should never happen
    #[test_case("" => ""; "empty")]
    fn unescapes_correctly(s: &str) -> String {
        unescape(s.chars()).collect()
    }
}
