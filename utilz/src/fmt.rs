/// Escape the byte sequence using standard ASCII escape notation.
pub fn escape_str(bytes: &[u8]) -> String {
    String::from_utf8(
        bytes
            .iter()
            .flat_map(|b| std::ascii::escape_default(*b))
            .collect::<Vec<u8>>(),
    )
    .unwrap()
}

/// A helper struct returned by [nqs].
pub struct NoQuoteString<'a>(&'a str);

impl<'a> std::fmt::Debug for NoQuoteString<'a> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(fmt, "{}", self.0)
    }
}

/// Wrap the provided `&str` in a way that formatting will not quote it.
///
/// Useful for making pretty debug/display calls when used with `debug_struct`.
pub fn nqs(s: &str) -> NoQuoteString<'_> {
    NoQuoteString(s)
}
