use std::error::Error;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use arrrg::CommandLine;
use caternary::{
    CODE_OPERATOR_ERROR, EvalError, Evaluator, ParseError, Quotable, Scheme, SmtLibSolver, Span,
    SpannedToken, SpannedTokenKind, StackTy, Token, Ty, WordTy, check_whole_program, core_scheme,
    format_word_type, infer_quote_type, parse_with_spans, register_all_builtins,
};
use handled::SError;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {}

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct CheckOptions {}

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct ReplOptions {}

enum Command {
    Check(CheckOptions, Vec<String>),
    Repl(ReplOptions, Vec<String>),
}

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CliError {}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Word(String),
    Number(f64),
    Bool(bool),
    Quotation(Vec<Token>),
}

const CORE_OPERATOR_NAMES: &[&str] = &[
    "DUP", "DROP", "SWAP", "OVER", "ROT", "-ROT", "NIP", "TUCK", "2DUP", "2DROP", "2SWAP", "2OVER",
    "2ROT", "CALL", "DIP", "2DIP", "3DIP", "IF", "KEEP", "2KEEP", "3KEEP", "BI", "BI*", "BI@",
    "TRI", "TRI*", "TRI@", "COMPOSE",
];

impl From<Token> for Value {
    fn from(token: Token) -> Self {
        match token {
            Token::Word(w) => {
                if let Ok(n) = w.parse::<f64>() {
                    Value::Number(n)
                } else if w == "true" {
                    Value::Bool(true)
                } else if w == "false" {
                    Value::Bool(false)
                } else {
                    Value::Word(w)
                }
            }
            Token::Bracket(tokens) => Value::Quotation(tokens),
        }
    }
}

impl Quotable for Value {
    fn as_quotation(&self) -> Option<&[Token]> {
        match self {
            Value::Quotation(tokens) => Some(tokens),
            _ => None,
        }
    }

    fn to_tokens(&self) -> Vec<Token> {
        match self {
            Value::Word(w) => vec![Token::Word(w.clone())],
            Value::Number(n) => vec![Token::Word(n.to_string())],
            Value::Bool(b) => vec![Token::Word(b.to_string())],
            Value::Quotation(tokens) => vec![Token::Bracket(tokens.clone())],
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            _ => true,
        }
    }

    fn as_sequence(&self) -> Option<Vec<Self>> {
        match self {
            Value::Quotation(tokens) => Some(tokens.iter().cloned().map(Value::from).collect()),
            _ => None,
        }
    }

