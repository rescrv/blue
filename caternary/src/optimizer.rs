//! String-based pattern matching optimizer for caternary.
//!
//! Patterns use variables to match and capture tokens:
//! - `$name` matches exactly one token (word or bracket)
//! - `$*name` matches zero or more tokens
//!
//! Variables are interpolated into the replacement string.

use std::collections::HashMap;

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
    /// Failed to parse pattern or replacement.
    Parse(ParseError),
    /// Variable in replacement not bound in pattern.
    UnboundVariable(String),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::Parse(e) => write!(f, "parse error: {}", e),
            RuleError::UnboundVariable(name) => write!(f, "unbound variable: {}", name),
        }
    }
}

impl std::error::Error for RuleError {}

impl From<ParseError> for RuleError {
    fn from(e: ParseError) -> Self {
        RuleError::Parse(e)
    }
}

impl Rule {
    /// Creates a new rule from pattern and replacement strings.
    pub fn new(pattern: &str, replacement: &str) -> Result<Self, RuleError> {
        let pattern_tokens = parse(pattern)?;
        let replacement_tokens = parse(replacement)?;

        let pattern = Self::parse_pattern(&pattern_tokens);
        let replacement = Self::parse_replacement(&replacement_tokens);

        // Validate that all replacement variables are bound in pattern
        let bound = Self::collect_pattern_vars(&pattern);
        for var in Self::collect_replacement_vars(&replacement) {
            if !bound.contains(&var) {
                return Err(RuleError::UnboundVariable(var));
            }
        }

        Ok(Self {
            pattern,
            replacement,
        })
    }

