pub fn escape_str(bytes: &[u8]) -> String {
    String::from_utf8(
        bytes
            .iter()
            .flat_map(|b| std::ascii::escape_default(*b))
            .collect::<Vec<u8>>(),
    )
    .unwrap()
}

pub struct NoQuoteString<'a>(&'a str);

impl<'a> std::fmt::Debug for NoQuoteString<'a> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(fmt, "{}", self.0)
    }
}

pub fn nqs(s: &str) -> NoQuoteString<'_> {
    NoQuoteString(s)
}