    fn from_sequence(elements: Vec<Self>) -> Self {
        Value::Quotation(elements.into_iter().flat_map(|v| v.to_tokens()).collect())
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("caternary: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let (_options, free) = Options::from_command_line_relaxed(
        "Usage: caternary [OPTIONS] <check|repl> [COMMAND OPTIONS]",
    );
    let command = arrrg::dispatch_subcommands!(free, {
        "check" => CheckOptions as check, check_free => {
            Ok(Command::Check(check, check_free))
        },
        "repl" => ReplOptions as repl, repl_free => {
            Ok(Command::Repl(repl, repl_free))
        },
    })?;
    match command {
        Command::Check(options, free) => check_command(options, free),
        Command::Repl(options, free) => repl_command(options, free),
    }
}

fn check_command(_options: CheckOptions, free: Vec<String>) -> Result<()> {
    expect_no_positionals("check", &free)?;

    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;

    let mut evaluator = new_evaluator();
    let tokens = parse_with_spans(&source)?;
    evaluator.load_with_spans(&tokens)?;
    check_whole_program(&evaluator, SmtLibSolver::new)?;

    Ok(())
}

fn repl_command(_options: ReplOptions, free: Vec<String>) -> Result<()> {
    expect_no_positionals("repl", &free)?;

    let config = Config::builder()
        .max_history_size(1_000_000)?
        .history_ignore_dups(true)?
        .history_ignore_space(true)
        .build();
    let hist = FileHistory::with_config(config);
    let mut rl: Editor<(), FileHistory> = Editor::with_history(config, hist)?;
    let history = history_path();
    if let Some(history) = &history
        && history.exists()
        && let Err(err) = rl.load_history(history)
    {
        eprintln!("warning: could not load history: {err}");
    }

    let mut state = ReplState::new();
    let mut pending = String::new();

    loop {
        let prompt = if pending.is_empty() {
            "caternary> "
        } else {
            "... "
        };
        match rl.readline(prompt) {
            Ok(line) => {
                rl.add_history_entry(&line)?;
                append_line(&mut pending, &line);
                let source = pending.trim();
                if source.is_empty() {
                    pending.clear();
                    continue;
                }
                if source.starts_with(':') {
                    let command = pending.clone();
                    pending.clear();
                    match state.handle_meta_command(command.trim()) {
                        Ok(ReplAction::Quit) => {
                            save_history(&mut rl, history.as_deref());
                            return Ok(());
                        }
                        Ok(ReplAction::Continue) => {}
                        Err(err) => eprintln!("error: {err}"),
                    }
                    continue;
                }

                if input_is_incomplete(&pending) {
                    continue;
                }

                let source = pending.clone();
                pending.clear();
                match state.eval_source(&source) {
                    Ok(()) => println!("{}", format_stack(&state.stack)),
                    Err(err) => eprintln!("error: {err}"),
                }
            }
            Err(ReadlineError::Interrupted) => {
                pending.clear();
                continue;
            }
            Err(ReadlineError::Eof) => {
                save_history(&mut rl, history.as_deref());
                return Ok(());
            }
            Err(err) => {
                save_history(&mut rl, history.as_deref());
                return Err(Box::new(err));
            }
        }
    }
}

#[derive(Clone)]
struct ReplState {
    evaluator: Evaluator<Value>,
    stack: Vec<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplAction {
    Continue,
    Quit,
}

impl ReplState {
    fn new() -> Self {
        Self {
            evaluator: new_evaluator(),
            stack: Vec::new(),
        }
    }

    fn eval_source(&mut self, source: &str) -> Result<()> {
        let mut candidate = self.clone();
        candidate.eval_source_in_place(source)?;
        *self = candidate;
        Ok(())
    }

    fn eval_source_in_place(&mut self, source: &str) -> Result<()> {
        let spanned = parse_with_spans(source)?;
        self.evaluator.load_with_spans(&spanned)?;
        let tokens = repl_runtime_tokens(&spanned);
        self.evaluator.eval_with_stack(&tokens, &mut self.stack)?;
        Ok(())
    }

    fn type_source(&self, source: &str) -> Result<String> {
        let mut candidate = self.evaluator.clone();
        let spanned = parse_with_spans(source)?;
        candidate.load_with_spans(&spanned)?;
        let tokens = repl_runtime_tokens(&spanned);
        let word = infer_quote_type(&candidate, &tokens)?;
        Ok(format_word_type(&word))
    }

    fn check_source(&self, source: Option<&str>) -> Result<()> {
        let mut candidate = self.evaluator.clone();
        if let Some(source) = source {
            let spanned = parse_with_spans(source)?;
            candidate.load_with_spans(&spanned)?;
        }
        check_whole_program(&candidate, SmtLibSolver::new)?;
        Ok(())
    }

    fn load_file(&mut self, path: &Path) -> Result<()> {
        let source = std::fs::read_to_string(path)?;
        self.eval_source(&source)
    }

    fn definition_lines(&self) -> Vec<String> {
        let mut names: Vec<&str> = self.evaluator.definition_names().collect();
        names.sort_unstable();
        names
            .into_iter()
            .map(|name| {
                let body = self.evaluator.definition_body(name).unwrap_or(&[]);
                format!("{name} = [{}]", format_tokens(body))
            })
            .collect()
    }

