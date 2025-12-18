#![doc = include_str!("../README.md")]

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::{Enumerate, Peekable};
use std::str::Chars;
use std::sync::Arc;

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
    /// Invalid characater.
    InvalidCharacter {
        /// The character that was expected.
        expected: char,
        /// What (if anything) was at this position.
        returned: Option<char>,
    },
    /// There were more than 256 variables during expansion (possible cycle?)
    DepthLimitExceeded,
    /// The user-requested ${FOO:?ERROR MESSAGE} form will return `"ERROR MESSAGE".to_string()` via
    /// this variant.
    Requested(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OpenSingleQuotes => write!(f, "unclosed single quotes"),
            Error::OpenDoubleQuotes => write!(f, "unclosed double quotes"),
            Error::TrailingRightBrace => write!(f, "unmatched right brace"),
            Error::InvalidVariable => write!(f, "invalid variable definition"),
            Error::InvalidCharacter { expected, returned } => match returned {
                Some(c) => write!(f, "expected '{}', found '{}'", expected, c),
                None => write!(f, "expected '{}', found end of input", expected),
            },
            Error::DepthLimitExceeded => {
                write!(f, "expansion depth limit exceeded (possible cycle)")
            }
            Error::Requested(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {}

/////////////////////////////////////////// ExpandOptions //////////////////////////////////////////

/// Configuration for which variable syntax forms are accepted during expansion.
///
/// Shell-style variable expansion supports multiple forms:
/// - `$VARNAME` - bareword form, variable name until non-identifier character
/// - `${VARNAME}` - curly brace form, supports modifiers like `${VAR:-default}`
/// - `$(VARNAME)` - parenthesis form, commonly used in Makefiles
///
/// By default, only `${curlybraces}` syntax is enabled for backward compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpandOptions {
    /// Allow `$VARNAME` syntax (bareword until whitespace/non-identifier).
    pub bareword: bool,
    /// Allow `${VARNAME}` syntax (curly braces).
    pub curly_braces: bool,
    /// Allow `$(VARNAME)` syntax (parentheses).
    pub parens: bool,
}

impl Default for ExpandOptions {
    fn default() -> Self {
        Self {
            bareword: false,
            curly_braces: true,
            parens: false,
        }
    }
}

impl ExpandOptions {
    /// Create options that allow all syntax forms.
    pub fn all() -> Self {
        Self {
            bareword: true,
            curly_braces: true,
            parens: true,
        }
    }

    /// Create options that only allow `$VARNAME` bareword syntax.
    pub fn bareword_only() -> Self {
        Self {
            bareword: true,
            curly_braces: false,
            parens: false,
        }
    }

    /// Create options that only allow `${VARNAME}` curly brace syntax.
    pub fn curly_braces_only() -> Self {
        Self {
            bareword: false,
            curly_braces: true,
            parens: false,
        }
    }

    /// Create options that only allow `$(VARNAME)` parenthesis syntax.
    pub fn parens_only() -> Self {
        Self {
            bareword: false,
            curly_braces: false,
            parens: true,
        }
    }
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
/// Quote the string using double quotes, single quotes, or awkward quotes.
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
        Some(format!("'{s}'"))
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
                if next_word.is_none() {
                    next_word = Some(String::new());
                }
                prev_was_whack = false;
            }
            (State::Unquoted, '"') => {
                state = State::Double;
                if next_word.is_none() {
                    next_word = Some(String::new());
                }
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
pub trait VariableProvider: std::fmt::Debug {
    /// Return the value for the rcvar `ident`.
    fn lookup(&self, ident: &str) -> Option<String>;
}

impl VariableProvider for () {
    fn lookup(&self, _: &str) -> Option<String> {
        None
    }
}

impl<K: Borrow<str> + Eq + Hash + Debug, V: AsRef<str> + Debug> VariableProvider for HashMap<K, V> {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.get(ident).map(|s| s.as_ref().to_string())
    }
}

impl<T: VariableProvider> VariableProvider for Vec<T> {
    fn lookup(&self, ident: &str) -> Option<String> {
        for vp in self.iter() {
            if let Some(value) = vp.lookup(ident) {
                return Some(value);
            }
        }
        None
    }
}

impl<T: VariableProvider> VariableProvider for &T {
    fn lookup(&self, ident: &str) -> Option<String> {
        <T as VariableProvider>::lookup(self, ident)
    }
}

impl<T: VariableProvider> VariableProvider for Box<T> {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.as_ref().lookup(ident)
    }
}

impl VariableProvider for Box<dyn VariableProvider> {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.as_ref().lookup(ident)
    }
}

impl<T: VariableProvider> VariableProvider for Arc<T> {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.as_ref().lookup(ident)
    }
}

macro_rules! impl_tuple_provider {
    ($($name:ident)+) => {
        #[allow(non_snake_case)]
        impl<$($name: VariableProvider),+> VariableProvider for ($($name,)+)
        where ($($name,)+): Debug,
        {
            fn lookup(&self, ident: &str) -> Option<String> {
                let ($(ref $name,)+) = *self;
                $(if let Some(value) = $name.lookup(ident) { return Some(value); })+
                None
            }
        }
    };
}

