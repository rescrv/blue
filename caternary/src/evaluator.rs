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

/// Error code for malformed or misplaced function definitions (load-time).
pub const CODE_DEFINITION_ERROR: &str = "definition-error";

/// Construct an operator error with a structured message.
pub(crate) fn operator_error(message: impl AsRef<str>) -> EvalError {
    SError::new(PHASE)
        .with_code(CODE_OPERATOR_ERROR)
        .with_message(message.as_ref())
}

/// Construct a definition error with a structured message. Raised only by the
/// `load` pre-pass when a `:name` binder is malformed or out of position.
pub(crate) fn definition_error(message: impl AsRef<str>) -> EvalError {
    SError::new(PHASE)
        .with_code(CODE_DEFINITION_ERROR)
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

/// Returns `Some(name)` if `w` is a well-formed definition binder `:name` whose
/// name is a valid identifier (the same grammar as locals). A `:`-prefixed word
/// with a malformed suffix returns `None` here; the `load` pre-pass detects the
/// `:` prefix separately (see [`has_def_prefix`]) so it can reject malformed
/// binders rather than silently treating them as ordinary words.
fn def_target(w: &str) -> Option<&str> {
    let rest = w.strip_prefix(':')?;
    if is_local_name(rest) {
        Some(rest)
    } else {
        None
    }
}

/// True for any word beginning with `:`. The `load` pre-pass uses this to give a
/// strict error for every `:`-prefixed word that is not a valid top-level
/// `[ body ] :name` definition, and `eval_scope` uses it as a backstop so a
/// stray binder can never be mistaken for a literal during evaluation.
fn has_def_prefix(w: &str) -> bool {
    w.starts_with(':')
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
///
/// User functions defined with `[ body ] :name` are collected by [`Evaluator::load`]
/// into the `definitions` table, which is a sibling of `operators` and shares its
/// lifetime: load once, evaluate many programs against the result. Definition names
/// are resolved ahead of operators (so a definition may shadow a builtin) and after
/// locals (so a `>name` local shadows a definition of the same name).
pub struct Evaluator<T> {
    operators: HashMap<String, Operator<T>>,
    definitions: HashMap<String, Vec<Token>>,
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
    /// Creates a new evaluator with no operators and no definitions.
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
            definitions: HashMap::new(),
        }
    }

    /// Defines an operator by name.
    pub fn define(&mut self, name: &str, op: Operator<T>) {
        self.operators.insert(name.to_string(), op);
    }

    /// Loads function definitions from a top-level token stream.
    ///
    /// This is a static pre-pass: it walks `tokens` and collects every
    /// `[ body ] :name` definition into the `definitions` table *before* any
    /// evaluation. Because the whole stream is seen before execution,
    /// references resolve regardless of textual order (forward references are
    /// fine) and regardless of runtime control flow; recursion works because a
    /// body resolves its own name at call time against the fully populated
    /// table. Load is additive and may be called multiple times to assemble a
    /// library; a redefinition is rejected across calls as well as within one.
    ///
    /// A `:`-prefixed word is legal only as the second half of a top-level
    /// `[ body ] :name` pair. Every other occurrence is a static
    /// [`definition_error`]: a binder nested inside a quotation, a binder with
    /// no preceding quotation, a binder following a non-quotation, a malformed
    /// name (e.g. `:2x` or a bare `:`), or a redefinition.
    ///
    /// Definitions are stored verbatim: at top level the locals environment is
    /// empty, so there are no enclosing locals to capture and no token
    /// expansion occurs.
    pub fn load(&mut self, tokens: &[Token]) -> Result<(), EvalError> {
        // A `:`-prefixed binder is only ever legal at the top level. Reject any
        // that appear (at any depth) inside a quotation before pairing the rest.
        fn reject_nested(tokens: &[Token]) -> Result<(), EvalError> {
            for tok in tokens {
                if let Token::Bracket(inner) = tok {
                    for t in inner {
                        if let Token::Word(w) = t
                            && has_def_prefix(w)
                        {
                            return Err(definition_error(format!(
                                "definition `{w}` must appear at top level, not inside a quotation"
                            )));
                        }
                    }
                    reject_nested(inner)?;
                }
            }
            Ok(())
        }
        reject_nested(tokens)?;

        // Walk the top level, pairing each binder with the quotation before it.
        let mut i = 0;
        while i < tokens.len() {
            if let Token::Word(w) = &tokens[i]
                && has_def_prefix(w)
            {
                let name = def_target(w).ok_or_else(|| {
                    definition_error(format!(
                        "malformed definition binder `{w}`: expected `:name`"
                    ))
                })?;
                if i == 0 {
                    return Err(definition_error(format!(
                        "definition `:{name}` has no preceding quotation to bind"
                    )));
                }
                let body = match &tokens[i - 1] {
                    Token::Bracket(body) => body.clone(),
                    Token::Word(prev) => {
                        return Err(definition_error(format!(
                            "definition `:{name}` must follow a quotation, found word `{prev}`"
                        )));
                    }
                };
                if self.definitions.contains_key(name) {
                    return Err(definition_error(format!(
                        "redefinition: `:{name}` is already defined"
                    )));
                }
                self.definitions.insert(name.to_string(), body);
            }
            i += 1;
        }
        Ok(())
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
                    } else if has_def_prefix(w) {
                        // A `:`-binder reached evaluation. `load` consumes every
                        // valid binder and rejects malformed ones, so reaching
                        // here means definitions were not loaded (or a binder
                        // was smuggled into a quotation body, which `load` also
                        // rejects). Fail loudly rather than push it as a literal.
                        return Err(definition_error(format!(
                            "definition binder `{w}` encountered during evaluation; \
                             definitions must be loaded with `Evaluator::load`, not evaluated"
                        )));
                    } else if let Some(value) = env.get(w) {
                        // reference: resolved scope-first, ahead of definitions and operators
                        stack.push(value.clone());
                    } else if let Some(body) = self.definitions.get(w) {
                        // function call: re-enter in a fresh scope. Resolved at
                        // call time against the populated table, so a definition
                        // may call itself (recursion) or any sibling definition,
                        // in any textual order.
                        let body = body.clone();
                        self.eval_with_stack(&body, stack)?;
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

    fn load_definitions(eval: &mut Evaluator<Value>, source: &str) {
        let tokens = parse(source).unwrap();
        eval.load(&tokens).unwrap();
    }

    fn assert_load_error(source: &str, expected: &str) {
        let mut eval = locals_eval();
        let tokens = parse(source).unwrap();
        let err = eval.load(&tokens).unwrap_err();
        assert!(
            err.to_string().contains(expected),
            "expected error to contain {expected:?}, got {err:?}"
        );
    }

    #[test]
    fn load_registers_definition_and_evaluates_later() {
        let mut eval = locals_eval();
        load_definitions(&mut eval, "[1 ADD] :inc");

        let tokens = parse("5 inc").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("6")]);
    }

    #[test]
    fn load_allows_forward_references_between_definitions() {
        let mut eval = locals_eval();
        load_definitions(&mut eval, "[inc inc] :twice [1 ADD] :inc");

        let tokens = parse("3 twice").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("5")]);
    }

    #[test]
    fn load_is_additive_across_calls() {
        let mut eval = locals_eval();
        load_definitions(&mut eval, "[1 ADD] :inc");
        load_definitions(&mut eval, "[inc inc] :twice");

        let tokens = parse("4 twice").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("6")]);
    }

    #[test]
    fn definition_names_shadow_operators() {
        let mut eval = locals_eval();
        eval.define("MARK", |stack: &mut Vec<Value>, _eval| {
            stack.push(word("operator"));
            Ok(())
        });
        load_definitions(&mut eval, "[definition] :MARK");

        let tokens = parse("MARK").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("definition")]);
    }

    #[test]
    fn locals_shadow_definitions() {
        let mut eval = locals_eval();
        load_definitions(&mut eval, "[definition] :x");

        let tokens = parse("local >x x").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("local")]);
    }

    #[test]
    fn definitions_are_visible_inside_quotations() {
        let mut eval = locals_eval();
        crate::register_combinators(&mut eval);
        load_definitions(&mut eval, "[1 ADD] :inc");

        let tokens = parse("5 [inc] CALL").unwrap();
        assert_eq!(eval.eval(&tokens).unwrap(), vec![word("6")]);
    }

    #[test]
    fn evaluating_definition_binder_without_load_is_an_error() {
        let eval = locals_eval();
        let tokens = parse("[1 ADD] :inc").unwrap();
        let err = eval.eval(&tokens).unwrap_err();
        assert!(
            err.to_string()
                .contains("definition binder `:inc` encountered during evaluation")
        );
    }

    #[test]
    fn load_rejects_malformed_definition_binder() {
        assert_load_error(
            "[body] :2x",
            "malformed definition binder `:2x`: expected `:name`",
        );
    }

    #[test]
    fn load_rejects_definition_without_preceding_quotation() {
        assert_load_error(":x", "definition `:x` has no preceding quotation to bind");
    }

    #[test]
    fn load_rejects_definition_after_word() {
        assert_load_error(
            "not_a_quotation :x",
            "definition `:x` must follow a quotation, found word `not_a_quotation`",
        );
    }

    #[test]
    fn load_rejects_nested_definition_binder() {
        assert_load_error(
            "[[inner] :x] :outer",
            "definition `:x` must appear at top level, not inside a quotation",
        );
    }

    #[test]
    fn load_rejects_redefinition_within_one_call() {
        assert_load_error(
            "[first] :x [second] :x",
            "redefinition: `:x` is already defined",
        );
    }

    #[test]
    fn load_rejects_redefinition_across_calls() {
        let mut eval = locals_eval();
        load_definitions(&mut eval, "[first] :x");

        let tokens = parse("[second] :x").unwrap();
        let err = eval.load(&tokens).unwrap_err();
        assert!(
            err.to_string()
                .contains("redefinition: `:x` is already defined")
        );
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