    fn operator_lines(&self) -> Vec<String> {
        let mut ops: Vec<(String, String)> = CORE_OPERATOR_NAMES
            .iter()
            .filter_map(|name| {
                core_scheme(name).map(|scheme| (name.to_string(), format_word_type(&scheme.ty)))
            })
            .collect();
        ops.extend(self.evaluator.contract_names().filter_map(|name| {
            self.evaluator
                .contract(name)
                .map(|scheme| (name.to_string(), format_word_type(&scheme.ty)))
        }));
        ops.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        ops.dedup_by(|left, right| left.0 == right.0);
        ops.into_iter()
            .map(|(name, ty)| format!("{name} : {ty}"))
            .collect()
    }

    fn handle_meta_command(&mut self, line: &str) -> Result<ReplAction> {
        let (command, args) = split_meta_command(line)?;
        match command {
            "help" => {
                print_help();
                Ok(ReplAction::Continue)
            }
            "q" | "quit" => {
                expect_empty_meta_args(command, args)?;
                Ok(ReplAction::Quit)
            }
            "stack" => {
                expect_empty_meta_args(command, args)?;
                println!("{}", format_stack(&self.stack));
                Ok(ReplAction::Continue)
            }
            "clear" => {
                expect_empty_meta_args(command, args)?;
                self.stack.clear();
                println!("{}", format_stack(&self.stack));
                Ok(ReplAction::Continue)
            }
            "reset" => {
                expect_empty_meta_args(command, args)?;
                *self = ReplState::new();
                println!("reset");
                Ok(ReplAction::Continue)
            }
            "defs" => {
                expect_empty_meta_args(command, args)?;
                print_lines_or_empty(self.definition_lines(), "no definitions");
                Ok(ReplAction::Continue)
            }
            "ops" => {
                expect_empty_meta_args(command, args)?;
                print_lines_or_empty(self.operator_lines(), "no operators");
                Ok(ReplAction::Continue)
            }
            "type" => {
                if args.trim().is_empty() {
                    return Err(cli_error("usage: :type <program>"));
                }
                println!("{}", self.type_source(args)?);
                Ok(ReplAction::Continue)
            }
            "check" => {
                let source = (!args.trim().is_empty()).then_some(args);
                self.check_source(source)?;
                println!("ok");
                Ok(ReplAction::Continue)
            }
            "load" => {
                let path = parse_load_path(args)?;
                self.load_file(Path::new(&path))?;
                println!("{}", format_stack(&self.stack));
                Ok(ReplAction::Continue)
            }
            _ => Err(cli_error(format!("unknown REPL command `:{command}`"))),
        }
    }
}

fn append_line(buffer: &mut String, line: &str) {
    if !buffer.is_empty() {
        buffer.push('\n');
    }
    buffer.push_str(line);
}

fn input_is_incomplete(source: &str) -> bool {
    match parse_with_spans(source) {
        Ok(_) => false,
        Err(err) => parse_error_is_incomplete(&err),
    }
}

fn parse_error_is_incomplete(err: &ParseError) -> bool {
    match err {
        ParseError::UnmatchedOpenBracket { .. } => true,
        ParseError::Tokenization { message } => {
            message.contains("unclosed single quotes") || message.contains("unclosed double quotes")
        }
        ParseError::UnmatchedCloseBracket { .. } => false,
    }
}

fn history_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".caternary_history"))
}

fn save_history(rl: &mut Editor<(), FileHistory>, history: Option<&Path>) {
    if let Some(history) = history
        && let Err(err) = rl.save_history(history)
    {
        eprintln!("warning: could not save history: {err}");
    }
}

fn split_meta_command(line: &str) -> Result<(&str, &str)> {
    let Some(rest) = line.trim().strip_prefix(':') else {
        return Err(cli_error("REPL commands must start with `:`"));
    };
    let rest = rest.trim_start();
    if rest.is_empty() {
        return Err(cli_error("empty REPL command"));
    }
    let split_at = rest
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(rest.len());
    let (command, args) = rest.split_at(split_at);
    Ok((command, args.trim_start()))
}