impl_tuple_provider! { A }
impl_tuple_provider! { A B }
impl_tuple_provider! { A B C }
impl_tuple_provider! { A B C D }
impl_tuple_provider! { A B C D E }
impl_tuple_provider! { A B C D E F }
impl_tuple_provider! { A B C D E F G }
impl_tuple_provider! { A B C D E F G H }
impl_tuple_provider! { A B C D E F G H I }
impl_tuple_provider! { A B C D E F G H I J }
impl_tuple_provider! { A B C D E F G H I J K }
impl_tuple_provider! { A B C D E F G H I J K L }
impl_tuple_provider! { A B C D E F G H I J K L M }
impl_tuple_provider! { A B C D E F G H I J K L M N }
impl_tuple_provider! { A B C D E F G H I J K L M N O }
impl_tuple_provider! { A B C D E F G H I J K L M N O P }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U V }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U V W }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U V W X }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
impl_tuple_provider! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }

//////////////////////////////////////// EnvironmentProvider ///////////////////////////////////////

/// A VariableProvider that reads variable values from the environment.
#[derive(Debug)]
pub struct EnvironmentProvider;

impl VariableProvider for EnvironmentProvider {
    fn lookup(&self, var: &str) -> Option<String> {
        std::env::var(var).ok()
    }
}

///////////////////////////////////// PrefixingVariableProvider ////////////////////////////////////

/// A VariableProvider that prepends a prefix before lookup.
#[derive(Debug)]
pub struct PrefixingVariableProvider<P: VariableProvider> {
    /// Lookup the prefixed queries in this variable provider.
    pub nested: P,
    /// Prepend this prefix to the variable key upon lookup.
    pub prefix: String,
}

impl<P: VariableProvider> VariableProvider for PrefixingVariableProvider<P> {
    fn lookup(&self, var: &str) -> Option<String> {
        let prefixed = self.prefix.clone() + var;
        self.nested.lookup(&prefixed)
    }
}

////////////////////////////////////////// VariableWitness /////////////////////////////////////////

/// A VariableWitness collects the identifiers of variables in use in a string.
pub trait VariableWitness {
    /// Called everytime `ident` is witnessed in the expansion.
    fn witness(&mut self, ident: &str);
}

impl VariableWitness for () {
    fn witness(&mut self, _: &str) {}
}

impl VariableWitness for HashSet<String> {
    fn witness(&mut self, ident: &str) {
        self.insert(String::from(ident));
    }
}

///////////////////////////////////////////// Tokenizer ////////////////////////////////////////////

#[derive(Clone, Debug)]
struct Tokenize<'a> {
    symbols: Peekable<Enumerate<Chars<'a>>>,
}

impl Tokenize<'_> {
    fn new(input: &str) -> Tokenize<'_> {
        let symbols = input.chars().enumerate().peekable();
        Tokenize { symbols }
    }

    fn expect(&mut self, c: char) -> Result<(), Error> {
        if self.accept(c) {
            Ok(())
        } else {
            Err(Error::InvalidCharacter {
                expected: c,
                returned: self.peek(),
            })
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
        self.prev = '"';
        self.inc_quote_count();
    }

    fn close_double_quotes(&mut self) {
        let was_in_quotes = self.within_quotes();
        self.dec_quote_count();
        let is_in_quotes = self.within_quotes();
        if was_in_quotes && !is_in_quotes {
            self.expanded.push('"');
        }
        self.prev = '"';
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

struct ParseContext<'a> {
    options: ExpandOptions,
    generate_errors: bool,
    escape_dollar_literal: bool,
    depth: usize,
    vars: &'a dyn VariableProvider,
}

impl ParseContext<'_> {
    fn deeper(&self) -> ParseContext<'_> {
        ParseContext {
            options: self.options,
            generate_errors: self.generate_errors,
            escape_dollar_literal: self.escape_dollar_literal,
            depth: self.depth + 1,
            vars: self.vars,
        }
    }
}

