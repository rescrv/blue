use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::iter::{Enumerate, Peekable};
use std::str::Chars;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// An error occurred during expansion or quoting operations.
#[derive(Clone, Debug)]
pub enum Error {
    /// The provided expansion leaves open a single quote.
    OpenSingleQuotes,
    /// The provided expansion leaves open a double quote.
    OpenDoubleQuotes,
    /// There's an unmatched brace.  Right now braces cannot appear in expansions.
    TrailingRightBrace,
    /// There's an invalid variable definition.
    InvalidVariable,
    /// The user-requested ${FOO:?ERROR MESSAGE} form will return `"ERROR MESSAGE".to_string()` via
    /// this variant.
    Requested(String),
}

////////////////////////////////////////////// quoting /////////////////////////////////////////////

// I consulted the FreeBSD man pages for guidance.
//
// On the subject of double quotes, it had this to say:
//
// Double Quotes
//
//      Enclosing characters within double quotes preserves the literal meaning of  all characters
//      except dollar sign (`$'), backquote (``'), and backslash (`\').  The    backslash inside
//      double  quotes is historically weird.  It remains literal unless  it  precedes the
//      following characters, which it serves to quote:
//
//      $      `     "     \     \n
//
pub fn quote_string(s: &str) -> String {
    let has_whitespace = !s.is_empty() && s.chars().any(|c| c.is_whitespace());
    let has_single_quote = !s.is_empty() && s.chars().any(|c| c == '\'');
    let has_double_quote = !s.is_empty() && s.chars().any(|c| c == '"');
    match (has_whitespace, has_single_quote, has_double_quote) {
        (false, false, false) => s.to_string(),
        (_, _, false) => double_quote_string(s),
        // SAFETY(rescrv):  Single quote the string never fails because there are no single quotes.
        (_, false, _) => single_quote_string(s).unwrap(),
        (_, true, true) => awkward_quote_string(s),
    }
}

/// Awkwardly quote the string.
///
/// This will put quotes around each whitespace or quotation mark character.
pub fn awkward_quote_string(s: &str) -> String {
    let mut output = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            c if c.is_whitespace() => {
                output.push('\'');
                output.push(c);
                output.push('\'');
            }
            '\'' => {
                output.push_str("\"'\"");
            }
            '"' => {
                output.push_str("'\"'");
            }
            _ => {
                output.push(c);
            }
        }
    }
    output
}

/// Put the string in double quotes, escaping '$', '`', '"', '\\', and '\n'.
pub fn double_quote_string(s: &str) -> String {
    let mut output = String::with_capacity(s.len() + 2);
    output.push('"');
    for c in s.chars() {
        if ['$', '`', '"', '\\', '\n'].contains(&c) {
            output.push('\\');
        }
        output.push(c);
    }
    output.push('"');
    output
}

/// Single quote the provided string, if it contains no single-quote characters.
pub fn single_quote_string(s: &str) -> Option<String> {
    if !s.chars().any(|c| c == '\'') {
        Some(format!("'{}'", s))
    } else {
        None
    }
}

/// Quote the pieces in such a way that splitting the quoted string will return the original
/// pieces.
pub fn quote(pieces: Vec<String>) -> String {
    pieces
        .into_iter()
        .map(|s| quote_string(&s))
        .collect::<Vec<_>>()
        .join(" ")
}

///////////////////////////////////////////// splitting ////////////////////////////////////////////