    fn parse_pattern(tokens: &[Token]) -> Vec<PatternElement> {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(w) if w.starts_with("$*") => {
                    PatternElement::VarMany(w[2..].to_string())
                }
                Token::Word(w) if w.starts_with('$') => PatternElement::Var(w[1..].to_string()),
                Token::Word(w) => PatternElement::Word(w.clone()),
                Token::Bracket(inner) => PatternElement::Bracket(Self::parse_pattern(inner)),
            })
            .collect()
    }

    fn parse_replacement(tokens: &[Token]) -> Vec<ReplacementElement> {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(w) if w.starts_with("$*") => {
                    ReplacementElement::VarMany(w[2..].to_string())
                }
                Token::Word(w) if w.starts_with('$') => ReplacementElement::Var(w[1..].to_string()),
                Token::Word(w) => ReplacementElement::Word(w.clone()),
                Token::Bracket(inner) => {
                    ReplacementElement::Bracket(Self::parse_replacement(inner))
                }
            })
            .collect()
    }

    fn collect_pattern_vars(pattern: &[PatternElement]) -> Vec<String> {
        let mut vars = Vec::new();
        for elem in pattern {
            match elem {
                PatternElement::Var(name) | PatternElement::VarMany(name) => {
                    vars.push(name.clone())
                }
                PatternElement::Bracket(inner) => vars.extend(Self::collect_pattern_vars(inner)),
                PatternElement::Word(_) => {}
            }
        }
        vars
    }

    fn collect_replacement_vars(replacement: &[ReplacementElement]) -> Vec<String> {
        let mut vars = Vec::new();
        for elem in replacement {
            match elem {
                ReplacementElement::Var(name) | ReplacementElement::VarMany(name) => {
                    vars.push(name.clone())
                }
                ReplacementElement::Bracket(inner) => {
                    vars.extend(Self::collect_replacement_vars(inner))
                }
                ReplacementElement::Word(_) => {}
            }
        }
        vars
    }

    /// Attempts to apply this rule to a token sequence, returning the rewritten sequence if matched.
    /// Returns `None` if no match produces a different result.
    pub fn apply(&self, tokens: &[Token]) -> Option<Vec<Token>> {
        for start in 0..=tokens.len() {
            if let Some((end, bindings)) = self.match_at(tokens, start) {
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

    fn match_at(&self, tokens: &[Token], start: usize) -> Option<(usize, Bindings)> {
        let mut bindings = Bindings::new();
        let end = self.match_pattern(&self.pattern, tokens, start, &mut bindings)?;
        Some((end, bindings))
    }

    fn match_pattern(
        &self,
        pattern: &[PatternElement],
        tokens: &[Token],
        pos: usize,
        bindings: &mut Bindings,
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
                        let end = self.match_pattern(inner_pattern, inner_tokens, 0, bindings)?;
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
                    bindings.insert(name.clone(), vec![tokens[tok_idx].clone()]);
                    tok_idx += 1;
                    pat_idx += 1;
                }
                PatternElement::VarMany(name) => {
                    // Greedy: try matching as many as possible first, then fewer
                    let remaining_pattern = &pattern[pat_idx + 1..];
                    let available = tokens.len().saturating_sub(tok_idx);
                    for take in (0..=available).rev() {
                        let captured: Vec<Token> = tokens[tok_idx..tok_idx + take].to_vec();
                        let mut test_bindings = bindings.clone();
                        test_bindings.insert(name.clone(), captured);
                        if let Some(end) = self.match_pattern(
                            remaining_pattern,
                            tokens,
                            tok_idx + take,
                            &mut test_bindings,
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

    fn substitute(&self, bindings: &Bindings) -> Vec<Token> {
        self.substitute_elements(&self.replacement, bindings)
    }

    fn substitute_elements(
        &self,
        elements: &[ReplacementElement],
        bindings: &Bindings,
    ) -> Vec<Token> {
        let mut result = Vec::new();
        for elem in elements {
            match elem {
                ReplacementElement::Word(w) => result.push(Token::Word(w.clone())),
                ReplacementElement::Bracket(inner) => {
                    result.push(Token::Bracket(self.substitute_elements(inner, bindings)));
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

/// An optimizer that applies a set of rules repeatedly.
#[derive(Debug, Clone, Default)]
pub struct Optimizer {
    rules: Vec<Rule>,
}

impl Optimizer {
    /// Creates a new optimizer with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Adds a rewrite rule.
    pub fn add_rule(&mut self, pattern: &str, replacement: &str) -> Result<(), RuleError> {
        self.rules.push(Rule::new(pattern, replacement)?);
        Ok(())
    }

    /// Applies rules until no more changes occur.
    pub fn optimize(&self, tokens: Vec<Token>) -> Vec<Token> {
        let mut current = tokens;
        loop {
            let mut changed = false;
            for rule in &self.rules {
                if let Some(rewritten) = rule.apply(&current) {
                    current = rewritten;
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
        }
        current
    }
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
        println!("Result: {:?}", result);
    }

    #[test]
    fn dup_filter_commute() {
        let mut opt = Optimizer::new();
        opt.add_rule("DUP $X FILTER", "$X FILTER DUP").unwrap();

        let tokens = parse("A SCAN DUP [foo < 5] FILTER").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A SCAN [foo < 5] FILTER DUP").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_many_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("[$*X] UNWRAP", "$*X").unwrap();

        let tokens = parse("[A B C] UNWRAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
    }

    #[test]
    fn unbound_variable_error() {
        let result = Rule::new("A B", "$X C");
        assert!(matches!(result, Err(RuleError::UnboundVariable(_))));
        println!("Error: {:?}", result);
    }

    #[test]
    fn no_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BUILD PROBE", "REWRITTEN").unwrap();

        let tokens = parse("A SCAN FILTER").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {:?}", result);
    }

    #[test]
    fn nested_bracket_pattern() {
        let mut opt = Optimizer::new();
        opt.add_rule("[INNER $X] OUTER", "RESULT $X").unwrap();

        let tokens = parse("[INNER foo] OUTER").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("RESULT foo").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_matches_word() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X DUP", "$X $X").unwrap();

        let tokens = parse("A DUP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_matches_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X EXEC", "DONE").unwrap();

        let tokens = parse("[A B C] EXEC").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("DONE").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_many_empty_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "WRAPPED").unwrap();

        let tokens = parse("BEGIN END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("WRAPPED").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_many_single_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "[$*X]").unwrap();

        let tokens = parse("BEGIN A END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[A]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_many_greedy() {
        let mut opt = Optimizer::new();
        opt.add_rule("$*X LAST", "[$*X]").unwrap();

        let tokens = parse("A B C LAST").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[A B C]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn match_at_beginning() {
        let mut opt = Optimizer::new();
        opt.add_rule("START", "BEGIN").unwrap();

        let tokens = parse("START A B").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("BEGIN A B").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn match_at_end() {
        let mut opt = Optimizer::new();
        opt.add_rule("END", "FINISH").unwrap();

        let tokens = parse("A B END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B FINISH").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn match_in_middle() {
        let mut opt = Optimizer::new();
        opt.add_rule("X Y", "Z").unwrap();

        let tokens = parse("A X Y B").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A Z B").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn repeated_application() {
        let mut opt = Optimizer::new();
        opt.add_rule("INC", "1 +").unwrap();

        let tokens = parse("INC INC INC").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("1 + 1 + 1 +").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
    }

    #[test]
    fn multiple_vars() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X $Y SWAP", "$Y $X").unwrap();

        let tokens = parse("A B SWAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("B A").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_used_multiple_times() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X TRIPLE", "$X $X $X").unwrap();

        let tokens = parse("FOO TRIPLE").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("FOO FOO FOO").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn bracket_literal_in_pattern() {
        let mut opt = Optimizer::new();
        opt.add_rule("[A B] MATCH", "FOUND").unwrap();

        let tokens = parse("[A B] MATCH").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("FOUND").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn bracket_literal_no_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("[A B] MATCH", "FOUND").unwrap();

        let tokens = parse("[A C] MATCH").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_in_replacement_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("$X WRAP", "[$X]").unwrap();

        let tokens = parse("FOO WRAP").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[FOO]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn var_many_in_replacement_bracket() {
        let mut opt = Optimizer::new();
        opt.add_rule("BEGIN $*X END", "[BLOCK $*X]").unwrap();

        let tokens = parse("BEGIN A B C END").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("[BLOCK A B C]").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn nested_var_many() {
        let mut opt = Optimizer::new();
        opt.add_rule("[OUTER [$*X]] FLATTEN", "$*X").unwrap();

        let tokens = parse("[OUTER [A B C]] FLATTEN").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
    }

    #[test]
    fn pattern_longer_than_input() {
        let mut opt = Optimizer::new();
        opt.add_rule("A B C D E", "MATCHED").unwrap();

        let tokens = parse("A B").unwrap();
        let result = opt.optimize(tokens.clone());

        assert_eq!(result, tokens);
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
    }

    #[test]
    fn unbound_var_many_error() {
        let result = Rule::new("A", "$*X");
        assert!(matches!(result, Err(RuleError::UnboundVariable(_))));
        println!("Error: {:?}", result);
    }

    #[test]
    fn preserve_tokens_before_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("X", "Y").unwrap();

        let tokens = parse("A B C X D E").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("A B C Y D E").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
    }

    #[test]
    fn preserve_tokens_after_match() {
        let mut opt = Optimizer::new();
        opt.add_rule("X Y", "Z").unwrap();

        let tokens = parse("X Y A B C").unwrap();
        let result = opt.optimize(tokens);

        let expected = parse("Z A B C").unwrap();
        assert_eq!(result, expected);
        println!("Result: {:?}", result);
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
        println!("Result: {:?}", result);
    }
}