fn expect_empty_meta_args(command: &str, args: &str) -> Result<()> {
    if args.trim().is_empty() {
        Ok(())
    } else {
        Err(cli_error(format!(":{command} takes no arguments")))
    }
}

fn parse_load_path(args: &str) -> Result<String> {
    let words = shvar::split(args)?;
    match words.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(cli_error("usage: :load <path>")),
        _ => Err(cli_error(":load takes exactly one path")),
    }
}

fn print_lines_or_empty(lines: Vec<String>, empty: &str) {
    if lines.is_empty() {
        println!("{empty}");
    } else {
        for line in lines {
            println!("{line}");
        }
    }
}

fn print_help() {
    println!(":help          show REPL commands");
    println!(":quit, :q      exit");
    println!(":stack         print the stack");
    println!(":clear         clear the stack");
    println!(":reset         clear stack and definitions");
    println!(":defs          list loaded definitions");
    println!(":ops           list typed operators");
    println!(":type <prog>   infer a stack effect");
    println!(":check [prog]  run the whole-program checker");
    println!(":load <path>   load and evaluate a source file");
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(CliError(message.into()))
}

fn new_evaluator() -> Evaluator<Value> {
    let mut evaluator = Evaluator::new();
    register_all_builtins(&mut evaluator);
    register_repl_builtins(&mut evaluator);
    evaluator
}

fn register_repl_builtins(evaluator: &mut Evaluator<Value>) {
    evaluator.define("TYPEOF", repl_typeof);
    evaluator.register_operator_with_contract("TYPEOF", typeof_scheme());
}

fn typeof_scheme() -> Scheme {
    let s = Span { start: 0, end: 0 };
    Scheme::new(
        vec![0],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::var(0, s)], 0, s),
            StackTy::new(vec![Ty::var(0, s)], 0, s),
        ),
    )
}

fn repl_typeof(
    stack: &mut Vec<Value>,
    evaluator: &Evaluator<Value>,
) -> std::result::Result<(), EvalError> {
    let value = stack
        .last()
        .ok_or_else(|| repl_operator_error("stack underflow: need at least 1 values, found 0"))?;
    println!("{}", typeof_value(evaluator, value)?);
    Ok(())
}

fn typeof_value(
    evaluator: &Evaluator<Value>,
    value: &Value,
) -> std::result::Result<String, EvalError> {
    match value {
        Value::Quotation(tokens) => infer_quote_type(evaluator, tokens)
            .map(|word| format_word_type(&word))
            .map_err(|err| repl_operator_error(err.to_string())),
        _ => Err(repl_operator_error(
            "TYPEOF expects a quotation on top of the stack",
        )),
    }
}

fn repl_operator_error(message: impl AsRef<str>) -> EvalError {
    SError::new("caternary-eval")
        .with_code(CODE_OPERATOR_ERROR)
        .with_message(message.as_ref())
}

fn expect_no_positionals(command: &str, free: &[String]) -> Result<()> {
    if free.is_empty() {
        Ok(())
    } else {
        Err(Box::new(CliError(format!(
            "`caternary {command}` takes no positional arguments"
        ))))
    }
}

fn repl_runtime_tokens(tokens: &[SpannedToken]) -> Vec<Token> {
    let mut skip = vec![false; tokens.len()];
    for i in 1..tokens.len() {
        let SpannedTokenKind::Word(word) = &tokens[i].kind else {
            continue;
        };
        if (is_definition_binder(word) || is_annotation_binder(word))
            && matches!(tokens[i - 1].kind, SpannedTokenKind::Bracket(_))
        {
            skip[i - 1] = true;
            skip[i] = true;
        }
    }
    tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip[*i])
        .map(|(_, token)| token.to_token())
        .collect()
}

fn is_definition_binder(word: &str) -> bool {
    word.strip_prefix(':').is_some_and(is_name)
}