/// Split the string in a way that respects quoting rules.
pub fn split(s: &str) -> Result<Vec<String>, Error> {
    #[derive(Clone, Copy)]
    enum State {
        Unquoted,
        Double,
        Single,
    }
    let mut state = State::Unquoted;
    let mut output = vec![];
    let mut next_word: Option<String> = Some("".to_string());
    let append_char = |next_word: &mut Option<String>, c: char| {
        if let Some(next_word) = next_word.as_mut() {
            next_word.push(c);
        } else {
            let mut s = String::new();
            s.push(c);
            *next_word = Some(s);
        }
    };
    let mut prev_was_whack = false;
    for c in s.chars() {
        match (state, c) {
            (State::Double, '$') if prev_was_whack => {
                append_char(&mut next_word, '$');
                prev_was_whack = false;
            }
            (State::Double, '`') if prev_was_whack => {
                append_char(&mut next_word, '`');
                prev_was_whack = false;
            }
            (State::Double, '"') if prev_was_whack => {
                append_char(&mut next_word, '"');
                prev_was_whack = false;
            }
            (State::Double, '\\') if prev_was_whack => {
                append_char(&mut next_word, '\\');
                prev_was_whack = false;
            }
            (State::Double, '\n') if prev_was_whack => {
                append_char(&mut next_word, '\n');
                prev_was_whack = false;
            }
            (State::Double, 'n') if prev_was_whack => {
                append_char(&mut next_word, '\n');
                prev_was_whack = false;
            }
            (State::Double, '"') => {
                state = State::Unquoted;
                prev_was_whack = false;
            }
            (State::Double, '\\') => {
                prev_was_whack = true;
            }
            (State::Double, c) if prev_was_whack => {
                append_char(&mut next_word, '\\');
                append_char(&mut next_word, c);
                prev_was_whack = false;
            }
            (State::Double, c) => {
                append_char(&mut next_word, c);
                prev_was_whack = false;
            }
            (State::Single, '\'') => {
                state = State::Unquoted;
                prev_was_whack = false;
            }
            (State::Single, c) => {
                append_char(&mut next_word, c);
                prev_was_whack = false;
            }
            (State::Unquoted, c) if c.is_whitespace() => {
                if let Some(next_word) = next_word.take() {
                    output.push(next_word);
                }
                prev_was_whack = false;
            }
            (State::Unquoted, '\'') => {
                state = State::Single;
                prev_was_whack = false;
            }
            (State::Unquoted, '"') => {
                state = State::Double;
                prev_was_whack = false;
            }
            (State::Unquoted, c) => {
                append_char(&mut next_word, c);
                prev_was_whack = false;
            }
        }
    }
    if let Some(next_word) = next_word.take() {
        output.push(next_word);
    }
    Ok(output)
}

///////////////////////////////////////// VariableProvider /////////////////////////////////////////

/// A VariableProvider provides a way to lookup the value of a variable.
///
/// It is expected that the provider do no expansion of its own.
pub trait VariableProvider {
    fn lookup(&self, ident: &str) -> Option<&str>;
}

impl VariableProvider for () {
    fn lookup(&self, _: &str) -> Option<&str> {
        None
    }
}

impl<K: Borrow<str> + Eq + Hash, V: AsRef<str>> VariableProvider for HashMap<K, V> {
    fn lookup(&self, ident: &str) -> Option<&str> {
        self.get(ident).map(|s| s.as_ref())
    }
}

///////////////////////////////////////// VariableProvider /////////////////////////////////////////

/// A VariableWitness collects the identifiers of variables in use in a string.
pub trait VariableWitness {
    fn witness(&mut self, ident: &str);
}

impl VariableWitness for () {
    fn witness(&mut self, _: &str) {}
}

///////////////////////////////////////////// Tokenizer ////////////////////////////////////////////

#[derive(Clone, Debug)]
struct Tokenize<'a> {
    symbols: Peekable<Enumerate<Chars<'a>>>,
}

impl<'a> Tokenize<'a> {
    fn new(input: &str) -> Tokenize {
        let symbols = input.chars().enumerate().peekable();
        Tokenize { symbols }
    }

    fn expect(&mut self, c: char) -> Result<(), Error> {
        if self.accept(c) {
            Ok(())
        } else {
            todo!();
        }
    }

