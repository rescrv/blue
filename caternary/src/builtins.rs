//! Standard stack-manipulation builtins.

// Operators must match the `Operator<T>` type signature which requires `&mut Vec<T>`.
#![allow(clippy::ptr_arg)]

use crate::EvalError;
use crate::Evaluator;
use crate::Quotable;
use crate::Scheme;
use crate::Span;
use crate::StackTy;
use crate::Token;
use crate::Ty;
use crate::WordTy;
use crate::evaluator::operator_error;

fn stack_underflow(expected: usize, found: usize) -> EvalError {
    operator_error(format!(
        "stack underflow: need at least {expected} values, found {found}"
    ))
}

fn require_len<T>(stack: &[T], expected: usize) -> Result<(), EvalError> {
    if stack.len() < expected {
        return Err(stack_underflow(expected, stack.len()));
    }
    Ok(())
}

fn dup<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let top = stack.last().unwrap().clone();
    stack.push(top);
    Ok(())
}

fn drop<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    stack.pop();
    Ok(())
}

fn swap<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    stack.swap(len - 2, len - 1);
    Ok(())
}

fn over<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let second = stack[len - 2].clone();
    stack.push(second);
    Ok(())
}

fn rot<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let len = stack.len();
    stack[len - 3..].rotate_left(1);
    Ok(())
}

fn nip<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    stack.remove(len - 2);
    Ok(())
}

fn tuck<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let top = stack[len - 1].clone();
    stack.insert(len - 2, top);
    Ok(())
}

fn two_dup<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let a = stack[len - 2].clone();
    let b = stack[len - 1].clone();
    stack.push(a);
    stack.push(b);
    Ok(())
}

fn two_drop<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    stack.pop();
    stack.pop();
    Ok(())
}

fn two_swap<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let len = stack.len();
    stack[len - 4..].rotate_left(2);
    Ok(())
}

fn two_over<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let len = stack.len();
    let a = stack[len - 4].clone();
    let b = stack[len - 3].clone();
    stack.push(a);
    stack.push(b);
    Ok(())
}

fn scalar_word<T: Quotable>(value: &T) -> Result<String, EvalError> {
    match value.to_tokens().as_slice() {
        [Token::Word(w)] => Ok(w.clone()),
        [Token::Bracket(_)] => Err(operator_error("expected a scalar word, found quotation")),
        _ => Err(operator_error(
            "expected a scalar value that renders as one word",
        )),
    }
}

fn pop_num<T: Quotable>(stack: &mut Vec<T>) -> Result<f64, EvalError> {
    let value = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
    let word = scalar_word(&value)?;
    word.parse::<f64>()
        .map_err(|_| operator_error(format!("expected numeric value, found `{word}`")))
}

fn pop_int<T: Quotable>(stack: &mut Vec<T>) -> Result<i128, EvalError> {
    let value = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
    let word = scalar_word(&value)?;
    word.parse::<i128>()
        .map_err(|_| operator_error(format!("expected integer value, found `{word}`")))
}

fn push_word<T: From<Token>>(stack: &mut Vec<T>, word: impl Into<String>) {
    stack.push(T::from(Token::Word(word.into())));
}

fn push_num<T: From<Token>>(stack: &mut Vec<T>, n: f64) {
    push_word(stack, n.to_string());
}

fn push_int<T: From<Token>>(stack: &mut Vec<T>, n: i128) {
    push_word(stack, n.to_string());
}

fn numeric_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(f64, f64) -> Result<f64, EvalError>,
{
    require_len(stack, 2)?;
    let b = pop_num(stack)?;
    let a = pop_num(stack)?;
    let c = f(a, b)?;
    push_num(stack, c);
    Ok(())
}

fn integer_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(i128, i128) -> Result<i128, EvalError>,
{
    require_len(stack, 2)?;
    let b = pop_int(stack)?;
    let a = pop_int(stack)?;
    let c = f(a, b)?;
    push_int(stack, c);
    Ok(())
}

fn bool_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(bool, bool) -> bool,
{
    require_len(stack, 2)?;
    let b = stack.pop().unwrap().is_truthy();
    let a = stack.pop().unwrap().is_truthy();
    push_word(stack, f(a, b).to_string());
    Ok(())
}

fn plus<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| Ok(a + b))
}

fn minus<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| Ok(a - b))
}

fn multiply<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| Ok(a * b))
}

fn divide<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| {
        if b == 0.0 {
            Err(operator_error("division by zero"))
        } else {
            Ok(a / b)
        }
    })
}

fn modulo<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| {
        if b == 0.0 {
            Err(operator_error("modulo by zero"))
        } else {
            Ok(a % b)
        }
    })
}

