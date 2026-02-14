//! String-based pattern matching optimizer for caternary.
//!
//! Patterns use variables to match and capture tokens:
//! - `$name` matches exactly one token (word or bracket)
//! - `$*name` matches zero or more tokens
//!
//! Variables are interpolated into the replacement string.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use crate::ParseError;
use crate::Token;
use crate::parse;

/// A rewrite rule: pattern => replacement.
#[derive(Debug, Clone)]
pub struct Rule {
    pattern: Vec<PatternElement>,
    replacement: Vec<ReplacementElement>,
}

#[derive(Debug, Clone)]
enum PatternElement {
    /// Matches a literal word.
    Word(String),
    /// Matches a literal bracket with a nested pattern.
    Bracket(Vec<PatternElement>),
    /// Matches exactly one token and binds it to a name.
    Var(String),
    /// Matches zero or more tokens and binds them to a name.
    VarMany(String),
}

#[derive(Debug, Clone)]
enum ReplacementElement {
    /// A literal word.
    Word(String),
    /// A literal bracket with nested replacements.
    Bracket(Vec<ReplacementElement>),
    /// Interpolate a bound variable (single token).
    Var(String),
    /// Interpolate a bound variable (multiple tokens).
    VarMany(String),
}

/// Bindings captured during pattern matching.
type Bindings = HashMap<String, Vec<Token>>;

/// An error from rule parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// Failed to parse the pattern string.
    PatternParse(ParseError),
    /// Failed to parse the replacement string.
    ReplacementParse(ParseError),
    /// Variable in replacement not bound in pattern.
    UnboundVariable(String),
    /// Variable name is invalid.
    InvalidVariableName(String),
    /// Same variable name used with both `$name` and `$*name`.
    VariableArityMismatch(String),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::PatternParse(e) => write!(f, "pattern parse error: {e}"),
            RuleError::ReplacementParse(e) => write!(f, "replacement parse error: {e}"),
            RuleError::UnboundVariable(name) => write!(f, "unbound variable: {name}"),
            RuleError::InvalidVariableName(name) => write!(f, "invalid variable name: {name}"),
            RuleError::VariableArityMismatch(name) => {
                write!(f, "variable used with mixed arity: {name}")
            }
        }
    }
}

impl std::error::Error for RuleError {}

impl Rule {
    /// Creates a new rule from pattern and replacement strings.
    pub fn new(pattern: &str, replacement: &str) -> Result<Self, RuleError> {
        let pattern_tokens = parse(pattern).map_err(RuleError::PatternParse)?;
        let replacement_tokens = parse(replacement).map_err(RuleError::ReplacementParse)?;

        let pattern = Self::parse_pattern(&pattern_tokens)?;
        let replacement = Self::parse_replacement(&replacement_tokens)?;

        // Validate that all replacement variables are bound in pattern and have matching arity.
        let mut bound: HashMap<String, bool> = HashMap::new();
        Self::collect_pattern_vars(&pattern, &mut bound)?;
        let mut replacement_vars: HashMap<String, bool> = HashMap::new();
        Self::collect_replacement_vars(&replacement, &mut replacement_vars)?;
        for (name, replacement_is_many) in replacement_vars {
            let Some(pattern_is_many) = bound.get(&name).copied() else {
                return Err(RuleError::UnboundVariable(name));
            };
            if pattern_is_many != replacement_is_many {
                return Err(RuleError::VariableArityMismatch(name));
            }
        }

        Ok(Self {
            pattern,
            replacement,
        })
    }

