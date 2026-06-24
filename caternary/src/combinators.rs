//! Quotation combinators for caternary.
//!
//! Combinators are operators that execute quotations (bracketed code) from the stack.
//! They enable higher-order programming patterns like `CALL`, `DIP`, `BI`, etc.
//!
//! To use combinators, your stack element type `T` must implement `Quotable`, which
//! allows extracting the underlying tokens from a quotation value.

use crate::EvalError;
use crate::Evaluator;
use crate::Token;
use crate::evaluator::operator_error;

/// A trait for stack element types that can contain quotations.
///
/// Quotations are bracketed code that can be executed by combinators. This trait
/// allows combinators to extract the tokens from a quotation value on the stack.
pub trait Quotable: From<Token> + Clone {
    /// Attempts to extract the tokens from a quotation value.
    ///
    /// Returns `Some(tokens)` if this value is a quotation (bracket), or `None` otherwise.
    fn as_quotation(&self) -> Option<&[Token]>;

    /// Converts this value back to tokens for use in quotation construction.
    ///
    /// Used by `CURRY` to embed a value into a quotation. Returns a vector of
    /// tokens that, when evaluated, would produce this value on the stack.
    fn to_tokens(&self) -> Vec<Token>;

    /// Returns true if this value represents a truthy condition.
    ///
    /// Used by conditional combinators like `IF`, `WHEN`, `UNLESS`.
    /// The default implementation returns `true` for all values.
    fn is_truthy(&self) -> bool {
        true
    }

    /// Attempts to extract this value as a sequence of elements.
    ///
    /// Returns `Some(elements)` if this value is a sequence, or `None` otherwise.
    /// Used by sequence combinators like `MAP`, `FILTER`, `FOLD`, `EACH`.
    fn as_sequence(&self) -> Option<Vec<Self>>;

    /// Creates a sequence value from a vector of elements.
    ///
    /// Used by sequence combinators to construct result sequences.
    fn from_sequence(elements: Vec<Self>) -> Self;
}

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

fn not_a_quotation() -> EvalError {
    operator_error("expected a quotation (bracketed code)")
}

fn not_a_sequence() -> EvalError {
    operator_error("expected a sequence")
}

/// Pops a quotation from the stack, returning its tokens.
fn pop_quotation<T: Quotable>(stack: &mut Vec<T>) -> Result<Vec<Token>, EvalError> {
    let val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    val.as_quotation()
        .map(|tokens| tokens.to_vec())
        .ok_or_else(not_a_quotation)
}

// ============================================================================
// Core Combinators
// ============================================================================

/// `CALL`: Execute a quotation.
///
/// Stack effect: `( [Q] -- ... )`
///
/// Pops a quotation from the stack and executes it.
fn call<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    let quotation = pop_quotation(stack)?;
    eval.eval_with_stack(&quotation, stack)
}

/// `DIP`: Execute a quotation while hiding the top element.
///
/// Stack effect: `( x [Q] -- ... x )`
///
/// Pops a quotation and the element below it, executes the quotation,
/// then pushes the hidden element back.
fn dip<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let hidden = stack.pop().unwrap();
    eval.eval_with_stack(&quotation, stack)?;
    stack.push(hidden);
    Ok(())
}

/// `2DIP`: Execute a quotation while hiding the top two elements.
///
/// Stack effect: `( x y [Q] -- ... x y )`
///
/// Pops a quotation and the two elements below it, executes the quotation,
/// then restores the hidden pair in order.
fn two_dip<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let quotation = pop_quotation(stack)?;
    let hidden = stack.split_off(stack.len() - 2);
    eval.eval_with_stack(&quotation, stack)?;
    stack.extend(hidden);
    Ok(())
}

/// `3DIP`: Execute a quotation while hiding the top three elements.
///
/// Stack effect: `( x y z [Q] -- ... x y z )`
///
/// Pops a quotation and the three elements below it, executes the quotation,
/// then restores the hidden triple in order.
fn three_dip<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let quotation = pop_quotation(stack)?;
    let hidden = stack.split_off(stack.len() - 3);
    eval.eval_with_stack(&quotation, stack)?;
    stack.extend(hidden);
    Ok(())
}

/// `KEEP`: Execute a quotation on a value, keeping the original value.
///
/// Stack effect: `( x [Q] -- ... x )`
///
/// Like `DIP` but the quotation receives a copy of x, and x is restored after.
fn keep<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let x = stack.last().unwrap().clone();
    eval.eval_with_stack(&quotation, stack)?;
    stack.push(x);
    Ok(())
}

