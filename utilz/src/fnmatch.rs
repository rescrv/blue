//! Perform fnmatch on text, returning true if it matches pattern.

fn fnmatch(pattern: &str, text: &str) -> bool {
    let mut pat = pattern.chars();
    let mut txt = text.chars();
    'processing: loop {
        match (pat.next(), txt.next()) {
            (Some(p), Some(t)) => {
                if p == '*' {
                    for (idx, _) in text.char_indices() {
                        if fnmatch(&pattern[1..], &text[idx..]) {
                            return true;
                        }
                    }
                    continue 'processing;
                } else if p == t {
                    continue 'processing;
                } else {
                    return false;
                }
            }
            (Some(p), None) => p == '*' && pat.all(|c| c == '*'),
            (None, Some(_)) => {
                return false;
            }
            (None, None) => {
                return true;
            }
        };
    }
}

////////////////////////////////////////////// Pattern /////////////////////////////////////////////

/// A [Pattern] captures the pattern for globbing.  Call `fnmatch` to check if a text string
/// matches.
pub struct Pattern {
    pattern: String,
}

impl Pattern {
    /// True if and only if the pattern is valid.
    ///
    /// Currently just checks that the pattern is less than 64 characters long.  An arbitrary
    /// restriction because recursion.
    pub fn is_valid(pattern: &str) -> bool {
        pattern.len() < 64
    }

    /// Create a pattern from String, or panic if the pattern is not valid.
    ///
    /// # Panics
    ///
    /// Panics on an invalid pattern.
    pub fn must(pattern: String) -> Self {
        if let Some(pat) = Self::new(pattern) {
            pat
        } else {
            panic!("invalid pattern in a must declaration");
        }
    }

    /// Create a new pattern if it is valid; else None.
    pub fn new(pattern: String) -> Option<Self> {
        if Pattern::is_valid(&pattern) {
            Some(Self { pattern })
        } else {
            None
        }
    }

    /// Run fnmatch of the pattern against `text`.
    pub fn fnmatch(&self, text: &str) -> bool {
        fnmatch(&self.pattern, text)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        assert!(Pattern::must("".to_owned()).fnmatch(""));
        assert!(!Pattern::must("".to_owned()).fnmatch("abc"));
        assert!(Pattern::must("abc".to_owned()).fnmatch("abc"));
        assert!(Pattern::must("a*c".to_owned()).fnmatch("abc"));
        assert!(Pattern::must("a*c".to_owned()).fnmatch("aabbcc"));
        assert!(Pattern::must("*bc".to_owned()).fnmatch("abc"));
        assert!(Pattern::must("*bc".to_owned()).fnmatch("bc"));
        assert!(Pattern::must("ab*".to_owned()).fnmatch("abc"));
        assert!(Pattern::must("ab*".to_owned()).fnmatch("ab"));
    }
}