    fn accept(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.symbols.next();
            true
        } else {
            false
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.symbols.peek().cloned().map(|x| x.1)
    }
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

#[derive(Clone, Copy)]
enum BuilderState {
    PerpetuallyQuoted,
    QuoteCount(usize),
}

impl Default for BuilderState {
    fn default() -> Self {
        Self::QuoteCount(0)
    }
}

#[derive(Default)]
struct Builder {
    state: BuilderState,
    expanded: String,
    prev: char,
}

impl Builder {
    fn from_other(other: &Builder) -> Self {
        if other.within_quotes() {
            Self {
                state: BuilderState::PerpetuallyQuoted,
                expanded: String::new(),
                prev: other.prev,
            }
        } else {
            Self {
                state: BuilderState::QuoteCount(0),
                expanded: String::new(),
                prev: other.prev,
            }
        }
    }

    fn into_string(self) -> String {
        self.expanded
    }

    fn push(&mut self, c: char) {
        if self.within_quotes() {
            self.expanded.push(c);
        } else if c.is_whitespace() && (self.expanded.is_empty() || self.prev.is_whitespace()) {
            // drop
        } else {
            self.expanded.push(c);
        }
        self.prev = c;
    }

    fn push_str(&mut self, s: &str) {
        for c in s.chars() {
            self.push(c)
        }
    }

    fn append(&mut self, other: Builder) {
        self.expanded += &other.expanded;
        self.prev = other.prev;
    }

    fn open_double_quotes(&mut self) {
        if !self.within_quotes() {
            self.expanded.push('"');
        }
        self.inc_quote_count();
    }

    fn close_double_quotes(&mut self) {
        let was_in_quotes = self.within_quotes();
        self.dec_quote_count();
        let is_in_quotes = self.within_quotes();
        if was_in_quotes != is_in_quotes {
            if self.expanded.ends_with('"') {
                self.expanded.pop();
            } else {
                self.expanded.push('"');
            }
        }
    }

    fn within_quotes(&self) -> bool {
        match self.state {
            BuilderState::PerpetuallyQuoted => true,
            BuilderState::QuoteCount(c) => c > 0,
        }
    }

    fn inc_quote_count(&mut self) {
        if let BuilderState::QuoteCount(c) = self.state {
            assert!(c < usize::MAX);
            self.state = BuilderState::QuoteCount(c + 1)
        }
    }