    fn parse_pattern(tokens: &[Token]) -> Result<Vec<PatternElement>, RuleError> {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(w) if w.starts_with("$*") => Ok(PatternElement::VarMany(
                    Self::parse_variable_name(&w[2..])?.to_string(),
                )),
                Token::Word(w) if w.starts_with('$') => Ok(PatternElement::Var(
                    Self::parse_variable_name(&w[1..])?.to_string(),
                )),
                Token::Word(w) => Ok(PatternElement::Word(w.clone())),
                Token::Bracket(inner) => Ok(PatternElement::Bracket(Self::parse_pattern(inner)?)),
            })
            .collect::<Result<Vec<_>, RuleError>>()
    }

    fn parse_replacement(tokens: &[Token]) -> Result<Vec<ReplacementElement>, RuleError> {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(w) if w.starts_with("$*") => Ok(ReplacementElement::VarMany(
                    Self::parse_variable_name(&w[2..])?.to_string(),
                )),
                Token::Word(w) if w.starts_with('$') => Ok(ReplacementElement::Var(
                    Self::parse_variable_name(&w[1..])?.to_string(),
                )),
                Token::Word(w) => Ok(ReplacementElement::Word(w.clone())),
                Token::Bracket(inner) => {
                    Ok(ReplacementElement::Bracket(Self::parse_replacement(inner)?))
                }
            })
            .collect::<Result<Vec<_>, RuleError>>()
    }

    fn collect_pattern_vars(
        pattern: &[PatternElement],
        vars: &mut HashMap<String, bool>,
    ) -> Result<(), RuleError> {
        for elem in pattern {
            match elem {
                PatternElement::Var(name) => {
                    Self::insert_var_arity(vars, name, false)?;
                }
                PatternElement::VarMany(name) => {
                    Self::insert_var_arity(vars, name, true)?;
                }
                PatternElement::Bracket(inner) => Self::collect_pattern_vars(inner, vars)?,
                PatternElement::Word(_) => {}
            }
        }
        Ok(())
    }

    fn collect_replacement_vars(
        replacement: &[ReplacementElement],
        vars: &mut HashMap<String, bool>,
    ) -> Result<(), RuleError> {
        for elem in replacement {
            match elem {
                ReplacementElement::Var(name) => {
                    Self::insert_var_arity(vars, name, false)?;
                }
                ReplacementElement::VarMany(name) => Self::insert_var_arity(vars, name, true)?,
                ReplacementElement::Bracket(inner) => Self::collect_replacement_vars(inner, vars)?,
                ReplacementElement::Word(_) => {}
            }
        }
        Ok(())
    }

    fn parse_variable_name(raw: &str) -> Result<&str, RuleError> {
        if !Self::is_valid_variable_name(raw) {
            return Err(RuleError::InvalidVariableName(raw.to_string()));
        }
        Ok(raw)
    }

    fn is_valid_variable_name(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    fn insert_var_arity(
        vars: &mut HashMap<String, bool>,
        name: &str,
        is_many: bool,
    ) -> Result<(), RuleError> {
        if let Some(existing) = vars.get(name).copied() {
            if existing != is_many {
                return Err(RuleError::VariableArityMismatch(name.to_string()));
            }
            return Ok(());
        }
        vars.insert(name.to_string(), is_many);
        Ok(())
    }

    /// Attempts to apply this rule to a token sequence, returning the rewritten sequence if matched.
    /// Returns `None` if no match produces a different result.
    pub fn apply(&self, tokens: &[Token]) -> Option<Vec<Token>> {
        self.apply_with_limit(tokens, usize::MAX)
    }

    /// Like [`Rule::apply`] but bounds backtracking through `$*name` pattern variables.
    pub fn apply_with_limit(&self, tokens: &[Token], max_backtracks: usize) -> Option<Vec<Token>> {
        let mut budget = BacktrackBudget::new(max_backtracks);
        for start in 0..=tokens.len() {
            if let Some((end, bindings)) = self.match_at(tokens, start, &mut budget) {
                let mut result = tokens[..start].to_vec();
                result.extend(self.substitute(&bindings));
                result.extend(tokens[end..].to_vec());
                if result != tokens {
                    return Some(result);
                }
            }
        }
        None
    }

    fn match_at(
        &self,
        tokens: &[Token],
        start: usize,
        budget: &mut BacktrackBudget,
    ) -> Option<(usize, Bindings)> {
        let mut bindings = Bindings::new();
        let end = Self::match_pattern(&self.pattern, tokens, start, &mut bindings, budget)?;
        Some((end, bindings))
    }

    fn match_pattern(
        pattern: &[PatternElement],
        tokens: &[Token],
        pos: usize,
        bindings: &mut Bindings,
        budget: &mut BacktrackBudget,
    ) -> Option<usize> {
        let mut pat_idx = 0;
        let mut tok_idx = pos;

        while pat_idx < pattern.len() {
            match &pattern[pat_idx] {
                PatternElement::Word(w) => {
                    if tok_idx >= tokens.len() {
                        return None;
                    }
                    if let Token::Word(tw) = &tokens[tok_idx] {
                        if tw != w {
                            return None;
                        }
                    } else {
                        return None;
                    }
                    tok_idx += 1;
                    pat_idx += 1;
                }
                PatternElement::Bracket(inner_pattern) => {
                    if tok_idx >= tokens.len() {
                        return None;
                    }
                    if let Token::Bracket(inner_tokens) = &tokens[tok_idx] {
                        let end =
                            Self::match_pattern(inner_pattern, inner_tokens, 0, bindings, budget)?;
                        if end != inner_tokens.len() {
                            return None;
                        }
                    } else {
                        return None;
                    }
                    tok_idx += 1;
                    pat_idx += 1;
                }
                PatternElement::Var(name) => {
                    if tok_idx >= tokens.len() {
                        return None;
                    }
                    if !Self::bind_tokens(name, vec![tokens[tok_idx].clone()], bindings) {
                        return None;
                    }
                    tok_idx += 1;
                    pat_idx += 1;
                }
                PatternElement::VarMany(name) => {
                    // Greedy: try matching as many as possible first, then fewer
                    let remaining_pattern = &pattern[pat_idx + 1..];
                    let available = tokens.len().saturating_sub(tok_idx);
                    for take in (0..=available).rev() {
                        if !budget.consume() {
                            return None;
                        }
                        let captured: Vec<Token> = tokens[tok_idx..tok_idx + take].to_vec();
                        let mut test_bindings = bindings.clone();
                        if !Self::bind_tokens(name, captured, &mut test_bindings) {
                            continue;
                        }
                        if let Some(end) = Self::match_pattern(
                            remaining_pattern,
                            tokens,
                            tok_idx + take,
                            &mut test_bindings,
                            budget,
                        ) {
                            *bindings = test_bindings;
                            return Some(end);
                        }
                    }
                    return None;
                }
            }
        }

        Some(tok_idx)
    }

    fn bind_tokens(name: &str, captured: Vec<Token>, bindings: &mut Bindings) -> bool {
        if let Some(existing) = bindings.get(name) {
            existing == &captured
        } else {
            bindings.insert(name.to_string(), captured);
            true
        }
    }

    fn substitute(&self, bindings: &Bindings) -> Vec<Token> {
        Self::substitute_elements(&self.replacement, bindings)
    }

    fn substitute_elements(elements: &[ReplacementElement], bindings: &Bindings) -> Vec<Token> {
        let mut result = Vec::new();
        for elem in elements {
            match elem {
                ReplacementElement::Word(w) => result.push(Token::Word(w.clone())),
                ReplacementElement::Bracket(inner) => {
                    result.push(Token::Bracket(Self::substitute_elements(inner, bindings)));
                }
                ReplacementElement::Var(name) => {
                    if let Some(tokens) = bindings.get(name) {
                        result.extend(tokens.clone());
                    }
                }
                ReplacementElement::VarMany(name) => {
                    if let Some(tokens) = bindings.get(name) {
                        result.extend(tokens.clone());
                    }
                }
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BacktrackBudget {
    remaining: usize,
}

impl BacktrackBudget {
    fn new(max_backtracks: usize) -> Self {
        Self {
            remaining: max_backtracks,
        }
    }

    fn consume(&mut self) -> bool {
        if self.remaining == usize::MAX {
            return true;
        }
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// An optimizer that applies a set of rules repeatedly.
#[derive(Debug, Clone)]
pub struct Optimizer {
    rules: Vec<Rule>,
    max_backtracks: usize,
}

impl Optimizer {
    /// Creates a new optimizer with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            max_backtracks: 100_000,
        }
    }

    /// Adds a rewrite rule.
    pub fn add_rule(&mut self, pattern: &str, replacement: &str) -> Result<(), RuleError> {
        self.rules.push(Rule::new(pattern, replacement)?);
        Ok(())
    }

    /// Set the per-optimize backtracking budget used by `$*name` pattern matching.
    pub fn set_max_backtracks(&mut self, max_backtracks: usize) {
        self.max_backtracks = max_backtracks;
    }

    /// Returns the current per-optimize backtracking budget.
    pub fn max_backtracks(&self) -> usize {
        self.max_backtracks
    }

    /// Applies rules until no more changes occur.
    pub fn optimize(&self, tokens: Vec<Token>) -> Vec<Token> {
        let mut current = tokens;
        let mut seen = HashSet::new();
        seen.insert(program_hash(&current));
        loop {
            let mut changed = false;
            for rule in &self.rules {
                if let Some(rewritten) = rule.apply_with_limit(&current, self.max_backtracks) {
                    current = rewritten;
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
            if !seen.insert(program_hash(&current)) {
                break;
            }
        }
        current
    }
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

fn program_hash(tokens: &[Token]) -> u64 {
    let mut hasher = DefaultHasher::new();
    tokens.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_probe_rewrite() {
        let mut opt = Optimizer::new();
        opt.add_rule("BUILD PROBE", "DUP STATS SWAP BUILD [PUSHDOWN] DIP PROBE")
            .unwrap();

        let tokens = parse("A SCAN BUILD PROBE").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A SCAN DUP STATS SWAP BUILD [PUSHDOWN] DIP PROBE").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn dup_filter_commute() {
        let mut opt = Optimizer::new();
        opt.add_rule("DUP $X FILTER", "$X FILTER DUP").unwrap();

        let tokens = parse("A SCAN DUP [foo < 5] FILTER").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A SCAN [foo < 5] FILTER DUP").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_many_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("[$*X] UNWRAP", "$*X").unwrap();

        let tokens = parse("[A B C] UNWRAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn multiple_rules() {
        let mut opt = Optimizer::new();
        opt.add_rule("BUILD PROBE", "DUP STATS SWAP BUILD [PUSHDOWN] DIP PROBE")
            .unwrap();
        opt.add_rule("DUP $X FILTER", "$X FILTER DUP").unwrap();

        let tokens = parse("A SCAN DUP [foo] FILTER BUILD PROBE").unwrap();
        let result = opt.optimize(tokens);

        let expected =
            parse("A SCAN [foo] FILTER DUP DUP STATS SWAP BUILD [PUSHDOWN] DIP PROBE").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn unbound_variable_error() {
        let result = Rule::new("A B", "$X C");
        assert!(matches!(result, Err(RuleError::UnboundVariable(_))));
        println!("Error: {result:?}");
    }

    #[test]
    fn no_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BUILD PROBE", "REWRITTEN").unwrap();

        let tokens = parse("A SCAN FILTER").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {result:?}");
    }

    #[test]
    fn nested_bracket_pattern() {
        let mut opt = Optimizer::new();
        opt.add_rule("[INNER $X] OUTER", "RESULT $X").unwrap();

        let tokens = parse("[INNER foo] OUTER").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("RESULT foo").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_matches_word() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X DUP", "$X $X").unwrap();

        let tokens = parse("A DUP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_matches_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X EXEC", "DONE").unwrap();

        let tokens = parse("[A B C] EXEC").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("DONE").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_many_empty_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "WRAPPED").unwrap();

        let tokens = parse("BEGIN END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("WRAPPED").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_many_single_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "[$*X]").unwrap();

        let tokens = parse("BEGIN A END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[A]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_many_greedy() {
        let mut opt = Optimizer::new();
        opt.add_rule("$*X LAST", "[$*X]").unwrap();

        let tokens = parse("A B C LAST").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[A B C]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn match_at_beginning() {
        let mut opt = Optimizer::new();
        opt.add_rule("START", "BEGIN").unwrap();

        let tokens = parse("START A B").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("BEGIN A B").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn match_at_end() {
        let mut opt = Optimizer::new();
        opt.add_rule("END", "FINISH").unwrap();

        let tokens = parse("A B END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B FINISH").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn match_in_middle() {
        let mut opt = Optimizer::new();
        opt.add_rule("X Y", "Z").unwrap();

        let tokens = parse("A X Y B").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A Z B").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn repeated_application() {
        let mut opt = Optimizer::new();
        opt.add_rule("INC", "1 +").unwrap();

        let tokens = parse("INC INC INC").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("1 + 1 + 1 +").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn no_infinite_loop() {
        let mut opt = Optimizer::new();
        opt.add_rule("A B", "B A").unwrap();

        let tokens = parse("A B").unwrap();
        let result = opt.optimize(tokens);

        // Should swap once then stop (B A doesn't match A B)
        let expected = parse("B A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn cycle_detection_stops_oscillation() {
        let mut opt = Optimizer::new();
        opt.add_rule("A", "B").unwrap();
        opt.add_rule("B", "A").unwrap();

        let tokens = parse("A").unwrap();
        let result = opt.optimize(tokens.clone());
        assert_eq!(result, tokens);
    }

    #[test]
    fn multiple_vars() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X $Y SWAP", "$Y $X").unwrap();

        let tokens = parse("A B SWAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("B A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn repeated_var_must_match_same_token() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X $X PAIR", "MATCHED").unwrap();

        let tokens = parse("A A PAIR").unwrap();
        let result = opt.optimize(tokens);
        let expected = parse("MATCHED").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn repeated_var_rejects_mismatch() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X $X PAIR", "MATCHED").unwrap();

        let tokens = parse("A B PAIR").unwrap();
        let result = opt.optimize(tokens.clone());
        assert_eq!(result, tokens);
    }

    #[test]
    fn repeated_var_many_rejects_mismatch() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X MID $*X END", "MATCHED").unwrap();

        let tokens = parse("BEGIN A MID B END").unwrap();
        let result = opt.optimize(tokens.clone());
        assert_eq!(result, tokens);
    }

    #[test]
    fn repeated_var_many_accepts_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X MID $*X END", "MATCHED").unwrap();

        let tokens = parse("BEGIN A B MID A B END").unwrap();
        let result = opt.optimize(tokens);
        let expected = parse("MATCHED").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn var_used_multiple_times() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X TRIPLE", "$X $X $X").unwrap();

        let tokens = parse("FOO TRIPLE").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("FOO FOO FOO").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn bracket_literal_in_pattern() {
        let mut opt = Optimizer::new();
        opt.add_rule("[A B] MATCH", "FOUND").unwrap();

        let tokens = parse("[A B] MATCH").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("FOUND").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn bracket_literal_no_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("[A B] MATCH", "FOUND").unwrap();

        let tokens = parse("[A C] MATCH").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_in_replacement_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X WRAP", "[$X]").unwrap();

        let tokens = parse("FOO WRAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[FOO]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn var_many_in_replacement_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "[BLOCK $*X]").unwrap();

        let tokens = parse("BEGIN A B C END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[BLOCK A B C]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn nested_var_many() {
        let mut opt = Optimizer::new();
        opt.add_rule("[OUTER [$*X]] FLATTEN", "$*X").unwrap();

        let tokens = parse("[OUTER [A B C]] FLATTEN").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn empty_pattern_never_matches() {
        // An empty pattern would match everywhere and cause issues
        // The rule creation should succeed, but it won't match anything meaningful
        let mut opt = Optimizer::new();
        opt.add_rule("", "").unwrap();

        let tokens = parse("A B C").unwrap();
        let result = opt.optimize(tokens.clone());

        // Empty pattern matches at position 0 with 0 tokens, replaces with nothing
        // This is degenerate but shouldn't crash
        assert_eq!(result, tokens);
        println!("Result: {result:?}");
    }

    #[test]
    fn pattern_longer_than_input() {
        let mut opt = Optimizer::new();
        opt.add_rule("A B C D E", "MATCHED").unwrap();

        let tokens = parse("A B").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {result:?}");
    }

    #[test]
    fn two_var_many_greedy_first() {
        let mut opt = Optimizer::new();
        opt.add_rule("$*X MID $*Y", "[$*X] [$*Y]").unwrap();

        let tokens = parse("A B MID C D").unwrap();
        let result = opt.optimize(tokens);

        // Greedy: first $*X captures as much as possible (A B)
        let expected = parse("[A B] [C D]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn rule_order_matters() {
        let mut opt = Optimizer::new();
        opt.add_rule("A", "FIRST").unwrap();
        opt.add_rule("A", "SECOND").unwrap();

        let tokens = parse("A").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("FIRST").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn unbound_var_many_error() {
        let result = Rule::new("A", "$*X");
        assert!(matches!(result, Err(RuleError::UnboundVariable(_))));
        println!("Error: {result:?}");
    }

    #[test]
    fn invalid_variable_name_error() {
        let result = Rule::new("$", "A");
        assert!(matches!(result, Err(RuleError::InvalidVariableName(_))));
    }

    #[test]
    fn mixed_arity_variable_error() {
        let result = Rule::new("$X", "$*X");
        assert!(matches!(result, Err(RuleError::VariableArityMismatch(_))));
    }

    #[test]
    fn backtrack_limit_can_block_expensive_match() {
        let mut opt = Optimizer::new();
        opt.set_max_backtracks(1);
        opt.add_rule("$*X END", "[$*X]").unwrap();

        let tokens = parse("A B C END").unwrap();
        let result = opt.optimize(tokens.clone());
        assert_eq!(result, tokens);
    }

    #[test]
    fn preserve_tokens_before_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("X", "Y").unwrap();

        let tokens = parse("A B C X D E").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C Y D E").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn preserve_tokens_after_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("X Y", "Z").unwrap();

        let tokens = parse("X Y A B C").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("Z A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }

    #[test]
    fn fixpoint_multiple_applications() {
        let mut opt = Optimizer::new();
        opt.add_rule("DOUBLE $X", "$X $X").unwrap();

        let tokens = parse("DOUBLE DOUBLE A").unwrap();
        let result = opt.optimize(tokens);

        // DOUBLE DOUBLE A: match at pos 1 ($X=A) -> DOUBLE A A
        // DOUBLE A A: match at pos 0 ($X=A) -> A A A
        // A A A: no match
        let expected = parse("A A A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {result:?}");
    }
}
