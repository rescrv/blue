//! Stack-based evaluator for caternary programs.
//!
//! The evaluator is generic over the stack element type `T`. Values (non-operator tokens)
//! are pushed onto the stack. Operators are functions that manipulate the stack.
//! Different evaluator instances can define different operators for the same program.

use std::collections::HashMap;
use std::collections::HashSet;

use handled::SError;

use crate::Quotable;
use crate::Token;

/// Evaluation error type.
pub type EvalError = SError;

const PHASE: &str = "caternary-eval";

/// Error code for operator failures.
pub const CODE_OPERATOR_ERROR: &str = "operator-error";

/// Construct an operator error with a structured message.
pub(crate) fn operator_error(message: impl AsRef<str>) -> EvalError {
    SError::new(PHASE)
        .with_code(CODE_OPERATOR_ERROR)
        .with_message(message.as_ref())
}

/// Returns `Some(name)` if `w` is a binding word `>name` whose name is a valid
/// local identifier: a leading ASCII letter or `_`, then ASCII alphanumerics or
/// `_` (the same grammar the optimizer uses for pattern variables).
fn bind_target(w: &str) -> Option<&str> {
    let rest = w.strip_prefix('>')?;
    if is_local_name(rest) {
        Some(rest)
    } else {
        None
    }
}