    fn dec_quote_count(&mut self) {
        if let BuilderState::QuoteCount(c) = self.state {
            assert!(c > 0);
            self.state = BuilderState::QuoteCount(c - 1)
        }
    }
}

/////////////////////////////////////////////// parse //////////////////////////////////////////////

fn parse_statement(
    vars: &mut dyn VariableProvider,
    witness: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    // SAFETY(rescrv):  If you add another break to this loop, update the assert in `expand`.
    while let Some(c) = tokens.peek() {
        match c {
            '\'' => {
                parse_single_quotes(vars, witness, tokens, output)?;
            }
            '"' => {
                parse_double_quotes(vars, witness, tokens, output)?;
            }
            '$' => {
                parse_variable(vars, witness, tokens, output)?;
            }
            '}' => {
                break;
            }
            c => {
                output.push(c);
                tokens.expect(c)?;
            }
        }
    }
    Ok(())
}

fn parse_single_quotes(
    _: &mut dyn VariableProvider,
    _: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    tokens.expect('\'')?;
    output.open_double_quotes();
    while let Some(c) = tokens.peek() {
        if tokens.accept('\'') {
            output.close_double_quotes();
            return Ok(());
        } else {
            tokens.accept(c);
            output.push(c);
        }
    }
    Err(Error::OpenSingleQuotes)
}

fn parse_double_quotes(
    vars: &mut dyn VariableProvider,
    witness: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    tokens.expect('"')?;
    output.open_double_quotes();
    let mut prev_was_whack = false;
    while let Some(c) = tokens.peek() {
        let mut noexpect = false;
        match c {
            '$' if prev_was_whack => {
                output.push('$');
                prev_was_whack = false;
            }
            '`' if prev_was_whack => {
                output.push('`');
                prev_was_whack = false;
            }
            '"' if prev_was_whack => {
                output.push('"');
                prev_was_whack = false;
            }
            '\\' if prev_was_whack => {
                output.push('\\');
                prev_was_whack = false;
            }
            'n' if prev_was_whack => {
                output.push('\n');
                prev_was_whack = false;
            }
            '\n' if prev_was_whack => {
                output.push(' ');
                prev_was_whack = false;
            }
            '"' => {
                output.close_double_quotes();
                tokens.expect('"')?;
                return Ok(());
            }
            '\\' => {
                prev_was_whack = true;
            }
            '$' => {
                noexpect = true;
                parse_variable(vars, witness, tokens, output)?;
            }
            c if prev_was_whack => {
                output.push('\\');
                output.push(c);
                prev_was_whack = false;
            }
            c => {
                output.push(c);
                prev_was_whack = false;
            }
        }
        if !noexpect {
            tokens.expect(c)?;
        }
    }
    Err(Error::OpenDoubleQuotes)
}

fn parse_variable(
    vars: &mut dyn VariableProvider,
    witness: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    tokens.expect('$')?;
    tokens.expect('{')?;
    let ident = parse_identifier(tokens)?;
    witness.witness(&ident);
    if tokens.accept(':') {
        let Some(action) = tokens.peek() else {
            return Err(Error::InvalidVariable);
        };
        tokens.accept(action);
        let mut expanded = Builder::from_other(output);
        parse_statement(vars, witness, tokens, &mut expanded)?;
        match action {
            '-' => {
                if let Some(val) = vars.lookup(&ident) {
                    output.push_str(val);
                } else {
                    output.append(expanded);
                }
            }
            '?' => {
                if let Some(val) = vars.lookup(&ident) {
                    output.push_str(val);
                } else {
                    return Err(Error::Requested(expanded.into_string()));
                }
            }
            '+' => {
                if vars.lookup(&ident).is_some() {
                    output.append(expanded);
                }
            }
            c => {
                todo!("complain about the token in this position: {c:?}");
            }
        }
    } else if let Some(val) = vars.lookup(&ident) {
        output.push_str(val);
    }
    tokens.expect('}')?;
    Ok(())
}

fn parse_identifier(tokens: &mut Tokenize) -> Result<String, Error> {
    let mut identifier = String::new();
    let mut first = true;
    while let Some(c) = tokens.peek() {
        match c {
            'a'..='z' | 'A'..='Z' | '_' if first => {
                identifier.push(c);
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' if !first => {
                identifier.push(c);
            }
            _ => {
                if !identifier.is_empty() {
                    return Ok(identifier);
                } else {
                    todo!("return an error for empty identifier");
                }
            }
        }
        tokens.expect(c)?;
        first = false;
    }
    Ok(identifier)
}

////////////////////////////////////////////// expand //////////////////////////////////////////////

/// Expand the input to a shell-quoted string suitable for passing to `split`.
pub fn expand(vars: &mut dyn VariableProvider, input: &str) -> Result<String, Error> {
    let mut tokens = Tokenize::new(input);
    let mut output = Builder::default();
    parse_statement(vars, &mut (), &mut tokens, &mut output)?;
    if tokens.peek().is_some() {
        // SAFETY(rescrv): We can only break out of the loop early on '}'.
        assert_eq!(Some('}'), tokens.peek());
        return Err(Error::TrailingRightBrace);
    }
    Ok(output.into_string().trim().to_string())
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_all_empty() {
        let mut env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", ""), ("s3", "")]);
        assert_eq!("", expand(&mut env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("", expand(&mut env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_space_empty_empty() {
        let mut env: HashMap<&str, &str> = HashMap::from([("s1", " "), ("s2", ""), ("s3", "")]);
        assert_eq!("", expand(&mut env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("", expand(&mut env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_empty_space_empty() {
        let mut env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", " "), ("s3", "")]);
        assert_eq!("", expand(&mut env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("\" \"", expand(&mut env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_empty_empty_space() {
        let mut env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", ""), ("s3", " ")]);
        assert_eq!("", expand(&mut env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("", expand(&mut env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn sample_expansion() {
        let mut env: HashMap<&str, &str> =
            HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
        assert_eq!(
            "foo-bar-baz",
            expand(&mut env, "${FOO}-${BAR}-${BAZ}").unwrap()
        );
    }

    proptest::proptest! {
        #[test]
        fn single_quote_roundtrip(s in "[_a-zA-Z0-9 \"]*") {
            let quoted = single_quote_string(&s).unwrap();
            let pieces = split(&quoted).unwrap();
            assert_eq!(1, pieces.len(), "s={s:?}");
            assert_eq!(s, pieces[0]);
        }

        #[test]
        fn double_quote_roundtrip(s in "[_a-zA-Z0-9 '\"]*") {
            let quoted = double_quote_string(&s);
            let pieces = split(&quoted).unwrap();
            assert_eq!(1, pieces.len(), "s={s:?}");
            assert_eq!(s, pieces[0]);
        }

        #[test]
        fn awkward_quote_roundtrip(s in "[_a-zA-Z0-9 \"']*") {
            let quoted = awkward_quote_string(&s);
            let pieces = split(&quoted).unwrap();
            assert_eq!(1, pieces.len(), "s={s:?}");
            assert_eq!(s, pieces[0]);
        }

        #[test]
        fn quote_string_roundtrip(s in "[_a-zA-Z0-9 \"']*") {
            let quoted = quote_string(&s);
            let pieces = split(&quoted).unwrap();
            assert_eq!(1, pieces.len(), "s={s:?}");
            assert_eq!(s, pieces[0]);
        }

        #[test]
        fn quote_roundtrip(s1 in "[_a-zA-Z0-9 \"']*", s2 in "[_a-zA-Z0-9 \"']*") {
            if !s1.is_empty() && !s2.is_empty() {
                let quoted = quote(vec![s1.clone(), s2.clone()]);
                let pieces = split(&quoted).unwrap();
                assert_eq!(2, pieces.len(), "quoted={quoted:?}");
                assert_eq!(s1, pieces[0]);
                assert_eq!(s2, pieces[1]);
            }
        }
    }

    #[test]
    fn describe_my_shell_foo_bar_two_words() {
        assert_eq!("foo bar", expand(&mut(), "foo bar").unwrap());
    }

    #[test]
    fn describe_my_shell_foo_bar_single_quotes() {
        assert_eq!(r#""foo bar""#, expand(&mut(), "'foo bar'").unwrap());
    }

    #[test]
    fn describe_my_shell_foo_bar_double_quotes() {
        assert_eq!(r#""foo bar""#, expand(&mut(), r#""foo bar""#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_no_quote() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#"foo bar"#, expand(&mut env, r#"${FOOBAR}"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_single_quotes() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#""${FOOBAR}""#, expand(&mut env, r#"'${FOOBAR}'"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_double_quotes() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#""foo bar""#, expand(&mut env, r#""${FOOBAR}""#).unwrap());
    }

    #[test]
    fn describe_my_shell_abcd_no_quote() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#"abfoo barcd"#, expand(&mut env, r#"ab${FOOBAR}cd"#).unwrap());
    }

    #[test]
    fn describe_my_shell_abcd_double_quotes() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#"ab"foo bar"cd"#, expand(&mut env, r#"ab"${FOOBAR}"cd"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foospace_no_quote() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOSPACE", "foo ")]);
        assert_eq!(r#"foo"#, expand(&mut env, r#"${FOOSPACE}"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foospace_double_quotes() {
        let mut env: HashMap<&str, &str> = HashMap::from([("FOOSPACE", "foo ")]);
        assert_eq!(r#""foo ""#, expand(&mut env, r#""${FOOSPACE}""#).unwrap());
    }
}
