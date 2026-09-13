//! Probe: read a program from argv[1] (or stdin), run the whole-program gate
//! and then the evaluator, printing both verdicts side by side.

use std::io::Read;

use caternary::*;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Word(String),
    Int(i128),
    Float(f64),
    Bool(bool),
    Quotation(Vec<QuoteItem<Value>>),
}

impl From<Token> for Value {
    fn from(token: Token) -> Self {
        match token {
            Token::Word(w) => {
                if let Ok(n) = w.parse::<i128>() {
                    Value::Int(n)
                } else if is_integer_literal(&w) {
                    // Out-of-range integer: preserve the exact lexeme (the
                    // runtime rejects it loudly) — never round through f64.
                    Value::Word(w)
                } else if let Ok(f) = w.parse::<f64>() {
                    if f.is_finite() {
                        Value::Float(f)
                    } else {
                        Value::Word(w)
                    }
                } else if w == "true" {
                    Value::Bool(true)
                } else if w == "false" {
                    Value::Bool(false)
                } else {
                    Value::Word(w)
                }
            }
            Token::Bracket(tokens) => Value::Quotation(quote_items_from_tokens(&tokens)),
        }
    }
}

impl Quotable for Value {
    fn as_quotation(&self) -> Option<&[QuoteItem<Value>]> {
        match self {
            Value::Quotation(tokens) => Some(tokens),
            _ => None,
        }
    }
    fn from_quotation(items: Vec<QuoteItem<Self>>) -> Self {
        Value::Quotation(items)
    }
    fn to_tokens(&self) -> Vec<Token> {
        match self {
            Value::Word(w) => vec![Token::Word(w.clone())],
            Value::Int(n) => vec![Token::Word(n.to_string())],
            Value::Float(f) => vec![Token::Word(f.to_string())],
            Value::Bool(b) => vec![Token::Word(b.to_string())],
            Value::Quotation(items) => vec![Token::Bracket(quote_items_to_tokens(items))],
        }
    }
    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            _ => true,
        }
    }
    fn as_sequence(&self) -> Option<Vec<Self>> {
        match self {
            Value::Quotation(items) => Some(quote_items_to_values(items)),
            _ => None,
        }
    }
    fn from_sequence(elements: Vec<Self>) -> Self {
        Value::Quotation(elements.into_iter().map(QuoteItem::Push).collect())
    }
}

fn fmt(v: &Value) -> String {
    match v {
        Value::Word(w) => format!("W({w})"),
        Value::Int(n) => format!("I({n})"),
        Value::Float(f) => format!("F({f})"),
        Value::Bool(b) => format!("B({b})"),
        Value::Quotation(items) => {
            let parts: Vec<String> = quote_items_to_tokens(items)
                .iter()
                .map(|t| match t {
                    Token::Word(w) => w.clone(),
                    Token::Bracket(_) => "[..]".to_string(),
                })
                .collect();
            format!("Q[{}]", parts.join(" "))
        }
    }
}

fn main() {
    let source = if let Some(src) = std::env::args().nth(1) {
        src
    } else {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).unwrap();
        s
    };

    // Gate.
    let mut evaluator = Evaluator::new();
    register_all_builtins(&mut evaluator);
    match parse_with_spans(&source) {
        Ok(tokens) => {
            if let Err(e) = evaluator.load_with_spans(&tokens) {
                println!("load: ERR {e}");
                return;
            }
            match check_whole_program(&evaluator, SmtLibSolver::new) {
                Ok(_) => println!("gate: OK"),
                Err(e) => println!("gate: ERR {e}"),
            }
            match evaluator.definition_body("main") {
                Some(body) => match infer_quote_type(&evaluator, body) {
                    Ok(w) => println!("type(main): {}", format_word_type(&w)),
                    Err(e) => println!("type(main): ERR {e}"),
                },
                None => {
                    let toks: Vec<Token> = tokens.iter().map(SpannedToken::to_token).collect();
                    match infer_quote_type(&evaluator, &toks) {
                        Ok(w) => println!("type: {}", format_word_type(&w)),
                        Err(e) => println!("type: ERR {e}"),
                    }
                }
            }
        }
        Err(e) => {
            println!("parse: ERR {e}");
            return;
        }
    }

    // Runtime: evaluate `main` like the gate assumes.
    let evaluator2 = evaluator.clone();
    if let Some(body) = evaluator2.definition_body("main") {
        let body = body.to_vec();
        match evaluator2.eval(&body) {
            Ok(stack) => {
                let rendered: Vec<String> = stack.iter().map(fmt).collect();
                println!("eval: OK [{}]", rendered.join(" "));
            }
            Err(e) => println!("eval: ERR {e}"),
        }
    } else {
        let tokens = parse(&source).unwrap();
        match evaluator2.eval(&tokens) {
            Ok(stack) => {
                let rendered: Vec<String> = stack.iter().map(fmt).collect();
                println!("eval: OK [{}]", rendered.join(" "));
            }
            Err(e) => println!("eval: ERR {e}"),
        }
    }
}