/// `2KEEP`: Execute a quotation on two values, keeping the original pair.
///
/// Stack effect: `( x y [Q] -- ... x y )`
///
/// Like `KEEP`, but preserves the top two values after the quotation runs.
fn two_keep<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let quotation = pop_quotation(stack)?;
    let kept = stack[stack.len() - 2..].to_vec();
    eval.eval_with_stack(&quotation, stack)?;
    stack.extend(kept);
    Ok(())
}

/// `3KEEP`: Execute a quotation on three values, keeping the original triple.
///
/// Stack effect: `( x y z [Q] -- ... x y z )`
///
/// Like `KEEP`, but preserves the top three values after the quotation runs.
fn three_keep<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let quotation = pop_quotation(stack)?;
    let kept = stack[stack.len() - 3..].to_vec();
    eval.eval_with_stack(&quotation, stack)?;
    stack.extend(kept);
    Ok(())
}

/// `BI`: Apply two quotations to a single value.
///
/// Stack effect: `( x [P] [Q] -- P(x) Q(x) )`
///
/// Pops two quotations and a value, applies each quotation to a copy of the value.
fn bi<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let q = pop_quotation(stack)?;
    let p = pop_quotation(stack)?;
    let x = stack.pop().unwrap();

    stack.push(x.clone());
    eval.eval_with_stack(&p, stack)?;

    stack.push(x);
    eval.eval_with_stack(&q, stack)?;

    Ok(())
}

/// `BI*`: Apply two quotations to two values respectively.
///
/// Stack effect: `( x y [P] [Q] -- P(x) Q(y) )`
///
/// Pops two quotations and two values, applies P to x and Q to y.
fn bi_star<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let q = pop_quotation(stack)?;
    let p = pop_quotation(stack)?;
    let y = stack.pop().unwrap();
    let x = stack.pop().unwrap();

    stack.push(x);
    eval.eval_with_stack(&p, stack)?;

    stack.push(y);
    eval.eval_with_stack(&q, stack)?;

    Ok(())
}

/// `BI@`: Apply one quotation to two values.
///
/// Stack effect: `( x y [Q] -- Q(x) Q(y) )`
///
/// Pops a quotation and two values, applies the quotation to each.
fn bi_at<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let q = pop_quotation(stack)?;
    let y = stack.pop().unwrap();
    let x = stack.pop().unwrap();

    stack.push(x);
    eval.eval_with_stack(&q, stack)?;

    stack.push(y);
    eval.eval_with_stack(&q, stack)?;

    Ok(())
}

/// `TRI`: Apply three quotations to a single value.
///
/// Stack effect: `( x [P] [Q] [R] -- P(x) Q(x) R(x) )`
///
/// Pops three quotations and a value, applies each quotation to a copy of the value.
fn tri<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let r = pop_quotation(stack)?;
    let q = pop_quotation(stack)?;
    let p = pop_quotation(stack)?;
    let x = stack.pop().unwrap();

    stack.push(x.clone());
    eval.eval_with_stack(&p, stack)?;

    stack.push(x.clone());
    eval.eval_with_stack(&q, stack)?;

    stack.push(x);
    eval.eval_with_stack(&r, stack)?;

    Ok(())
}

/// `TRI*`: Apply three quotations to three values respectively.
///
/// Stack effect: `( x y z [P] [Q] [R] -- P(x) Q(y) R(z) )`
///
/// Pops three quotations and three values, applies P to x, Q to y, and R to z.
fn tri_star<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 6)?;
    let r = pop_quotation(stack)?;
    let q = pop_quotation(stack)?;
    let p = pop_quotation(stack)?;
    let z = stack.pop().unwrap();
    let y = stack.pop().unwrap();
    let x = stack.pop().unwrap();

    stack.push(x);
    eval.eval_with_stack(&p, stack)?;

    stack.push(y);
    eval.eval_with_stack(&q, stack)?;

    stack.push(z);
    eval.eval_with_stack(&r, stack)?;

    Ok(())
}