fn bit_or<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a | b))
}

fn bit_and<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a & b))
}

fn bit_xor<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a ^ b))
}

fn bit_not<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let a = pop_int(stack)?;
    push_int(stack, !a);
    Ok(())
}

fn shift_amount(n: i128) -> Result<u32, EvalError> {
    u32::try_from(n).map_err(|_| operator_error(format!("invalid shift amount `{n}`")))
}

fn shift_left<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| {
        a.checked_shl(shift_amount(b)?)
            .ok_or_else(|| operator_error(format!("invalid shift amount `{b}`")))
    })
}

fn shift_right<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| {
        a.checked_shr(shift_amount(b)?)
            .ok_or_else(|| operator_error(format!("invalid shift amount `{b}`")))
    })
}

fn bool_or<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    bool_bin(stack, |a, b| a || b)
}

fn bool_and<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    bool_bin(stack, |a, b| a && b)
}

fn bool_not<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let a = stack.pop().unwrap().is_truthy();
    push_word(stack, (!a).to_string());
    Ok(())
}

fn eq<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let b = stack.pop().unwrap();
    let a = stack.pop().unwrap();
    push_word(stack, (a.to_tokens() == b.to_tokens()).to_string());
    Ok(())
}

fn ne<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let b = stack.pop().unwrap();
    let a = stack.pop().unwrap();
    push_word(stack, (a.to_tokens() != b.to_tokens()).to_string());
    Ok(())
}

fn numeric_cmp<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(f64, f64) -> bool,
{
    require_len(stack, 2)?;
    let b = pop_num(stack)?;
    let a = pop_num(stack)?;
    push_word(stack, f(a, b).to_string());
    Ok(())
}

fn lt<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, |a, b| a < b)
}

fn le<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, |a, b| a <= b)
}

fn gt<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, |a, b| a > b)
}

fn ge<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, |a, b| a >= b)
}

fn span() -> Span {
    Span { start: 0, end: 0 }
}

fn num_num_num_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::num(s)], 0, s),
        ),
    )
}

