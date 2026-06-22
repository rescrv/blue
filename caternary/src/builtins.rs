//! Standard stack-manipulation builtins.

// Operators must match the `Operator<T>` type signature which requires `&mut Vec<T>`.
#![allow(clippy::ptr_arg)]

use crate::EvalError;
use crate::Evaluator;
use crate::Token;
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

#[cfg(test)]
mod tests {
    use crate::{Evaluator, Quotable, Token, parse};

    use super::register_stack_builtins;

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
}