/// `TRI@`: Apply one quotation to three values.
///
/// Stack effect: `( x y z [Q] -- Q(x) Q(y) Q(z) )`
///
/// Pops a quotation and three values, applies the quotation to each.
fn tri_at<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let q = pop_quotation(stack)?;
    let z = stack.pop().unwrap();
    let y = stack.pop().unwrap();
    let x = stack.pop().unwrap();

    stack.push(x);
    eval.eval_with_stack(&q, stack)?;

    stack.push(y);
    eval.eval_with_stack(&q, stack)?;

    stack.push(z);
    eval.eval_with_stack(&q, stack)?;

    Ok(())
}

/// `CLEAVE`: Apply multiple quotations to a single value.
///
/// Stack effect: `( x [[P] [Q] ...] -- P(x) Q(x) ... )`
///
/// Pops a list of quotations and a value, applies each quotation to a copy of the value.
fn cleave<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotations_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let quotations = quotations_val
        .as_quotation()
        .ok_or_else(not_a_quotation)?
        .to_vec();
    let x = stack.pop().unwrap();

    for token in quotations {
        if let Token::Bracket(q) = token {
            stack.push(x.clone());
            eval.eval_with_stack(&q, stack)?;
        } else {
            return Err(operator_error("CLEAVE expects a list of quotations"));
        }
    }

    Ok(())
}

/// `SPREAD`: Apply quotations from a list to corresponding stack values.
///
/// Stack effect: `( x y z [[P] [Q] [R]] -- P(x) Q(y) R(z) )`
///
/// Pops a list of quotations, then pops that many values, applies each quotation
/// to its corresponding value.
fn spread<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let quotations_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let quotations = quotations_val
        .as_quotation()
        .ok_or_else(not_a_quotation)?
        .to_vec();

    let n = quotations.len();
    require_len(stack, n)?;

    let mut values: Vec<T> = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(stack.pop().unwrap());
    }
    values.reverse();

    for (val, token) in values.into_iter().zip(quotations) {
        if let Token::Bracket(q) = token {
            stack.push(val);
            eval.eval_with_stack(&q, stack)?;
        } else {
            return Err(operator_error("SPREAD expects a list of quotations"));
        }
    }

    Ok(())
}

/// `COMPOSE`: Concatenate two quotations.
///
/// Stack effect: `( [P] [Q] -- [P Q] )`
///
/// Creates a new quotation that executes P followed by Q.
fn compose<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let q = pop_quotation(stack)?;
    let p = pop_quotation(stack)?;

    let mut combined = p;
    combined.extend(q);

    stack.push(T::from(Token::Bracket(combined)));
    Ok(())
}

/// `CURRY`: Partially apply a value to a quotation.
///
/// Stack effect: `( x [Q] -- [x Q] )`
///
/// Creates a new quotation that pushes x, then executes Q.
fn curry<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let q = pop_quotation(stack)?;
    let x_val = stack.pop().unwrap();

    let mut combined = x_val.to_tokens();
    combined.extend(q);

    stack.push(T::from(Token::Bracket(combined)));
    Ok(())
}

/// `2CURRY`: Partially apply two values to a quotation.
///
/// Stack effect: `( x y [Q] -- [x y Q] )`
///
/// Creates a new quotation that pushes x and y, then executes Q.
fn two_curry<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let q = pop_quotation(stack)?;
    let values = stack.split_off(stack.len() - 2);

    let mut combined = Vec::new();
    for value in values {
        combined.extend(value.to_tokens());
    }
    combined.extend(q);

    stack.push(T::from(Token::Bracket(combined)));
    Ok(())
}

/// `3CURRY`: Partially apply three values to a quotation.
///
/// Stack effect: `( x y z [Q] -- [x y z Q] )`
///
/// Creates a new quotation that pushes x, y, and z, then executes Q.
fn three_curry<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let q = pop_quotation(stack)?;
    let values = stack.split_off(stack.len() - 3);

    let mut combined = Vec::new();
    for value in values {
        combined.extend(value.to_tokens());
    }
    combined.extend(q);

    stack.push(T::from(Token::Bracket(combined)));
    Ok(())
}

// ============================================================================
// Conditional Combinators
// ============================================================================

/// `IF`: Conditional execution.
///
/// Stack effect: `( ? [T] [F] -- ... )`
///
/// Pops a condition and two quotations. Executes T if condition is truthy, F otherwise.
fn if_combinator<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let false_branch = pop_quotation(stack)?;
    let true_branch = pop_quotation(stack)?;
    let condition = stack.pop().unwrap();

    if condition.is_truthy() {
        eval.eval_with_stack(&true_branch, stack)
    } else {
        eval.eval_with_stack(&false_branch, stack)
    }
}