fn num_num_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn bool_bool_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::bool(s), Ty::bool(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn bool_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::bool(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn same_same_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![0],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::var(0, s), Ty::var(0, s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn num_num_num_refinements() -> [(&'static str, &'static str); 4] {
    [
        ("+", "+ : ( a: Num b: Num -- c: Num where c = a + b )"),
        ("-", "- : ( a: Num b: Num -- c: Num where c = a - b )"),
        ("*", "* : ( a: Num b: Num -- c: Num where c = a * b )"),
        (
            "/",
            "/ : ( a: Num b: Num where b * b > 0 -- c: Num where c = a / b )",
        ),
    ]
}

/// Register standard stack combinators/manipulators on an evaluator.
pub fn register_stack_builtins<T>(evaluator: &mut Evaluator<T>)
where
    T: From<Token> + Clone,
{
    evaluator.define("DUP", dup::<T>);
    evaluator.define("DROP", drop::<T>);
    evaluator.define("SWAP", swap::<T>);
    evaluator.define("OVER", over::<T>);
    evaluator.define("ROT", rot::<T>);
    evaluator.define("NIP", nip::<T>);
    evaluator.define("TUCK", tuck::<T>);
    evaluator.define("2DUP", two_dup::<T>);
    evaluator.define("2DROP", two_drop::<T>);
    evaluator.define("2SWAP", two_swap::<T>);
    evaluator.define("2OVER", two_over::<T>);
}

/// Register scalar arithmetic, comparison, boolean, and integer bitwise builtins.
///
/// Arithmetic operators use the language's single `Num` type. Bitwise operators
/// accept integer-valued `Num` lexemes at runtime and reject fractional values.
pub fn register_scalar_builtins<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    evaluator.define("+", plus::<T>);
    evaluator.define("-", minus::<T>);
    evaluator.define("*", multiply::<T>);
    evaluator.define("/", divide::<T>);
    evaluator.define("%", modulo::<T>);
    evaluator.define("|", bit_or::<T>);
    evaluator.define("&", bit_and::<T>);
    evaluator.define("^", bit_xor::<T>);
    evaluator.define("~", bit_not::<T>);
    evaluator.define("<<", shift_left::<T>);
    evaluator.define(">>", shift_right::<T>);
    evaluator.define("||", bool_or::<T>);
    evaluator.define("or", bool_or::<T>);
    evaluator.define("&&", bool_and::<T>);
    evaluator.define("and", bool_and::<T>);
    evaluator.define("!", bool_not::<T>);
    evaluator.define("not", bool_not::<T>);
    evaluator.define("=", eq::<T>);
    evaluator.define("==", eq::<T>);
    evaluator.define("!=", ne::<T>);
    evaluator.define("<", lt::<T>);
    evaluator.define("<=", le::<T>);
    evaluator.define(">", gt::<T>);
    evaluator.define(">=", ge::<T>);

    for op in ["+", "-", "*", "/", "%", "|", "&", "^", "<<", ">>"] {
        evaluator.register_operator_with_contract(op, num_num_num_scheme());
    }
    evaluator.register_operator_with_contract("~", {
        let s = span();
        Scheme::new(
            vec![],
            vec![0],
            WordTy::new(
                StackTy::new(vec![Ty::num(s)], 0, s),
                StackTy::new(vec![Ty::num(s)], 0, s),
            ),
        )
    });
    for op in ["||", "or", "&&", "and"] {
        evaluator.register_operator_with_contract(op, bool_bool_bool_scheme());
    }
    for op in ["!", "not"] {
        evaluator.register_operator_with_contract(op, bool_bool_scheme());
    }
    for op in ["=", "==", "!="] {
        evaluator.register_operator_with_contract(op, same_same_bool_scheme());
    }
    for op in ["<", "<=", ">", ">="] {
        evaluator.register_operator_with_contract(op, num_num_bool_scheme());
    }
    for (_, refinement) in num_num_num_refinements() {
        evaluator
            .attach_refinement(refinement)
            .expect("builtin arithmetic refinement must parse");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Evaluator, Quotable, Scheme, Span, StackTy, Token, Ty, WordTy, check_whole_program, parse,
        parse_with_spans,
    };

    use super::{register_scalar_builtins, register_stack_builtins};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Number(i32);

    impl From<Token> for Number {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => Number(w.parse().unwrap_or(0)),
                Token::Bracket(_) => Number(0),
            }
        }
    }

    impl Quotable for Number {
        fn as_quotation(&self) -> Option<&[Token]> {
            None
        }

        fn to_tokens(&self) -> Vec<Token> {
            vec![Token::Word(self.0.to_string())]
        }

        fn is_truthy(&self) -> bool {
            self.0 != 0
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            None
        }

        fn from_sequence(_elements: Vec<Self>) -> Self {
            Number(0)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
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
            Value::Quotation(elements.iter().flat_map(|v| v.to_tokens()).collect())
        }
    }

    #[test]
    fn registers_and_runs_builtins() {
        let mut eval: Evaluator<Number> = Evaluator::new();
        register_stack_builtins(&mut eval);

        let tokens = parse("1 2 DUP SWAP OVER ROT 2DUP 2DROP").unwrap();
        let stack = eval.eval(&tokens).unwrap();

        assert_eq!(stack, vec![Number(1), Number(2), Number(2), Number(2)]);
    }

    #[test]
    fn reports_underflow() {
        let mut eval: Evaluator<Number> = Evaluator::new();
        register_stack_builtins(&mut eval);

        let tokens = parse("DROP").unwrap();
        let err = eval.eval(&tokens).unwrap_err();

        assert!(
            err.to_string()
                .contains("stack underflow: need at least 1 values, found 0")
        );
    }

    #[test]
    fn scalar_builtins_run_common_operations() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);

        let tokens = parse(
            "false true || false true && 0 2 | 1 3 & 1 3 ^ \
             2 3 + 2 3 * 2 3 - 2 4 /",
        )
        .unwrap();
        let stack = eval.eval(&tokens).unwrap();

        assert_eq!(
            stack,
            vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Number(2.0),
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(5.0),
                Value::Number(6.0),
                Value::Number(-1.0),
                Value::Number(0.5),
            ]
        );
    }

    #[test]
    fn scalar_builtins_register_type_contracts() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let tokens =
            parse_with_spans("[ 2 3 + DROP 2 4 / DROP false true || DROP 1 3 ^ DROP ] :main")
                .unwrap();
        eval.load_with_spans(&tokens).unwrap();

        check_whole_program(&eval, crate::SmtLibSolver::new).unwrap();
    }

    #[test]
    fn arithmetic_refinement_publishes_exact_sum() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let s = Span { start: 0, end: 0 };
        eval.register_operator_with_contract(
            "need5",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(StackTy::new(vec![Ty::num(s)], 0, s), StackTy::empty(0, s)),
            ),
        );
        eval.attach_refinement("need5 : ( n: Num where n >= 5 -- )")
            .unwrap();
        let tokens = parse_with_spans("[ 2 3 + need5 ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();

        check_whole_program(&eval, crate::SmtLibSolver::new).unwrap();
    }
}
