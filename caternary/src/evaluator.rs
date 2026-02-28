//! Stack-based evaluator for caternary programs.
//!
//! The evaluator is generic over the stack element type `T`. Values (non-operator tokens)
//! are pushed onto the stack. Operators are functions that manipulate the stack.
//! Different evaluator instances can define different operators for the same program.

use std::collections::HashMap;

use crate::Token;

/// An error that can occur during evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Operator returned an error.
    OperatorError(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::OperatorError(msg) => write!(f, "operator error: {}", msg),
        }
    }
}

impl std::error::Error for EvalError {}

/// An operator function that manipulates the stack.
pub type Operator<T> = fn(&mut Vec<T>) -> Result<(), EvalError>;

/// A stack-based evaluator for caternary programs.
///
/// The evaluator is generic over the stack element type `T`. Operators are registered
/// by name and invoked when the corresponding word is encountered. Non-operator tokens
/// are converted to `T` via `From<Token>` and pushed onto the stack.
pub struct Evaluator<T> {
    operators: HashMap<String, Operator<T>>,
}

impl<T> Default for Evaluator<T>
where
    T: From<Token>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Evaluator<T>
where
    T: From<Token>,
{
    /// Creates a new evaluator with no operators defined.
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
        }
    }

    /// Defines an operator by name.
    pub fn define(&mut self, name: &str, op: Operator<T>) {
        self.operators.insert(name.to_string(), op);
    }

    /// Evaluates a program, returning the final stack.
    pub fn eval(&self, tokens: &[Token]) -> Result<Vec<T>, EvalError> {
        let mut stack = Vec::new();
        self.eval_with_stack(tokens, &mut stack)?;
        Ok(stack)
    }

    /// Evaluates tokens using an existing stack.
    pub fn eval_with_stack(&self, tokens: &[Token], stack: &mut Vec<T>) -> Result<(), EvalError> {
        for token in tokens {
            match token {
                Token::Word(name) if self.operators.contains_key(name) => {
                    let op = self.operators.get(name).unwrap();
                    op(stack)?;
                }
                _ => {
                    stack.push(T::from(token.clone()));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[derive(Debug, Clone, PartialEq)]
    enum Value {
        Word(String),
        Bracket(Vec<Token>),
    }

    impl From<Token> for Value {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => Value::Word(w),
                Token::Bracket(b) => Value::Bracket(b),
            }
        }
    }

    impl From<Token> for i32 {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => w.parse().unwrap_or(0),
                Token::Bracket(_) => 0,
            }
        }
    }

    fn underflow() -> EvalError {
        EvalError::OperatorError("stack underflow".to_string())
    }

    #[test]
    fn push_values() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("A B C").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![
                Value::Word("A".to_string()),
                Value::Word("B".to_string()),
                Value::Word("C".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn operator_pops_and_pushes() {
        fn dup(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let val = stack.pop().ok_or(underflow())?;
            stack.push(val.clone());
            stack.push(val);
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("DUP", dup);

        let tokens = parse("A DUP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![Value::Word("A".to_string()), Value::Word("A".to_string()),]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn bracket_pushed_as_value() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("A [foo < 5] B").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], Value::Word(_)));
        assert!(matches!(result[1], Value::Bracket(_)));
        assert!(matches!(result[2], Value::Word(_)));
        println!("Stack: {:?}", result);
    }

    #[test]
    fn different_evaluators_same_program() {
        fn scan_tables(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _name = stack.pop().ok_or(underflow())?;
            stack.push(Value::Word("scanned_table".to_string()));
            Ok(())
        }

        fn scan_files(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _name = stack.pop().ok_or(underflow())?;
            stack.push(Value::Word("scanned_file".to_string()));
            Ok(())
        }

        let mut table_eval: Evaluator<Value> = Evaluator::new();
        table_eval.define("SCAN", scan_tables);

        let mut file_eval: Evaluator<Value> = Evaluator::new();
        file_eval.define("SCAN", scan_files);

        let tokens = parse("users SCAN").unwrap();

        let table_result = table_eval.eval(&tokens).unwrap();
        let file_result = file_eval.eval(&tokens).unwrap();

        assert_eq!(table_result, vec![Value::Word("scanned_table".to_string())]);
        assert_eq!(file_result, vec![Value::Word("scanned_file".to_string())]);
        println!("Table result: {:?}", table_result);
        println!("File result: {:?}", file_result);
    }

    #[test]
    fn filter_uses_bracket_arg() {
        fn filter(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let predicate = stack
                .pop()
                .ok_or(EvalError::OperatorError("need predicate".to_string()))?;
            let data = stack
                .pop()
                .ok_or(EvalError::OperatorError("need data".to_string()))?;
            stack.push(Value::Word(format!(
                "filtered({:?}, {:?})",
                data, predicate
            )));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("FILTER", filter);

        let tokens = parse("data [x > 5] FILTER").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result.len(), 1);
        println!("Stack: {:?}", result);
    }

    #[test]
    fn eval_with_existing_stack() {
        fn add(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _b = stack.pop().ok_or(underflow())?;
            let _a = stack.pop().ok_or(underflow())?;
            stack.push(Value::Word("sum".to_string()));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("ADD", add);

        let mut stack = vec![Value::Word("existing".to_string())];
        let tokens = parse("A B ADD").unwrap();
        eval.eval_with_stack(&tokens, &mut stack).unwrap();

        assert_eq!(
            stack,
            vec![
                Value::Word("existing".to_string()),
                Value::Word("sum".to_string()),
            ]
        );
        println!("Stack: {:?}", stack);
    }

    #[test]
    fn empty_program() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert!(result.is_empty());
        println!("Stack: {:?}", result);
    }

    #[test]
    fn operator_error_propagates() {
        fn fail(_stack: &mut Vec<Value>) -> Result<(), EvalError> {
            Err(EvalError::OperatorError("intentional failure".to_string()))
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("FAIL", fail);

        let tokens = parse("A B FAIL C").unwrap();
        let result = eval.eval(&tokens);

        assert!(matches!(result, Err(EvalError::OperatorError(_))));
        println!("Error: {:?}", result);
    }

    #[test]
    fn stack_underflow_in_operator() {
        fn pop_two(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _a = stack.pop().ok_or(underflow())?;
            let _b = stack.pop().ok_or(underflow())?;
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("POP2", pop_two);

        let tokens = parse("A POP2").unwrap();
        let result = eval.eval(&tokens);

        assert!(matches!(result, Err(EvalError::OperatorError(_))));
        println!("Error: {:?}", result);
    }

    #[test]
    fn swap_operator() {
        fn swap(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(b);
            stack.push(a);
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("SWAP", swap);

        let tokens = parse("A B SWAP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![Value::Word("B".to_string()), Value::Word("A".to_string()),]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn drop_operator() {
        fn drop(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _ = stack.pop().ok_or(underflow())?;
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("DROP", drop);

        let tokens = parse("A B C DROP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![Value::Word("A".to_string()), Value::Word("B".to_string()),]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn over_operator() {
        fn over(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(a.clone());
            stack.push(b);
            stack.push(a);
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("OVER", over);

        let tokens = parse("A B OVER").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![
                Value::Word("A".to_string()),
                Value::Word("B".to_string()),
                Value::Word("A".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn rot_operator() {
        fn rot(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let c = stack.pop().ok_or(underflow())?;
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(b);
            stack.push(c);
            stack.push(a);
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("ROT", rot);

        let tokens = parse("A B C ROT").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![
                Value::Word("B".to_string()),
                Value::Word("C".to_string()),
                Value::Word("A".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn multiple_operators_in_sequence() {
        fn dup(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let val = stack.pop().ok_or(underflow())?;
            stack.push(val.clone());
            stack.push(val);
            Ok(())
        }

        fn swap(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(b);
            stack.push(a);
            Ok(())
        }

        fn drop(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let _ = stack.pop().ok_or(underflow())?;
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("DUP", dup);
        eval.define("SWAP", swap);
        eval.define("DROP", drop);

        let tokens = parse("A B DUP SWAP DROP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        // A B DUP -> A B B
        // A B B SWAP -> A B B with top two swapped -> A B B (B and B swapped, same)
        // Actually: A B B SWAP -> [A] [B B] -> swap top two of [B B] -> [A] [B B]
        // Wait, SWAP swaps top two: A B B -> A B B (B<->B) = A B B
        // Then DROP: A B B DROP -> A B
        assert_eq!(
            result,
            vec![Value::Word("A".to_string()), Value::Word("B".to_string()),]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn operator_with_numeric_stack() {
        fn add(stack: &mut Vec<i32>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(a + b);
            Ok(())
        }

        fn mul(stack: &mut Vec<i32>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(a * b);
            Ok(())
        }

        let mut eval: Evaluator<i32> = Evaluator::new();
        eval.define("ADD", add);
        eval.define("MUL", mul);

        let tokens = parse("2 3 ADD 4 MUL").unwrap();
        let result = eval.eval(&tokens).unwrap();

        // 2 3 ADD -> 5
        // 5 4 MUL -> 20
        assert_eq!(result, vec![20]);
        println!("Stack: {:?}", result);
    }

    #[test]
    fn redefine_operator() {
        fn first(_stack: &mut Vec<Value>) -> Result<(), EvalError> {
            Err(EvalError::OperatorError("first".to_string()))
        }

        fn second(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            stack.push(Value::Word("second".to_string()));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("OP", first);
        eval.define("OP", second);

        let tokens = parse("OP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result, vec![Value::Word("second".to_string())]);
        println!("Stack: {:?}", result);
    }

    #[test]
    fn operator_name_is_case_sensitive() {
        fn lower(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            stack.push(Value::Word("lower".to_string()));
            Ok(())
        }

        fn upper(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            stack.push(Value::Word("upper".to_string()));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("op", lower);
        eval.define("OP", upper);

        let tokens = parse("op OP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![
                Value::Word("lower".to_string()),
                Value::Word("upper".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn nested_bracket_preserved() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("[[A B] [C D]]").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result.len(), 1);
        if let Value::Bracket(outer) = &result[0] {
            assert_eq!(outer.len(), 2);
            assert!(matches!(&outer[0], Token::Bracket(_)));
            assert!(matches!(&outer[1], Token::Bracket(_)));
        } else {
            panic!("Expected bracket");
        }
        println!("Stack: {:?}", result);
    }

    #[test]
    fn word_same_as_operator_name_pushed_when_not_defined() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("DUP SWAP DROP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        // No operators defined, so these are just words
        assert_eq!(
            result,
            vec![
                Value::Word("DUP".to_string()),
                Value::Word("SWAP".to_string()),
                Value::Word("DROP".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }

    #[test]
    fn clear_stack_operator() {
        fn clear(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            stack.clear();
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("CLEAR", clear);

        let tokens = parse("A B C CLEAR D").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result, vec![Value::Word("D".to_string())]);
        println!("Stack: {:?}", result);
    }

    #[test]
    fn depth_operator() {
        fn depth(stack: &mut Vec<Value>) -> Result<(), EvalError> {
            let d = stack.len();
            stack.push(Value::Word(d.to_string()));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("DEPTH", depth);

        let tokens = parse("A B C DEPTH").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(
            result,
            vec![
                Value::Word("A".to_string()),
                Value::Word("B".to_string()),
                Value::Word("C".to_string()),
                Value::Word("3".to_string()),
            ]
        );
        println!("Stack: {:?}", result);
    }
}