/// `WHEN`: Execute quotation if condition is truthy.
///
/// Stack effect: `( ? [Q] -- ... )`
///
/// Pops a condition and a quotation. Executes Q only if condition is truthy.
fn when<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let condition = stack.pop().unwrap();

    if condition.is_truthy() {
        eval.eval_with_stack(&quotation, stack)
    } else {
        Ok(())
    }
}

/// `UNLESS`: Execute quotation if condition is falsy.
///
/// Stack effect: `( ? [Q] -- ... )`
///
/// Pops a condition and a quotation. Executes Q only if condition is falsy.
fn unless<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let condition = stack.pop().unwrap();

    if !condition.is_truthy() {
        eval.eval_with_stack(&quotation, stack)
    } else {
        Ok(())
    }
}

// ============================================================================
// Sequence Combinators
// ============================================================================

/// `MAP`: Transform each element of a sequence.
///
/// Stack effect: `( seq [Q] -- seq' )`
///
/// Applies Q to each element of the sequence, collecting results.
fn map<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let seq_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let seq = seq_val.as_sequence().ok_or_else(not_a_sequence)?;

    let mut results = Vec::with_capacity(seq.len());
    for elem in seq {
        let before = stack.len();
        stack.push(elem);
        eval.eval_with_stack(&quotation, stack)?;
        if stack.len() != before + 1 {
            return Err(operator_error(
                "MAP quotation must consume one element and leave one result",
            ));
        }
        let result = stack
            .pop()
            .ok_or_else(|| operator_error("MAP quotation must leave one value on stack"))?;
        results.push(result);
    }

    stack.push(T::from_sequence(results));
    Ok(())
}

/// `FILTER`: Select elements matching a predicate.
///
/// Stack effect: `( seq [pred] -- seq' )`
///
/// Applies pred to each element, keeping only those where pred returns truthy.
fn filter<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let seq_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let seq = seq_val.as_sequence().ok_or_else(not_a_sequence)?;

    let mut results = Vec::new();
    for elem in seq {
        let before = stack.len();
        stack.push(elem.clone());
        eval.eval_with_stack(&quotation, stack)?;
        if stack.len() != before + 1 {
            return Err(operator_error(
                "FILTER quotation must consume one element and leave one predicate value",
            ));
        }
        let predicate_result = stack
            .pop()
            .ok_or_else(|| operator_error("FILTER quotation must leave one value on stack"))?;
        if predicate_result.is_truthy() {
            results.push(elem);
        }
    }

    stack.push(T::from_sequence(results));
    Ok(())
}

/// `FOLD`: Reduce a sequence with an accumulator.
///
/// Stack effect: `( seq init [Q] -- result )`
///
/// Starting with init, applies Q to (accumulator, element) for each element.
/// Q should leave the new accumulator on the stack.
fn fold<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let quotation = pop_quotation(stack)?;
    let init = stack.pop().unwrap();
    let seq_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let seq = seq_val.as_sequence().ok_or_else(not_a_sequence)?;

    stack.push(init);
    for elem in seq {
        let before = stack.len();
        stack.push(elem);
        eval.eval_with_stack(&quotation, stack)?;
        if stack.len() != before {
            return Err(operator_error(
                "FOLD quotation must consume accumulator+element and leave one accumulator",
            ));
        }
    }

    Ok(())
}

/// `EACH`: Execute a quotation for each element (for side effects).
///
/// Stack effect: `( seq [Q] -- )`
///
/// Applies Q to each element, discarding results.
fn each<T: Quotable>(stack: &mut Vec<T>, eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let quotation = pop_quotation(stack)?;
    let seq_val = stack.pop().ok_or_else(|| stack_underflow(1, 0))?;
    let seq = seq_val.as_sequence().ok_or_else(not_a_sequence)?;

    for elem in seq {
        let before = stack.len();
        stack.push(elem);
        eval.eval_with_stack(&quotation, stack)?;
        if stack.len() != before {
            return Err(operator_error(
                "EACH quotation must consume one element and leave no extra values",
            ));
        }
    }

    Ok(())
}

// ============================================================================
// Registration
// ============================================================================

