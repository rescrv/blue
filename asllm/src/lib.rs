use std::io::{self, Write};
use std::time::Duration;

/// Default delay applied before each token is written and flushed.
pub const DEFAULT_TOKEN_DELAY: Duration = Duration::from_nanos(1_000_000_000 / 42);

/// Maximum size (in characters) for a single long word or number token.
const DEFAULT_LONG_TOKEN_LIMIT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    Word,
    Number,
    Punctuation,
    Other,
}

#[derive(Debug)]
pub struct AsLlm<W: Write> {
    inner: W,
    delay: Duration,
}

impl<W: Write> AsLlm<W> {
    /// Wrap any `Write` implementor, using the default token delay of 42 tokens/s.
    pub fn new(writer: W) -> Self {
        Self::with_delay(writer, DEFAULT_TOKEN_DELAY)
    }

    /// Wrap any `Write` implementor with a custom token delay.
    ///
    /// A zero delay is useful for tests.
    pub fn with_delay(writer: W, delay: Duration) -> Self {
        Self {
            inner: writer,
            delay,
        }
    }

    /// Consume the wrapper and return the nested writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for AsLlm<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = std::str::from_utf8(buf).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "AsLlm writer expects UTF-8 text input",
            )
        })?;

        let bytes_len = buf.len();
        for token in tokenize(text) {
            std::thread::sleep(self.delay);
            self.inner.write_all(token.as_bytes())?;
            self.inner.flush()?;
        }

        Ok(bytes_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn char_class(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphabetic() {
        CharClass::Word
    } else if c.is_numeric() {
        CharClass::Number
    } else if c.is_ascii_punctuation() {
        CharClass::Punctuation
    } else {
        CharClass::Other
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_class = None;
    let mut current_chars = 0usize;

    for ch in text.chars() {
        let class = char_class(ch);
        let should_split = current_class
            .map(|prev| {
                prev != class
                    || ((prev == CharClass::Word || prev == CharClass::Number)
                        && current_chars >= DEFAULT_LONG_TOKEN_LIMIT)
            })
            .unwrap_or(false);

        if should_split {
            tokens.push(std::mem::take(&mut current));
            current_chars = 0;
            current_class = Some(class);
        } else if current_class.is_none() {
            current_class = Some(class);
        }

        current.push(ch);
        current_chars += 1;
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FlushingWriter {
        buf: Vec<u8>,
        pub flushes: usize,
    }

    impl Write for FlushingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buf.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    #[test]
    fn default_token_delay_is_42_tokens_per_second() {
        assert_eq!(
            Duration::from_nanos(1_000_000_000 / 42),
            DEFAULT_TOKEN_DELAY
        );
    }

    #[test]
    fn tokenizes_and_flushes_per_token() -> io::Result<()> {
        let mut writer = AsLlm::with_delay(FlushingWriter::default(), Duration::ZERO);

        write!(writer, "Hi there!!! ThisIsALongWord")?;

        let inner = writer.into_inner();
        assert_eq!(inner.buf, b"Hi there!!! ThisIsALongWord");
        assert!(inner.flushes > 1);
        Ok(())
    }

    #[test]
    fn tokenization_breaks_by_class_and_long_words() {
        let tokens = tokenize("hello,world!!! 12AB34 12345678901");
        assert_eq!(
            tokens,
            vec![
                "hello",
                ",",
                "world",
                "!!!",
                " ",
                "12",
                "AB",
                "34",
                " ",
                "12345678901",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
    }
}