fn is_annotation_binder(word: &str) -> bool {
    word.strip_prefix('@').is_some_and(is_name)
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn format_stack(stack: &[Value]) -> String {
    let values = stack.iter().map(format_value).collect::<Vec<_>>();
    format!("[{}]", values.join(" "))
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Word(w) => format_word(w),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Quotation(tokens) => format!("[{}]", format_tokens(tokens)),
    }
}

fn format_word(word: &str) -> String {
    if word.is_empty() {
        "\"\"".to_string()
    } else {
        shvar::quote_string(word)
    }
}

fn format_tokens(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|token| match token {
            Token::Word(w) => format_word(w),
            Token::Bracket(inner) => format!("[{}]", format_tokens(inner)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_quotes_whitespace_words() {
        let tokens = vec![
            Token::Word("hello world".to_string()),
            Token::Bracket(vec![Token::Word("two words".to_string())]),
        ];

        let formatted = format_tokens(&tokens);

        assert_eq!(parse_with_spans(&formatted).unwrap().len(), 2);
        assert_eq!(
            caternary::parse(&formatted).unwrap(),
            vec![
                Token::Word("hello world".to_string()),
                Token::Bracket(vec![Token::Word("two words".to_string())]),
            ],
        );
    }

    #[test]
    fn format_word_quotes_empty_words() {
        assert_eq!("\"\"", format_word(""));
        assert_eq!(
            caternary::parse(&format_word("")).unwrap(),
            vec![Token::Word(String::new())],
        );
    }

    #[test]
    fn typeof_renders_inferred_quote_type() {
        let evaluator = new_evaluator();
        let value = Value::Quotation(caternary::parse("1 +").unwrap());

        let rendered = typeof_value(&evaluator, &value).unwrap();

        assert_eq!("( 'S Num -- 'S Num )", rendered);
    }

    #[test]
    fn repl_typeof_preserves_the_stack() {
        let mut state = ReplState::new();

        state.eval_source("[ 1 + ] TYPEOF").unwrap();

        assert_eq!(format_stack(&state.stack), "[[1 +]]");
    }

    #[test]
    fn typeof_resolves_repl_definitions() {
        let mut state = ReplState::new();
        state.eval_source("[ 1 + ] :inc").unwrap();
        state.eval_source("[ inc ]").unwrap();

        let rendered = typeof_value(&state.evaluator, state.stack.last().unwrap()).unwrap();

        assert_eq!("( 'S Num -- 'S Num )", rendered);
    }

    #[test]
    fn repl_eval_is_transactional_on_error() {
        let mut state = ReplState::new();
        state.eval_source("1 2").unwrap();

        let err = state.eval_source("DROP DROP DROP").unwrap_err();

        assert!(err.to_string().contains("stack underflow"));
        assert_eq!(format_stack(&state.stack), "[1 2]");
    }

    #[test]
    fn type_source_loads_definitions_read_only() {
        let state = ReplState::new();

        let rendered = state.type_source("[ 1 + ] :inc inc").unwrap();

        assert_eq!("( 'S Num -- 'S Num )", rendered);
        assert!(!state.evaluator.has_definition("inc"));
    }

    #[test]
    fn definition_lines_show_sorted_bodies() {
        let mut state = ReplState::new();
        state.eval_source("[ 2 * ] :double [ 1 + ] :inc").unwrap();

        let lines = state.definition_lines();

        assert_eq!(
            vec!["double = [2 *]".to_string(), "inc = [1 +]".to_string()],
            lines
        );
    }

    #[test]
    fn operator_lines_include_core_and_repl_contracts() {
        let state = ReplState::new();
        let lines = state.operator_lines();

        assert!(lines.iter().any(|line| line.starts_with("DUP : ")));
        assert!(lines.iter().any(|line| line.starts_with("TYPEOF : ")));
    }

    #[test]
    fn unmatched_open_bracket_is_incomplete_input() {
        assert!(input_is_incomplete("[ 1 2"));
        assert!(!input_is_incomplete("[ 1 2 ]"));
        assert!(!input_is_incomplete("]"));
    }
}