/// Register quotation combinators on an evaluator.
///
/// This registers: `CALL`, `DIP`, `2DIP`, `3DIP`, `KEEP`, `2KEEP`, `3KEEP`,
/// `BI`, `BI*`, `BI@`, `TRI`, `TRI*`, `TRI@`, `CLEAVE`, `SPREAD`, `COMPOSE`,
/// `CURRY`, `2CURRY`, `3CURRY`.
pub fn register_combinators<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    evaluator.define("CALL", call::<T>);
    evaluator.define("DIP", dip::<T>);
    evaluator.define("2DIP", two_dip::<T>);
    evaluator.define("3DIP", three_dip::<T>);
    evaluator.define("KEEP", keep::<T>);
    evaluator.define("2KEEP", two_keep::<T>);
    evaluator.define("3KEEP", three_keep::<T>);
    evaluator.define("BI", bi::<T>);
    evaluator.define("BI*", bi_star::<T>);
    evaluator.define("BI@", bi_at::<T>);
    evaluator.define("TRI", tri::<T>);
    evaluator.define("TRI*", tri_star::<T>);
    evaluator.define("TRI@", tri_at::<T>);
    evaluator.define("CLEAVE", cleave::<T>);
    evaluator.define("SPREAD", spread::<T>);
    evaluator.define("COMPOSE", compose::<T>);
    evaluator.define("CURRY", curry::<T>);
    evaluator.define("2CURRY", two_curry::<T>);
    evaluator.define("3CURRY", three_curry::<T>);
}

/// Register conditional combinators on an evaluator.
///
/// This registers: `IF`, `WHEN`, `UNLESS`.
pub fn register_conditionals<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    evaluator.define("IF", if_combinator::<T>);
    evaluator.define("WHEN", when::<T>);
    evaluator.define("UNLESS", unless::<T>);
}