fn is_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Bake enclosing locals into a quotation by value (lexical capture at
/// construction time). A bare-word reference is replaced by the tokens of its
/// bound value only when it is *free* in the quotation -- not shadowed by a
/// binder (`>name`) lexically preceding it -- and bound in the enclosing `env`.
/// References that are not bound here are left untouched so they resolve when
/// (and where) the quotation eventually runs.
fn capture_body<T: Quotable>(body: &[Token], env: &HashMap<String, T>) -> Vec<Token> {
    let mut local: HashSet<&str> = HashSet::new();
    let mut out: Vec<Token> = Vec::with_capacity(body.len());
    for tok in body {
        match tok {
            Token::Word(w) => {
                if let Some(name) = bind_target(w) {
                    local.insert(name);
                    out.push(tok.clone());
                } else if !local.contains(w.as_str()) {
                    if let Some(value) = env.get(w) {
                        out.extend(value.to_tokens());
                    } else {
                        out.push(tok.clone());
                    }
                } else {
                    out.push(tok.clone());
                }
            }
            Token::Bracket(inner) => {
                if local.is_empty() {
                    out.push(Token::Bracket(capture_body(inner, env)));
                } else {
                    let shadowed: HashMap<String, T> = env
                        .iter()
                        .filter(|(k, _)| !local.contains(k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    out.push(Token::Bracket(capture_body(inner, &shadowed)));
                }
            }
        }
    }
    out
}

/// An operator function that manipulates the stack.
///
/// Operators receive a mutable reference to the stack and an immutable reference to
/// the evaluator. The evaluator reference allows operators to execute quotations
/// (bracketed code) that are on the stack, enabling combinators like `CALL`, `DIP`, etc.
pub type Operator<T> = fn(&mut Vec<T>, &Evaluator<T>) -> Result<(), EvalError>;

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
    T: Quotable,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Evaluator<T> {
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
}

impl<T> Evaluator<T>
where
    T: Quotable,
{
    /// Evaluates a program, returning the final stack.
    pub fn eval(&self, tokens: &[Token]) -> Result<Vec<T>, EvalError> {
        let mut stack = Vec::new();
        self.eval_with_stack(tokens, &mut stack)?;
        Ok(stack)
    }

    /// Evaluates tokens using an existing stack.
    ///
    /// Each call evaluates `tokens` in a single fresh lexical scope. Locals
    /// bound with `>name` live in a per-call environment and do not persist
    /// across calls (top-level reset). A quotation captures the locals in scope
    /// by value at the point it is pushed, so a quotation that escapes its
    /// defining scope still resolves its references. Re-entrant combinators call
    /// back through this method, so each quotation body runs in its own scope.
    pub fn eval_with_stack(&self, tokens: &[Token], stack: &mut Vec<T>) -> Result<(), EvalError> {
        let mut env: HashMap<String, T> = HashMap::new();
        self.eval_scope(tokens, stack, &mut env)
    }

    fn eval_scope(
        &self,
        tokens: &[Token],
        stack: &mut Vec<T>,
        env: &mut HashMap<String, T>,
    ) -> Result<(), EvalError> {
        for token in tokens {
            match token {
                Token::Word(w) => {
                    if let Some(name) = bind_target(w) {
                        // bind: pop the top of stack into a single-assignment local
                        if env.contains_key(name) {
                            return Err(operator_error(format!(
                                "single-assignment violation: `{name}` is already bound in this scope"
                            )));
                        }
                        let value = stack.pop().ok_or_else(|| {
                            operator_error(format!(
                                "stack underflow: `>{name}` needs a value to bind"
                            ))
                        })?;
                        env.insert(name.to_string(), value);
                    } else if let Some(value) = env.get(w) {
                        // reference: resolved scope-first, ahead of operator lookup
                        stack.push(value.clone());
                    } else if let Some(op) = self.operators.get(w) {
                        op(stack, self)?;
                    } else {
                        stack.push(T::from(token.clone()));
                    }
                }
                Token::Bracket(body) => {
                    // A quotation value captures the locals in scope by value.
                    // The empty-env fast path is byte-for-byte the original
                    // behavior, so programs without locals are unaffected.
                    if env.is_empty() {
                        stack.push(T::from(token.clone()));
                    } else {
                        stack.push(T::from(Token::Bracket(capture_body(body, env))));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Quotable;
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

    impl Quotable for Value {
        fn as_quotation(&self) -> Option<&[Token]> {
            match self {
                Value::Bracket(b) => Some(b),
                Value::Word(_) => None,
            }
        }

        fn to_tokens(&self) -> Vec<Token> {
            match self {
                Value::Word(w) => vec![Token::Word(w.clone())],
                Value::Bracket(b) => vec![Token::Bracket(b.clone())],
            }
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            match self {
                Value::Bracket(b) => Some(b.iter().map(|t| Value::from(t.clone())).collect()),
                Value::Word(_) => None,
            }
        }

        fn from_sequence(elements: Vec<Self>) -> Self {
            Value::Bracket(elements.iter().flat_map(|v| v.to_tokens()).collect())
        }
    }

    impl Quotable for i32 {
        fn as_quotation(&self) -> Option<&[Token]> {
            None
        }

        fn to_tokens(&self) -> Vec<Token> {
            vec![Token::Word(self.to_string())]
        }

        fn is_truthy(&self) -> bool {
            *self != 0
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            None
        }

        fn from_sequence(_elements: Vec<Self>) -> Self {
            0
        }
    }

    fn underflow() -> EvalError {
        operator_error("stack underflow")
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn operator_pops_and_pushes() {
        fn dup(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn different_evaluators_same_program() {
        fn scan_tables(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            let _name = stack.pop().ok_or(underflow())?;
            stack.push(Value::Word("scanned_table".to_string()));
            Ok(())
        }

        fn scan_files(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Table result: {table_result:?}");
        println!("File result: {file_result:?}");
    }

    #[test]
    fn filter_uses_bracket_arg() {
        fn filter(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            let predicate = stack
                .pop()
                .ok_or_else(|| operator_error("need predicate"))?;
            let data = stack.pop().ok_or_else(|| operator_error("need data"))?;
            stack.push(Value::Word(format!("filtered({data:?}, {predicate:?})")));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("FILTER", filter);

        let tokens = parse("data [x > 5] FILTER").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result.len(), 1);
        println!("Stack: {result:?}");
    }

    #[test]
    fn eval_with_existing_stack() {
        fn add(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {stack:?}");
    }

    #[test]
    fn empty_program() {
        let eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse("").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert!(result.is_empty());
        println!("Stack: {result:?}");
    }

    #[test]
    fn operator_error_propagates() {
        #[allow(clippy::ptr_arg)]
        fn fail(_stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            Err(operator_error("intentional failure"))
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("FAIL", fail);

        let tokens = parse("A B FAIL C").unwrap();
        let result = eval.eval(&tokens);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("intentional failure"));
        println!("Error: {err:?}");
    }

    #[test]
    fn stack_underflow_in_operator() {
        fn pop_two(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            let _a = stack.pop().ok_or(underflow())?;
            let _b = stack.pop().ok_or(underflow())?;
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("POP2", pop_two);

        let tokens = parse("A POP2").unwrap();
        let result = eval.eval(&tokens);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("stack underflow"));
        println!("Error: {err:?}");
    }

    #[test]
    fn swap_operator() {
        fn swap(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn drop_operator() {
        fn drop(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn over_operator() {
        fn over(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn rot_operator() {
        fn rot(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn multiple_operators_in_sequence() {
        fn dup(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            let val = stack.pop().ok_or(underflow())?;
            stack.push(val.clone());
            stack.push(val);
            Ok(())
        }

        fn swap(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(b);
            stack.push(a);
            Ok(())
        }

        fn drop(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn operator_with_numeric_stack() {
        fn add(stack: &mut Vec<i32>, _eval: &Evaluator<i32>) -> Result<(), EvalError> {
            let b = stack.pop().ok_or(underflow())?;
            let a = stack.pop().ok_or(underflow())?;
            stack.push(a + b);
            Ok(())
        }

        fn mul(stack: &mut Vec<i32>, _eval: &Evaluator<i32>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn redefine_operator() {
        #[allow(clippy::ptr_arg)]
        fn first(_stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            Err(operator_error("first"))
        }

        fn second(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            stack.push(Value::Word("second".to_string()));
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("OP", first);
        eval.define("OP", second);

        let tokens = parse("OP").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result, vec![Value::Word("second".to_string())]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn operator_name_is_case_sensitive() {
        fn lower(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            stack.push(Value::Word("lower".to_string()));
            Ok(())
        }

        fn upper(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
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
        println!("Stack: {result:?}");
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
        println!("Stack: {result:?}");
    }

    #[test]
    fn clear_stack_operator() {
        fn clear(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
            stack.clear();
            Ok(())
        }

        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("CLEAR", clear);

        let tokens = parse("A B C CLEAR D").unwrap();
        let result = eval.eval(&tokens).unwrap();

        assert_eq!(result, vec![Value::Word("D".to_string())]);
        println!("Stack: {result:?}");
    }

    #[test]
    fn depth_operator() {
        fn depth(stack: &mut Vec<Value>, _eval: &Evaluator<Value>) -> Result<(), EvalError> {
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
        println!("Stack: {result:?}");
    }
    fn locals_eval() -> Evaluator<Value> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        crate::register_combinators(&mut eval);

        eval.define("SWAP", |stack: &mut Vec<Value>, _eval| {
            let b = stack.pop().ok_or_else(underflow)?;
            let a = stack.pop().ok_or_else(underflow)?;
            stack.push(b);
            stack.push(a);
            Ok(())
        });

        eval.define("ADD", |stack: &mut Vec<Value>, _eval| {
            let b = stack.pop().ok_or_else(underflow)?;
            let a = stack.pop().ok_or_else(underflow)?;
            match (a, b) {
                (Value::Word(a), Value::Word(b)) => {
                    let x: i64 = a
                        .parse()
                        .map_err(|_| operator_error("ADD needs integers"))?;
                    let y: i64 = b
                        .parse()
                        .map_err(|_| operator_error("ADD needs integers"))?;
                    stack.push(Value::Word((x + y).to_string()));
                    Ok(())
                }
                _ => Err(operator_error("ADD needs integers")),
            }
        });

        eval
    }

    fn word(s: &str) -> Value {
        Value::Word(s.to_string())
    }

    #[test]
    fn local_binds_and_references() {
        let eval = locals_eval();
        let tokens = parse("5 >x x x ADD").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("10")]);
    }

    #[test]
    fn local_set_aside_and_reintroduce() {
        let eval = locals_eval();
        let tokens = parse("5 >x 3 x ADD").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("8")]);
    }

    #[test]
    fn local_names_allow_underscore_and_digits_after_first_character() {
        let eval = locals_eval();
        let tokens = parse("11 >_x1 _x1 4 ADD").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("15")]);
    }

    #[test]
    fn invalid_binding_words_remain_literals() {
        let eval = locals_eval();
        let tokens = parse("5 >1x > >x-y").unwrap();
        assert_eq!(
            eval.eval(&tokens).unwrap(),
            vec![word("5"), word(">1x"), word(">"), word(">x-y")]
        );
    }

    #[test]
    fn closure_captures_local_by_value_inline() {
        let eval = locals_eval();
        let tokens = parse("5 10 >n [n ADD] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("15")]);
    }

    #[test]
    fn nested_quotation_captures_outer_local() {
        let eval = locals_eval();
        let tokens = parse("5 10 >n [[n ADD] CALL] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("15")]);
    }

    #[test]
    fn closure_capture_survives_scope_exit() {
        // [n ADD] is built where n = 10, then escapes that scope before CALL.
        // By-value capture must keep n = 10.
        let eval = locals_eval();
        let tokens = parse("[ 10 >n [n ADD] ] CALL 20 SWAP CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("30")]);
    }

    #[test]
    fn inner_binder_shadows_captured_name() {
        // Inner >n rebinds n to 7; the outer n = 99 must not leak in (would be 198).
        let eval = locals_eval();
        let tokens = parse("99 >n [ 7 >n n n ADD ] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("14")]);
    }

    #[test]
    fn nested_quotation_after_inner_binder_uses_inner_scope() {
        let eval = locals_eval();
        let tokens = parse("1 99 >n [ 7 >n [n ADD] CALL ] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("8")]);
    }

    #[test]
    fn local_can_hold_and_call_quotation_value() {
        let eval = locals_eval();
        let tokens = parse("[2 ADD] >q 3 q CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("5")]);
    }

    #[test]
    fn reference_resolves_before_operator() {
        // A local named ADD shadows the ADD operator within its scope.
        let eval = locals_eval();
        let tokens = parse("7 >ADD ADD").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("7")]);
    }

    #[test]
    fn rebinding_in_same_scope_is_an_error() {
        let eval = locals_eval();
        let tokens = parse("5 >x 6 >x").unwrap();
        let err = eval.eval(&tokens).unwrap_err();
        assert!(
            err.to_string()
                .contains("single-assignment violation: `x` is already bound in this scope")
        );
    }

    #[test]
    fn binding_with_empty_stack_is_an_error() {
        let eval = locals_eval();
        let tokens = parse(">x").unwrap();
        let err = eval.eval(&tokens).unwrap_err();
        assert!(
            err.to_string()
                .contains("stack underflow: `>x` needs a value to bind")
        );
    }

    #[test]
    fn locals_do_not_persist_across_calls() {
        let eval = locals_eval();
        let mut stack = Vec::new();
        eval.eval_with_stack(&parse("5 >x").unwrap(), &mut stack)
            .unwrap();
        // x is gone on the next call; `x` is now an undefined word, pushed as a literal.
        eval.eval_with_stack(&parse("x").unwrap(), &mut stack)
            .unwrap();
        assert_eq!(stack, vec![word("x")]);
    }

    #[test]
    fn locals_do_not_leak_between_quotation_calls() {
        let eval = locals_eval();
        let tokens = parse("[5 >x] CALL [x] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("x")]);
    }
}