fn parse_statement(
    ctx: &ParseContext<'_>,
    witness: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    if ctx.depth > 256 {
        return Err(Error::DepthLimitExceeded);
    }
    // SAFETY(rescrv):  If you add another break to this loop, update the assert in `expand`.
    while let Some(c) = tokens.peek() {
        match c {
            '\'' => {
                parse_single_quotes(ctx.vars, witness, tokens, output)?;
            }
            '"' => {
                parse_double_quotes(ctx, witness, tokens, output)?;
            }
            '$' => {
                parse_variable(ctx, witness, tokens, output)?;
            }
            '}' if ctx.options.curly_braces => {
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
    _: &dyn VariableProvider,
    _: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    tokens.expect('\'')?;
    // NOTE(rescrv): Single quotes would seem to want single quotes here.
    // Instead, what we want is to have the literal string pop up in double quotes.
    output.open_double_quotes();
    while let Some(c) = tokens.peek() {
        if tokens.accept('\'') {
            output.close_double_quotes();
            return Ok(());
        } else {
            tokens.accept(c);
            if c == '"' {
                output.push('\\');
                output.push(c);
            } else if c == '\n' {
                output.push('\\');
                output.push('n');
            } else {
                output.push(c);
            }
        }
    }
    Err(Error::OpenSingleQuotes)
}

fn parse_double_quotes(
    ctx: &ParseContext<'_>,
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
                parse_variable(ctx, witness, tokens, output)?;
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
    ctx: &ParseContext<'_>,
    witness: &mut dyn VariableWitness,
    tokens: &mut Tokenize,
    output: &mut Builder,
) -> Result<(), Error> {
    tokens.expect('$')?;

    // Check if this is a short-form automatic variable ($@, $<, $^, $+, $?) or $$
    if let Some(c) = tokens.peek() {
        if matches!(c, '@' | '<' | '^' | '+' | '?') {
            let ident = c.to_string();
            tokens.expect(c)?;
            witness.witness(&ident);
            if let Some(val) = ctx.vars.lookup(&ident) {
                output.push_str(&val);
            }
            return Ok(());
        } else if c == '$' {
            // Handle $$ - behavior depends on escape_dollar_literal
            tokens.expect('$')?;
            witness.witness("$");
            if ctx.escape_dollar_literal {
                output.push_str("$$");
            } else {
                output.push('$');
            }
            return Ok(());
        }
    }

    // Determine syntax form: ${...}, $(...), or $BAREWORD
    // Check what follows the $
    let next_char = tokens.peek();

    // Try paren form $(...)
    if next_char == Some('(') && ctx.options.parens {
        tokens.expect('(')?;
        let ident = parse_identifier(tokens)?;
        witness.witness(&ident);
        if let Some(val) = ctx.vars.lookup(&ident) {
            output.push_str(&val);
        } else if ident == "$" {
            if ctx.escape_dollar_literal {
                output.push_str("$$");
            } else {
                output.push_str("$");
            }
        }
        tokens.expect(')')?;
        return Ok(());
    }

    // Try curly brace form ${...}
    if next_char == Some('{') && ctx.options.curly_braces {
        tokens.expect('{')?;
        let ident = parse_identifier(tokens)?;
        witness.witness(&ident);
        if tokens.accept(':') {
            let Some(action) = tokens.peek() else {
                return Err(Error::InvalidVariable);
            };
            tokens.accept(action);
            let mut expanded = Builder::from_other(output);
            parse_statement(&ctx.deeper(), witness, tokens, &mut expanded)?;
            match action {
                '-' => {
                    if let Some(val) = ctx.vars.lookup(&ident) {
                        output.push_str(&val);
                    } else {
                        output.append(expanded);
                    }
                }
                '+' => {
                    if ctx.vars.lookup(&ident).is_some() {
                        output.append(expanded);
                    }
                }
                '?' => {
                    if let Some(val) = ctx.vars.lookup(&ident) {
                        output.push_str(&val);
                    } else if ctx.generate_errors {
                        return Err(Error::Requested(expanded.into_string()));
                    }
                }
                c => {
                    return Err(Error::InvalidCharacter {
                        expected: '-',
                        returned: Some(c),
                    });
                }
            }
        } else if let Some(val) = ctx.vars.lookup(&ident) {
            output.push_str(&val);
        } else if ident == "$" {
            if ctx.escape_dollar_literal {
                output.push_str("$$");
            } else {
                output.push_str("$");
            }
        }
        tokens.expect('}')?;
        return Ok(());
    }

    // Try bareword form $VARNAME
    if ctx.options.bareword {
        if let Some(c) = next_char {
            if matches!(c, 'a'..='z' | 'A'..='Z' | '_') {
                let ident = parse_bareword_identifier(tokens)?;
                witness.witness(&ident);
                if let Some(val) = ctx.vars.lookup(&ident) {
                    output.push_str(&val);
                }
                return Ok(());
            }
        }
    }

    // No valid syntax found - output literal $
    output.push('$');
    Ok(())
}

fn parse_bareword_identifier(tokens: &mut Tokenize) -> Result<String, Error> {
    let mut identifier = String::new();
    let mut first = true;
    while let Some(c) = tokens.peek() {
        match c {
            'a'..='z' | 'A'..='Z' | '_' if first => {
                identifier.push(c);
                tokens.expect(c)?;
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' if !first => {
                identifier.push(c);
                tokens.expect(c)?;
            }
            _ => {
                break;
            }
        }
        first = false;
    }
    if identifier.is_empty() {
        return Err(Error::InvalidVariable);
    }
    Ok(identifier)
}

fn parse_identifier(tokens: &mut Tokenize) -> Result<String, Error> {
    let mut identifier = String::new();
    let mut first = true;
    while let Some(c) = tokens.peek() {
        match c {
            // Regular variable names
            'a'..='z' | 'A'..='Z' | '_' if first => {
                identifier.push(c);
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' if !first => {
                identifier.push(c);
            }
            // Make-style automatic variables (single character)
            '@' | '<' | '^' | '+' | '?' | '$' if first => {
                let special_ident = c.to_string();
                tokens.expect(c)?;
                return Ok(special_ident);
            }
            _ => {
                if !identifier.is_empty() {
                    return Ok(identifier);
                } else {
                    return Err(Error::InvalidVariable);
                }
            }
        }
        tokens.expect(c)?;
        first = false;
    }
    Ok(identifier)
}

////////////////////////////////////////////// expand //////////////////////////////////////////////

/// Default options for backward compatibility: curly braces and parens enabled.
const LEGACY_OPTIONS: ExpandOptions = ExpandOptions {
    bareword: false,
    curly_braces: true,
    parens: true,
};

/// Expand the input to a shell-quoted string suitable for passing to `split`.
///
/// Uses legacy options (curly braces and parens) for backward compatibility.
pub fn expand(vars: &dyn VariableProvider, input: &str) -> Result<String, Error> {
    expand_once(LEGACY_OPTIONS, vars, input, false)
}

/// Expand the input with custom options to a shell-quoted string suitable for passing to `split`.
pub fn expand_with_options(
    options: ExpandOptions,
    vars: &dyn VariableProvider,
    input: &str,
) -> Result<String, Error> {
    expand_once(options, vars, input, false)
}

fn expand_once(
    options: ExpandOptions,
    vars: &dyn VariableProvider,
    input: &str,
    escape_dollar_literal: bool,
) -> Result<String, Error> {
    let ctx = ParseContext {
        options,
        generate_errors: true,
        escape_dollar_literal,
        depth: 0,
        vars,
    };
    let mut tokens = Tokenize::new(input);
    let mut output = Builder::default();
    parse_statement(&ctx, &mut (), &mut tokens, &mut output)?;
    if tokens.peek().is_some() {
        // SAFETY(rescrv): We can only break out of the loop early on '}' when curly_braces is enabled.
        assert!(options.curly_braces);
        assert_eq!(Some('}'), tokens.peek());
        return Err(Error::TrailingRightBrace);
    }
    let result = output.into_string().trim().to_string();
    Ok(result)
}

///////////////////////////////////////// expand_recursive /////////////////////////////////////////

/// Recursively expand variables until no more expansions are possible.
///
/// Uses legacy options (curly braces and parens) for backward compatibility.
pub fn expand_recursive(vars: &dyn VariableProvider, input: &str) -> Result<String, Error> {
    expand_recursive_with_options(LEGACY_OPTIONS, vars, input)
}

/// Recursively expand variables with custom options until no more expansions are possible.
pub fn expand_recursive_with_options(
    options: ExpandOptions,
    vars: &dyn VariableProvider,
    input: &str,
) -> Result<String, Error> {
    fn generate_witnesses(
        options: ExpandOptions,
        vars: &dyn VariableProvider,
        input: &str,
    ) -> Result<HashSet<String>, Error> {
        let ctx = ParseContext {
            options,
            generate_errors: false,
            escape_dollar_literal: true,
            depth: 0,
            vars,
        };
        let mut witnesses = HashSet::default();
        let mut tokens = Tokenize::new(input);
        let mut output = Builder::default();
        parse_statement(&ctx, &mut witnesses, &mut tokens, &mut output)?;
        Ok(witnesses)
    }

    fn post_process(options: ExpandOptions, s: &str) -> Result<String, Error> {
        expand_once(options, &(), s, false)
    }

    let mut witnesses = generate_witnesses(options, vars, input)?;
    let mut input = input.to_string();
    for _ in 0..128 {
        let once = expand_once(options, vars, &input, true)?;
        if once == input {
            return post_process(options, &once);
        }
        let new_witnesses = generate_witnesses(options, vars, &once)?;
        if new_witnesses.is_empty() || (new_witnesses.len() == 1 && new_witnesses.contains("$")) {
            return post_process(options, &once);
        }
        if witnesses.is_subset(&new_witnesses) {
            return Err(Error::DepthLimitExceeded);
        }
        input = once;
        witnesses = new_witnesses;
    }
    Err(Error::DepthLimitExceeded)
}

/////////////////////////////////////////////// rcvar //////////////////////////////////////////////

/// Return a vector of the variables in use by this script.
///
/// Uses legacy options (curly braces and parens) for backward compatibility.
pub fn rcvar(input: &str) -> Result<Vec<String>, Error> {
    rcvar_with_options(LEGACY_OPTIONS, input)
}

/// Return a vector of the variables in use by this script with custom options.
pub fn rcvar_with_options(options: ExpandOptions, input: &str) -> Result<Vec<String>, Error> {
    let ctx = ParseContext {
        options,
        generate_errors: false,
        escape_dollar_literal: true,
        depth: 0,
        vars: &(),
    };
    let mut tokens = Tokenize::new(input);
    let mut output = Builder::default();
    let mut witnesses: HashSet<String> = HashSet::new();
    parse_statement(&ctx, &mut witnesses, &mut tokens, &mut output)?;
    if tokens.peek().is_some() {
        // SAFETY(rescrv): We can only break out of the loop early on '}' when curly_braces is enabled.
        assert!(options.curly_braces);
        assert_eq!(Some('}'), tokens.peek());
        return Err(Error::TrailingRightBrace);
    }
    let mut witnesses: Vec<_> = witnesses.into_iter().collect();
    witnesses.sort();
    Ok(witnesses)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_all_empty() {
        let env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", ""), ("s3", "")]);
        assert_eq!("", expand(&env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("\"\"", expand(&env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_space_empty_empty() {
        let env: HashMap<&str, &str> = HashMap::from([("s1", " "), ("s2", ""), ("s3", "")]);
        assert_eq!("", expand(&env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("\"\"", expand(&env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_empty_space_empty() {
        let env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", " "), ("s3", "")]);
        assert_eq!("", expand(&env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("\" \"", expand(&env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn expand_empty_empty_space() {
        let env: HashMap<&str, &str> = HashMap::from([("s1", ""), ("s2", ""), ("s3", " ")]);
        assert_eq!("", expand(&env, "${s1}${s2}${s3}").unwrap());
        assert_eq!("\"\"", expand(&env, "${s1}\"${s2}\"${s3}").unwrap());
    }

    #[test]
    fn sample_expansion() {
        let env: HashMap<&str, &str> =
            HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
        assert_eq!("foo-bar-baz", expand(&env, "${FOO}-${BAR}-${BAZ}").unwrap());
    }

    #[test]
    fn novar_expansion() {
        let env: HashMap<&str, &str> = HashMap::new();
        assert_eq!(
            r#""" "" """#,
            expand(&env, "\"${FOO}\" \"${BAR}\" \"${BAZ}\"").unwrap()
        );
    }

    #[test]
    fn my_command1() {
        let env: HashMap<&str, &str> = HashMap::new();
        assert_eq!(
            r#"my-command --args" "foo --field1 "" --field2 """#,
            expand(
                &env,
                "my-command --args\" \"foo --field1 \"${FIELD1}\" --field2 \"${FIELD2}\""
            )
            .unwrap()
        );
    }

    #[test]
    fn my_command2() {
        assert_eq!(
            vec![
                "my-command".to_string(),
                "--args foo".to_string(),
                "--field1".to_string(),
                "".to_string(),
                "--field2".to_string(),
                "".to_string(),
            ],
            split("my-command --args\" \"foo --field1 \"\" --field2 \"\"").unwrap(),
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
        assert_eq!("foo bar", expand(&(), "foo bar").unwrap());
    }

    #[test]
    fn describe_my_shell_foo_bar_single_quotes() {
        assert_eq!(r#""foo bar""#, expand(&(), "'foo bar'").unwrap());
    }

    #[test]
    fn describe_my_shell_foo_bar_double_quotes() {
        assert_eq!(r#""foo bar""#, expand(&(), r#""foo bar""#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_no_quote() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#"foo bar"#, expand(&env, r#"${FOOBAR}"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_single_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#""${FOOBAR}""#, expand(&env, r#"'${FOOBAR}'"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foobar_double_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#""foo bar""#, expand(&env, r#""${FOOBAR}""#).unwrap());
    }

    #[test]
    fn describe_my_shell_abcd_no_quote() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(r#"abfoo barcd"#, expand(&env, r#"ab${FOOBAR}cd"#).unwrap());
    }

    #[test]
    fn describe_my_shell_abcd_double_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOBAR", "foo bar")]);
        assert_eq!(
            r#"ab"foo bar"cd"#,
            expand(&env, r#"ab"${FOOBAR}"cd"#).unwrap()
        );
    }

    #[test]
    fn describe_my_shell_foospace_no_quote() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOSPACE", "foo ")]);
        assert_eq!(r#"foo"#, expand(&env, r#"${FOOSPACE}"#).unwrap());
    }

    #[test]
    fn describe_my_shell_foospace_double_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([("FOOSPACE", "foo ")]);
        assert_eq!(r#""foo ""#, expand(&env, r#""${FOOSPACE}""#).unwrap());
    }

    #[test]
    fn four_rcvar() {
        assert_eq!(
            vec![
                "BAR".to_string(),
                "BAZ".to_string(),
                "FOO".to_string(),
                "QUUX".to_string()
            ],
            rcvar("${FOO}-${BAR}-${BAZ}-${QUUX}").unwrap(),
        );
    }

    #[test]
    fn expand_recursive() {
        let vp = HashMap::from_iter([
            ("HOST", "${METRO}.${CUSTOMER}.example.org"),
            ("METRO", "sjc"),
            ("CUSTOMER", "CyberDyne"),
        ]);
        assert_eq!(
            "sjc.CyberDyne.example.org",
            super::expand_recursive(&vp, "${HOST}").unwrap()
        );
    }

    #[test]
    fn make_automatic_variables_long_form() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "target.o"),
            ("<", "source.c"),
            ("^", "source.c header.h"),
            ("+", "source.c header.h source.c"),
            ("?", "source.c"),
        ]);

        assert_eq!("target.o", expand(&env, "${@}").unwrap());
        assert_eq!("source.c", expand(&env, "${<}").unwrap());
        assert_eq!("source.c header.h", expand(&env, "${^}").unwrap());
        assert_eq!("source.c header.h source.c", expand(&env, "${+}").unwrap());
        assert_eq!("source.c", expand(&env, "${?}").unwrap());
    }

    #[test]
    fn make_automatic_variables_short_form() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "target.o"),
            ("<", "source.c"),
            ("^", "source.c header.h"),
            ("+", "source.c header.h source.c"),
            ("?", "source.c"),
        ]);

        assert_eq!("target.o", expand(&env, "$@").unwrap());
        assert_eq!("source.c", expand(&env, "$<").unwrap());
        assert_eq!("source.c header.h", expand(&env, "$^").unwrap());
        assert_eq!("source.c header.h source.c", expand(&env, "$+").unwrap());
        assert_eq!("source.c", expand(&env, "$?").unwrap());
    }

    #[test]
    fn make_automatic_variables_long_form_in_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "my target.o"),
            ("<", "my source.c"),
            ("^", "my dependencies.h header.h"),
            ("+", "my all.c files.c"),
            ("?", "my newer.c"),
        ]);

        assert_eq!(r#""my target.o""#, expand(&env, r#""${@}""#).unwrap());
        assert_eq!(r#""my source.c""#, expand(&env, r#""${<}""#).unwrap());
        assert_eq!(
            r#""my dependencies.h header.h""#,
            expand(&env, r#""${^}""#).unwrap()
        );
        assert_eq!(r#""my all.c files.c""#, expand(&env, r#""${+}""#).unwrap());
        assert_eq!(r#""my newer.c""#, expand(&env, r#""${?}""#).unwrap());
    }

    #[test]
    fn make_automatic_variables_short_form_in_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "my target.o"),
            ("<", "my source.c"),
            ("^", "my dependencies.h header.h"),
            ("+", "my all.c files.c"),
            ("?", "my newer.c"),
        ]);

        assert_eq!(r#""my target.o""#, expand(&env, r#""$@""#).unwrap());
        assert_eq!(r#""my source.c""#, expand(&env, r#""$<""#).unwrap());
        assert_eq!(
            r#""my dependencies.h header.h""#,
            expand(&env, r#""$^""#).unwrap()
        );
        assert_eq!(r#""my all.c files.c""#, expand(&env, r#""$+""#).unwrap());
        assert_eq!(r#""my newer.c""#, expand(&env, r#""$?""#).unwrap());
    }

    #[test]
    fn make_automatic_variables_mixed_forms() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "target.o"),
            ("<", "source.c"),
            ("^", "dependencies.h header.h"),
            ("+", "all.c files.c"),
            ("?", "newer.c"),
            ("{@}", "target.o"),
            ("{<}", "source.c"),
            ("{^}", "dependencies.h header.h"),
            ("{+}", "all.c files.c"),
            ("{?}", "newer.c"),
        ]);

        // Test mixing short and long forms
        assert_eq!("target.o source.c", expand(&env, "$@ ${<}").unwrap());
        assert_eq!("target.o source.c", expand(&env, "${@} $<").unwrap());
        assert_eq!(
            "dependencies.h header.h all.c files.c",
            expand(&env, "$^ ${+}").unwrap()
        );
        assert_eq!(
            "dependencies.h header.h all.c files.c",
            expand(&env, "${^} $+").unwrap()
        );
        assert_eq!("newer.c target.o", expand(&env, "$? ${@}").unwrap());
        assert_eq!("newer.c target.o", expand(&env, "${?} $@").unwrap());
    }

    #[test]
    fn make_automatic_variables_long_form_rcvar() {
        assert_eq!(
            vec![
                "+".to_string(),
                "<".to_string(),
                "?".to_string(),
                "@".to_string(),
                "^".to_string()
            ],
            rcvar("${@} ${<} ${^} ${+} ${?}").unwrap(),
        );
    }

    #[test]
    fn make_automatic_variables_short_form_rcvar() {
        assert_eq!(
            vec![
                "+".to_string(),
                "<".to_string(),
                "?".to_string(),
                "@".to_string(),
                "^".to_string()
            ],
            rcvar("$@ $< $^ $+ $?").unwrap(),
        );
    }

    #[test]
    fn make_automatic_variables_mixed_forms_rcvar() {
        assert_eq!(
            vec![
                "+".to_string(),
                "<".to_string(),
                "?".to_string(),
                "@".to_string(),
                "^".to_string()
            ],
            rcvar("$@ ${<} $^ ${+} $?").unwrap(),
        );
    }

    #[test]
    fn make_automatic_variables_consistent_substitution() {
        // Test that $@ and ${@} resolve to the same value
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "target.o"),
            ("<", "source.c"),
            ("^", "deps.h"),
            ("+", "all.c"),
            ("?", "newer.c"),
        ]);

        // Test that short and long forms resolve to the same values
        assert_eq!("target.o", expand(&env, "$@").unwrap());
        assert_eq!("target.o", expand(&env, "${@}").unwrap());

        assert_eq!("source.c", expand(&env, "$<").unwrap());
        assert_eq!("source.c", expand(&env, "${<}").unwrap());

        assert_eq!("deps.h", expand(&env, "$^").unwrap());
        assert_eq!("deps.h", expand(&env, "${^}").unwrap());

        assert_eq!("all.c", expand(&env, "$+").unwrap());
        assert_eq!("all.c", expand(&env, "${+}").unwrap());

        assert_eq!("newer.c", expand(&env, "$?").unwrap());
        assert_eq!("newer.c", expand(&env, "${?}").unwrap());

        // Test mixing different forms in same expression
        assert_eq!("target.o source.c", expand(&env, "$@ ${<}").unwrap());
        assert_eq!("target.o source.c", expand(&env, "${@} $<").unwrap());
    }

    #[test]
    fn dollar_paren_syntax_regular_variables() {
        let env: HashMap<&str, &str> =
            HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);

        assert_eq!("foo", expand(&env, "$(FOO)").unwrap());
        assert_eq!("bar", expand(&env, "$(BAR)").unwrap());
        assert_eq!("baz", expand(&env, "$(BAZ)").unwrap());
        assert_eq!("foo-bar-baz", expand(&env, "$(FOO)-$(BAR)-$(BAZ)").unwrap());
    }

    #[test]
    fn dollar_paren_syntax_automatic_variables() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("@", "paren-target.o"),
            ("<", "paren-source.c"),
            ("^", "paren-dependencies.h header.h"),
            ("+", "paren-all.c files.c"),
            ("?", "paren-newer.c"),
        ]);

        assert_eq!("paren-target.o", expand(&env, "$(@)").unwrap());
        assert_eq!("paren-source.c", expand(&env, "$(<)").unwrap());
        assert_eq!(
            "paren-dependencies.h header.h",
            expand(&env, "$(^)").unwrap()
        );
        assert_eq!("paren-all.c files.c", expand(&env, "$(+)").unwrap());
        assert_eq!("paren-newer.c", expand(&env, "$(?)").unwrap());
    }

    #[test]
    fn dollar_paren_syntax_in_quotes() {
        let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo bar"), ("@", "my target.o")]);

        assert_eq!(r#""foo bar""#, expand(&env, r#""$(FOO)""#).unwrap());
        assert_eq!(r#""my target.o""#, expand(&env, r#""$(@)""#).unwrap());
    }

    #[test]
    fn dollar_paren_syntax_mixed_with_other_forms() {
        let env: HashMap<&str, &str> =
            HashMap::from([("FOO", "foo"), ("@", "consistent-at"), ("BAR", "bar")]);

        // Mix $(VAR) with ${VAR} and $VAR
        assert_eq!("foo bar", expand(&env, "$(FOO) ${BAR}").unwrap());
        assert_eq!(
            "consistent-at consistent-at consistent-at",
            expand(&env, "$@ ${@} $(@)").unwrap()
        );
    }

    #[test]
    fn dollar_paren_syntax_rcvar() {
        assert_eq!(
            vec![
                "<".to_string(),
                "?".to_string(),
                "@".to_string(),
                "FOO".to_string(),
                "^".to_string(),
            ],
            rcvar("$(FOO) $(@) $(<) $(^) $(?)").unwrap(),
        );
    }

    #[test]
    fn dollar_dollar_literal_expansion() {
        let env: HashMap<&str, &str> = HashMap::new();

        // Test short form $$
        assert_eq!("$", expand(&env, "$$").unwrap());

        // Test long forms ${$} and $($)
        assert_eq!("$", expand(&env, "${$}").unwrap());
        assert_eq!("$", expand(&env, "$($)").unwrap());

        // Test in context
        assert_eq!("Price: $10", expand(&env, "Price: $$10").unwrap());
        assert_eq!("Price: $10", expand(&env, "Price: ${$}10").unwrap());
        assert_eq!("Price: $10", expand(&env, "Price: $($)10").unwrap());

        // Test multiple $$ in one string
        assert_eq!("$1 $2 $3", expand(&env, "$$1 $$2 $$3").unwrap());

        // Test with other variables
        let env2: HashMap<&str, &str> = HashMap::from([("FOO", "bar")]);
        assert_eq!("bar$", expand(&env2, "${FOO}$$").unwrap());
        assert_eq!("$bar", expand(&env2, "$$${FOO}").unwrap());
    }

    #[test]
    fn dollar_dollar_in_quotes() {
        let env: HashMap<&str, &str> = HashMap::new();

        // Test in double quotes
        assert_eq!("\"$\"", expand(&env, "\"$$\"").unwrap());
        assert_eq!("\"$\"", expand(&env, "\"${$}\"").unwrap());
        assert_eq!("\"$\"", expand(&env, "\"$($)\"").unwrap());

        // Test in single quotes (should be literal)
        assert_eq!("\"$$\"", expand(&env, "'$$'").unwrap());
        assert_eq!("\"${$}\"", expand(&env, "'${$}'").unwrap());
        assert_eq!("\"$($)\"", expand(&env, "'$($)'").unwrap());
    }

    #[test]
    fn dollar_dollar_rcvar() {
        // Test that $$ is properly tracked in rcvar
        assert_eq!(vec!["$".to_string()], rcvar("$$").unwrap(),);

        assert_eq!(vec!["$".to_string()], rcvar("${$}").unwrap(),);

        assert_eq!(vec!["$".to_string()], rcvar("$($)").unwrap(),);

        // Test mixed with other variables
        assert_eq!(
            vec!["$".to_string(), "FOO".to_string()],
            rcvar("$$ ${FOO}").unwrap(),
        );
    }

    #[test]
    fn dollar_dollar_comprehensive_edge_cases() {
        let env: HashMap<&str, &str> = HashMap::new();

        // Test consecutive $$ expansions
        assert_eq!("$$", expand(&env, "$$$$").unwrap());

        // Test $$ followed immediately by digits (like process ID)
        assert_eq!("$123", expand(&env, "$$123").unwrap());
        assert_eq!("$456", expand(&env, "$$456").unwrap());

        // Test $$ in shell command contexts
        assert_eq!("kill -9 $", expand(&env, "kill -9 $$").unwrap());
        assert_eq!("echo $ > file", expand(&env, "echo $$ > file").unwrap());

        // Test $$ with mixed variable forms
        let env2: HashMap<&str, &str> = HashMap::from([("PID", "12345")]);
        assert_eq!(
            "Process $ has PID 12345",
            expand(&env2, "Process $$ has PID ${PID}").unwrap()
        );

        // Test $$ in complex quoted scenarios
        assert_eq!("\"echo $\"", expand(&env, "\"echo $$\"").unwrap());
        assert_eq!("\"$123\"", expand(&env, "\"$$123\"").unwrap());

        // Test split functionality with $$ results
        let expanded = expand(&env, "arg1 $$ arg3").unwrap();
        assert_eq!("arg1 $ arg3", expanded);
        let split_result = split(&expanded).unwrap();
        assert_eq!(vec!["arg1", "$", "arg3"], split_result);
    }

    #[test]
    fn expand_recursive_dollar_dollar_monotonic() {
        let env: HashMap<&str, &str> = HashMap::new();

        // Test that expand_recursive stops when only $$ -> $ transformation occurs
        assert_eq!("$", super::expand_recursive(&env, "$$").unwrap());
        assert_eq!("$ $", super::expand_recursive(&env, "$$ $$").unwrap());
        assert_eq!("test $", super::expand_recursive(&env, "test $$").unwrap());
        assert_eq!("$ test", super::expand_recursive(&env, "$$ test").unwrap());
        assert_eq!(
            "$ test $",
            super::expand_recursive(&env, "$$ test $$").unwrap()
        );

        // Test that it still works with real variables
        let env2: HashMap<&str, &str> = HashMap::from([("FOO", "bar")]);

        // First test basic expand works
        println!("expand result: {:?}", expand(&env2, "${FOO} $$"));

        assert_eq!(
            "bar $",
            super::expand_recursive(&env2, "${FOO} $$").unwrap()
        );
        assert_eq!(
            "$ bar",
            super::expand_recursive(&env2, "$$ ${FOO}").unwrap()
        );
    }

    #[test]
    fn expand_options_bareword_syntax() {
        let env: HashMap<&str, &str> =
            HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);

        let opts = ExpandOptions::all();

        // bareword expansion
        assert_eq!("foo", expand_with_options(opts, &env, "$FOO").unwrap());
        assert_eq!("bar", expand_with_options(opts, &env, "$BAR").unwrap());
        assert_eq!("baz", expand_with_options(opts, &env, "$BAZ").unwrap());

        // bareword with trailing text
        assert_eq!(
            "foo-bar",
            expand_with_options(opts, &env, "$FOO-$BAR").unwrap()
        );

        // bareword at end of string
        assert_eq!(
            "value: foo",
            expand_with_options(opts, &env, "value: $FOO").unwrap()
        );

        // bareword followed by non-identifier characters
        assert_eq!(
            "foo/bar",
            expand_with_options(opts, &env, "$FOO/$BAR").unwrap()
        );
        assert_eq!(
            "foo.bar",
            expand_with_options(opts, &env, "$FOO.$BAR").unwrap()
        );

        // bareword in quotes
        assert_eq!(
            "\"foo bar\"",
            expand_with_options(opts, &env, "\"$FOO $BAR\"").unwrap()
        );

        // unset variable as bareword
        assert_eq!("", expand_with_options(opts, &env, "$NOTSET").unwrap());
    }

    #[test]
    fn expand_options_bareword_only() {
        let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
        let opts = ExpandOptions::bareword_only();

        // bareword works
        assert_eq!("foo", expand_with_options(opts, &env, "$FOO").unwrap());

        // curly braces treated as literal
        assert_eq!("${FOO}", expand_with_options(opts, &env, "${FOO}").unwrap());

        // parens treated as literal
        assert_eq!("$(FOO)", expand_with_options(opts, &env, "$(FOO)").unwrap());
    }

    #[test]
    fn expand_options_curly_braces_only() {
        let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
        let opts = ExpandOptions::curly_braces_only();

        // curly braces work
        assert_eq!("foo", expand_with_options(opts, &env, "${FOO}").unwrap());

        // bareword $ is literal
        assert_eq!("$FOO", expand_with_options(opts, &env, "$FOO").unwrap());

        // parens treated as literal
        assert_eq!("$(FOO)", expand_with_options(opts, &env, "$(FOO)").unwrap());
    }

    #[test]
    fn expand_options_parens_only() {
        let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
        let opts = ExpandOptions::parens_only();

        // parens work
        assert_eq!("foo", expand_with_options(opts, &env, "$(FOO)").unwrap());

        // curly braces treated as literal
        assert_eq!("${FOO}", expand_with_options(opts, &env, "${FOO}").unwrap());

        // bareword $ is literal
        assert_eq!("$FOO", expand_with_options(opts, &env, "$FOO").unwrap());
    }

    #[test]
    fn expand_options_all_forms() {
        let env: HashMap<&str, &str> = HashMap::from([("A", "a"), ("B", "b"), ("C", "c")]);
        let opts = ExpandOptions::all();

        // all three forms in same string
        assert_eq!(
            "a b c",
            expand_with_options(opts, &env, "$A ${B} $(C)").unwrap()
        );

        // mixed forms with text
        assert_eq!(
            "val=a, curly=b, paren=c",
            expand_with_options(opts, &env, "val=$A, curly=${B}, paren=$(C)").unwrap()
        );
    }

    #[test]
    fn expand_options_bareword_edge_cases() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("FOO", "foo"),
            ("FOO_BAR", "foobar"),
            ("_UNDERSCORE", "under"),
            ("A1", "a1"),
        ]);
        let opts = ExpandOptions::all();

        // underscore in variable name
        assert_eq!(
            "foobar",
            expand_with_options(opts, &env, "$FOO_BAR").unwrap()
        );

        // leading underscore
        assert_eq!(
            "under",
            expand_with_options(opts, &env, "$_UNDERSCORE").unwrap()
        );

        // digits after first char
        assert_eq!("a1", expand_with_options(opts, &env, "$A1").unwrap());

        // $ followed by digit is literal
        assert_eq!("$1", expand_with_options(opts, &env, "$1").unwrap());

        // $ followed by non-identifier is literal
        assert_eq!("$-test", expand_with_options(opts, &env, "$-test").unwrap());

        // $ at end of string
        assert_eq!("test$", expand_with_options(opts, &env, "test$").unwrap());

        // consecutive $
        assert_eq!("$", expand_with_options(opts, &env, "$$").unwrap());
    }

    #[test]
    fn expand_options_rcvar_bareword() {
        let opts = ExpandOptions::all();

        assert_eq!(
            vec!["BAR".to_string(), "FOO".to_string()],
            rcvar_with_options(opts, "$FOO $BAR").unwrap()
        );

        // mixed forms
        assert_eq!(
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            rcvar_with_options(opts, "$A ${B} $(C)").unwrap()
        );
    }

    #[test]
    fn expand_options_recursive_bareword() {
        let env: HashMap<&str, &str> = HashMap::from([
            ("HOST", "$METRO.$CUSTOMER.example.org"),
            ("METRO", "sjc"),
            ("CUSTOMER", "CyberDyne"),
        ]);
        let opts = ExpandOptions::all();

        assert_eq!(
            "sjc.CyberDyne.example.org",
            expand_recursive_with_options(opts, &env, "$HOST").unwrap()
        );
    }

    #[test]
    fn expand_options_default() {
        // default should be curly_braces only
        let opts = ExpandOptions::default();
        assert!(!opts.bareword);
        assert!(opts.curly_braces);
        assert!(!opts.parens);
    }

    #[test]
    fn expand_options_curly_with_modifiers() {
        let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
        let opts = ExpandOptions::all();

        // default modifier
        assert_eq!(
            "default",
            expand_with_options(opts, &env, "${NOTSET:-default}").unwrap()
        );
        assert_eq!(
            "foo",
            expand_with_options(opts, &env, "${FOO:-default}").unwrap()
        );

        // alternate modifier
        assert_eq!(
            "",
            expand_with_options(opts, &env, "${NOTSET:+alternate}").unwrap()
        );
        assert_eq!(
            "alternate",
            expand_with_options(opts, &env, "${FOO:+alternate}").unwrap()
        );
    }
}