/// Register sequence combinators on an evaluator.
///
/// This registers: `MAP`, `FILTER`, `FOLD`, `EACH`.
pub fn register_sequence_combinators<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    evaluator.define("MAP", map::<T>);
    evaluator.define("FILTER", filter::<T>);
    evaluator.define("FOLD", fold::<T>);
    evaluator.define("EACH", each::<T>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use crate::register_stack_builtins;

    #[derive(Debug, Clone, PartialEq)]
    enum Value {
        Int(i64),
        Bool(bool),
        Word(String),
        Quotation(Vec<Token>),
        Sequence(Vec<Value>),
    }

    impl From<Token> for Value {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => {
                    if let Ok(n) = w.parse::<i64>() {
                        Value::Int(n)
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
                Value::Int(n) => vec![Token::Word(n.to_string())],
                Value::Bool(b) => vec![Token::Word(b.to_string())],
                Value::Word(w) => vec![Token::Word(w.clone())],
                Value::Quotation(tokens) => vec![Token::Bracket(tokens.clone())],
                Value::Sequence(elems) => {
                    let inner: Vec<Token> = elems.iter().flat_map(|e| e.to_tokens()).collect();
                    vec![Token::Bracket(inner)]
                }
            }
        }

        fn is_truthy(&self) -> bool {
            match self {
                Value::Bool(b) => *b,
                Value::Int(n) => *n != 0,
                _ => true,
            }
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            match self {
                Value::Sequence(elems) => Some(elems.clone()),
                Value::Quotation(tokens) => {
                    Some(tokens.iter().map(|t| Value::from(t.clone())).collect())
                }
                _ => None,
            }
        }

        fn from_sequence(elements: Vec<Self>) -> Self {
            Value::Sequence(elements)
        }
    }

    fn make_eval() -> Evaluator<Value> {
        let mut eval = Evaluator::new();
        register_stack_builtins(&mut eval);
        register_combinators(&mut eval);
        register_conditionals(&mut eval);
        register_sequence_combinators(&mut eval);

        eval.define("ADD", |stack: &mut Vec<Value>, _eval| {
            let b = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            let a = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            match (a, b) {
                (Value::Int(a), Value::Int(b)) => stack.push(Value::Int(a + b)),
                _ => {
                    return Err(operator_error("ADD requires two integers"));
                }
            }
            Ok(())
        });

        eval.define("MUL", |stack: &mut Vec<Value>, _eval| {
            let b = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            let a = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            match (a, b) {
                (Value::Int(a), Value::Int(b)) => stack.push(Value::Int(a * b)),
                _ => {
                    return Err(operator_error("MUL requires two integers"));
                }
            }
            Ok(())
        });

        eval.define("GT", |stack: &mut Vec<Value>, _eval| {
            let b = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            let a = stack.pop().ok_or_else(|| stack_underflow(2, stack.len()))?;
            match (a, b) {
                (Value::Int(a), Value::Int(b)) => stack.push(Value::Bool(a > b)),
                _ => {
                    return Err(operator_error("GT requires two integers"));
                }
            }
            Ok(())
        });

        eval.define("EVEN", |stack: &mut Vec<Value>, _eval| {
            let a = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
            match a {
                Value::Int(n) => stack.push(Value::Bool(n % 2 == 0)),
                _ => {
                    return Err(operator_error("EVEN requires an integer"));
                }
            }
            Ok(())
        });

        eval.define("ECHO", |stack: &mut Vec<Value>, _eval| {
            let value = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
            stack.push(value);
            Ok(())
        });

        eval
    }

    // ========================================================================
    // CALL tests
    // ========================================================================

    #[test]
    fn call_executes_quotation() {
        let eval = make_eval();
        let tokens = parse("1 2 [ADD] CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(3)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn call_nested_quotations() {
        let eval = make_eval();
        let tokens = parse("1 2 [[ADD] CALL] CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(3)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn echo_preserves_shell_quoted_word() {
        let eval = make_eval();
        let tokens = parse(r#""hello world" ECHO"#).unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Word("hello world".to_string())]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn call_executes_compact_quotation_with_shell_quoted_word() {
        let eval = make_eval();
        let tokens = parse(r#"["hello world" ECHO]CALL"#).unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Word("hello world".to_string())]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // DIP tests
    // ========================================================================

    #[test]
    fn dip_hides_top() {
        let eval = make_eval();
        let tokens = parse("1 2 3 [ADD] DIP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(3), Value::Int(3)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn dip_restores_hidden_value() {
        let eval = make_eval();
        let tokens = parse("10 20 [DUP] DIP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(10), Value::Int(10), Value::Int(20)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn two_dip_hides_top_pair() {
        let eval = make_eval();
        let tokens = parse("1 2 3 4 [ADD] 2DIP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(3), Value::Int(3), Value::Int(4)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn three_dip_hides_top_triple() {
        let eval = make_eval();
        let tokens = parse("1 2 3 4 5 [ADD] 3DIP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(
            result,
            vec![Value::Int(3), Value::Int(3), Value::Int(4), Value::Int(5)]
        );
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // KEEP tests
    // ========================================================================

    #[test]
    fn keep_preserves_value() {
        let eval = make_eval();
        let tokens = parse("5 [DUP MUL] KEEP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(25), Value::Int(5)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn two_keep_preserves_pair() {
        let eval = make_eval();
        let tokens = parse("3 4 [ADD] 2KEEP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(7), Value::Int(3), Value::Int(4)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn three_keep_preserves_triple() {
        let eval = make_eval();
        let tokens = parse("2 3 4 [ADD ADD] 3KEEP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(
            result,
            vec![Value::Int(9), Value::Int(2), Value::Int(3), Value::Int(4)]
        );
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // BI tests
    // ========================================================================

    #[test]
    fn bi_applies_two_quotations() {
        let eval = make_eval();
        let tokens = parse("5 [DUP ADD] [DUP MUL] BI").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(10), Value::Int(25)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // BI* tests
    // ========================================================================

    #[test]
    fn bi_star_applies_to_two_values() {
        let eval = make_eval();
        let tokens = parse("3 4 [DUP MUL] [DUP ADD] BI*").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(9), Value::Int(8)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // BI@ tests
    // ========================================================================

    #[test]
    fn bi_at_applies_same_quotation() {
        let eval = make_eval();
        let tokens = parse("3 4 [DUP MUL] BI@").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(9), Value::Int(16)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // TRI tests
    // ========================================================================

    #[test]
    fn tri_applies_three_quotations() {
        let eval = make_eval();
        let tokens = parse("5 [DUP ADD] [DUP MUL] [1 ADD] TRI").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(10), Value::Int(25), Value::Int(6)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // TRI* tests
    // ========================================================================

    #[test]
    fn tri_star_applies_to_three_values() {
        let eval = make_eval();
        let tokens = parse("3 4 5 [DUP MUL] [DUP ADD] [1 ADD] TRI*").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(9), Value::Int(8), Value::Int(6)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // TRI@ tests
    // ========================================================================

    #[test]
    fn tri_at_applies_same_quotation() {
        let eval = make_eval();
        let tokens = parse("3 4 5 [DUP MUL] TRI@").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(9), Value::Int(16), Value::Int(25)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // CLEAVE tests
    // ========================================================================

    #[test]
    fn cleave_applies_multiple_quotations() {
        let eval = make_eval();
        let tokens = parse("5 [[DUP ADD] [DUP MUL] [1 ADD]] CLEAVE").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(10), Value::Int(25), Value::Int(6)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // SPREAD tests
    // ========================================================================

    #[test]
    fn spread_distributes_quotations() {
        let eval = make_eval();
        let tokens = parse("1 2 3 [[DUP ADD] [DUP MUL] [1 ADD]] SPREAD").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(2), Value::Int(4), Value::Int(4)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // COMPOSE tests
    // ========================================================================

    #[test]
    fn compose_concatenates_quotations() {
        let eval = make_eval();
        let tokens = parse("[1 ADD] [2 MUL] COMPOSE 5 SWAP CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(12)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // CURRY tests
    // ========================================================================

    #[test]
    fn curry_partial_application() {
        let eval = make_eval();
        let tokens = parse("10 [ADD] CURRY 5 SWAP CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(15)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn two_curry_partial_application() {
        let eval = make_eval();
        let tokens = parse("10 20 [ADD ADD] 2CURRY 5 SWAP CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(35)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn three_curry_partial_application() {
        let eval = make_eval();
        let tokens = parse("1 2 3 [ADD ADD ADD] 3CURRY 4 SWAP CALL").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(10)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // IF tests
    // ========================================================================

    #[test]
    fn if_true_branch() {
        let eval = make_eval();
        let tokens = parse("true [1] [2] IF").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(1)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn if_false_branch() {
        let eval = make_eval();
        let tokens = parse("false [1] [2] IF").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(2)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn if_with_comparison() {
        let eval = make_eval();
        let tokens = parse("5 3 GT [yes] [no] IF").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Word("yes".to_string())]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // WHEN tests
    // ========================================================================

    #[test]
    fn when_true_executes() {
        let eval = make_eval();
        let tokens = parse("true [42] WHEN").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(42)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn when_false_skips() {
        let eval = make_eval();
        let tokens = parse("false [42] WHEN").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert!(result.is_empty());
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // UNLESS tests
    // ========================================================================

    #[test]
    fn unless_false_executes() {
        let eval = make_eval();
        let tokens = parse("false [42] UNLESS").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(42)]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn unless_true_skips() {
        let eval = make_eval();
        let tokens = parse("true [42] UNLESS").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert!(result.is_empty());
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // MAP tests
    // ========================================================================

    #[test]
    fn map_transforms_sequence() {
        let eval = make_eval();
        let tokens = parse("[1 2 3] [DUP MUL] MAP").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(
            result,
            vec![Value::Sequence(vec![
                Value::Int(1),
                Value::Int(4),
                Value::Int(9)
            ])]
        );
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // FILTER tests
    // ========================================================================

    #[test]
    fn filter_selects_matching() {
        let eval = make_eval();
        let tokens = parse("[1 2 3 4 5 6] [EVEN] FILTER").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(
            result,
            vec![Value::Sequence(vec![
                Value::Int(2),
                Value::Int(4),
                Value::Int(6)
            ])]
        );
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // FOLD tests
    // ========================================================================

    #[test]
    fn fold_accumulates() {
        let eval = make_eval();
        let tokens = parse("[1 2 3 4 5] 0 [ADD] FOLD").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(15)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // EACH tests
    // ========================================================================

    #[test]
    fn each_iterates() {
        let eval = make_eval();
        let tokens = parse("0 [1 2 3] [ADD] EACH").unwrap();
        let result = eval.eval(&tokens).unwrap();
        assert_eq!(result, vec![Value::Int(6)]);
        println!("Stack: {result:?}");
    }

    // ========================================================================
    // Error handling tests
    // ========================================================================

    #[test]
    fn call_non_quotation_fails() {
        let eval = make_eval();
        let tokens = parse("42 CALL").unwrap();
        let result = eval.eval(&tokens);
        assert!(result.is_err());
        println!("Error: {result:?}");
    }

    #[test]
    fn dip_underflow_fails() {
        let eval = make_eval();
        let tokens = parse("[ADD] DIP").unwrap();
        let result = eval.eval(&tokens);
        assert!(result.is_err());
        println!("Error: {result:?}");
    }
}
