use std::error::Error;
use std::fmt;
use std::io::Read;

use arrrg::CommandLine;
use caternary::{
    CODE_OPERATOR_ERROR, EvalError, Evaluator, Quotable, Scheme, SmtLibSolver, Span, SpannedToken,
    SpannedTokenKind, StackTy, Token, Ty, WordTy, check_whole_program, format_word_type,
    infer_quote_type, parse_with_spans, register_all_builtins,
};
use handled::SError;
use rustyline::error::ReadlineError;
use rustyline::history::MemHistory;
use rustyline::{Config, Editor};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {}

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct CheckOptions {}

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct ShellOptions {}

enum Command {
    Check(CheckOptions, Vec<String>),
    Shell(ShellOptions, Vec<String>),
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
        "Usage: caternary [OPTIONS] <check|shell> [COMMAND OPTIONS]",
    );
    let command = arrrg::dispatch_subcommands!(free, {
        "check" => CheckOptions as check, check_free => {
            Ok(Command::Check(check, check_free))
        },
        "shell" => ShellOptions as shell, shell_free => {
            Ok(Command::Shell(shell, shell_free))
        },
    })?;
    match command {
        Command::Check(options, free) => check_command(options, free),
        Command::Shell(options, free) => shell_command(options, free),
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

fn shell_command(_options: ShellOptions, free: Vec<String>) -> Result<()> {
    expect_no_positionals("shell", &free)?;

    let config = Config::builder()
        .max_history_size(1_000_000)?
        .history_ignore_dups(true)?
        .history_ignore_space(true)
        .build();
    let hist = MemHistory::new();
    let mut rl: Editor<(), MemHistory> = Editor::with_history(config, hist)?;
    let mut evaluator = new_evaluator();
    let mut stack = Vec::new();

    loop {
        match rl.readline("caternary> ") {
            Ok(line) => {
                rl.add_history_entry(&line)?;
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    return Ok(());
                }
                match shell_eval_line(&mut evaluator, &mut stack, line) {
                    Ok(()) => println!("{}", format_stack(&stack)),
                    Err(err) => eprintln!("error: {err}"),
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => return Ok(()),
            Err(err) => return Err(Box::new(err)),
        }
    }
}

fn shell_eval_line(
    evaluator: &mut Evaluator<Value>,
    stack: &mut Vec<Value>,
    line: &str,
) -> Result<()> {
    let spanned = parse_with_spans(line)?;
    evaluator.load_with_spans(&spanned)?;
    let tokens = shell_runtime_tokens(&spanned);
    evaluator.eval_with_stack(&tokens, stack)?;
    Ok(())
}

fn new_evaluator() -> Evaluator<Value> {
    let mut evaluator = Evaluator::new();
    register_all_builtins(&mut evaluator);
    register_shell_builtins(&mut evaluator);
    evaluator
}

fn register_shell_builtins(evaluator: &mut Evaluator<Value>) {
    evaluator.define("TYPEOF", shell_typeof);
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

fn shell_typeof(
    stack: &mut Vec<Value>,
    evaluator: &Evaluator<Value>,
) -> std::result::Result<(), EvalError> {
    let value = stack
        .last()
        .ok_or_else(|| shell_operator_error("stack underflow: need at least 1 values, found 0"))?;
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
            .map_err(|err| shell_operator_error(err.to_string())),
        _ => Err(shell_operator_error(
            "TYPEOF expects a quotation on top of the stack",
        )),
    }
}

fn shell_operator_error(message: impl AsRef<str>) -> EvalError {
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

fn shell_runtime_tokens(tokens: &[SpannedToken]) -> Vec<Token> {
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
    fn shell_typeof_preserves_the_stack() {
        let mut evaluator = new_evaluator();
        let mut stack = Vec::new();

        shell_eval_line(&mut evaluator, &mut stack, "[ 1 + ] TYPEOF").unwrap();

        assert_eq!(format_stack(&stack), "[[1 +]]");
    }

    #[test]
    fn typeof_resolves_shell_definitions() {
        let mut evaluator = new_evaluator();
        let mut stack = Vec::new();
        shell_eval_line(&mut evaluator, &mut stack, "[ 1 + ] :inc").unwrap();
        shell_eval_line(&mut evaluator, &mut stack, "[ inc ]").unwrap();

        let rendered = typeof_value(&evaluator, stack.last().unwrap()).unwrap();

        assert_eq!("( 'S Num -- 'S Num )", rendered);
    }
}
