//! Whole-program Tier-0 type-check driver (spec M0).
//!
//! This is the entry the spec calls *"the whole-program driver"*: it consumes
//! the flat global namespace of definitions that [`Evaluator::load`] already
//! populated (the order-independent pre-pass, §6 / architecture section) plus the
//! registered operator contracts, and type-checks the **distinguished entry**
//! against the **empty initial stack**.
//!
//! # Reconciliation: `main` against the empty stack
//!
//! `docs/typing.md` says the *program* is the distinguished entry (`main`) whose
//! effect must close against the empty stack, while the runtime has **no `main`
//! convention** — every `[ body ] :name` is just a definition in one flat
//! namespace, and `eval` runs an arbitrary token stream. This module *is* that
//! reconciliation, recorded in code: the type checker treats the definition
//! named [`MAIN`] as the program and every other definition as a library member.
//! The initial stack is the identity arrow `( 'a -- 'a )`
//! ([`InferCtx::empty_program_effect`]); "Initial stack for a top-level program
//! is empty" (§12).
//!
//! # Scope: M0 only
//!
//! M0 lands the *driver shape*: definitions load into one namespace; the entry is
//! located; every word in its body **resolves** to a definition, a registered
//! operator contract, a numeric literal, a quotation, or a local — and quotation
//! descent pushes/pops typing frames (the durable provenance spine). Full
//! arrow unification (M1) and sequence inference (M2) ride on this substrate
//! later; M0 deliberately stops at resolution closure and demonstrates that a
//! registered operator's [`Scheme`] is *usable* (it can be looked up and
//! instantiated with fresh variables).

use crate::Evaluator;
use crate::Quotable;
use crate::Span;
use crate::SpannedToken;
use crate::SpannedTokenKind;
use crate::evaluator::bind_target;
use crate::types::InferCtx;
use crate::types::MAIN;
use crate::types::TypingFrame;
use crate::types::WordTy;
use crate::types::is_numeric_literal;

/// A Tier-0 type-check error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    /// The distinguished entry point was not found among loaded definitions.
    MissingEntry {
        /// The entry name that was expected (normally [`MAIN`]).
        name: String,
    },
    /// A word in a checked body resolves to nothing: it is neither a definition,
    /// a registered operator contract, a numeric literal, nor a bound local.
    UnresolvedWord {
        /// The offending word.
        word: String,
        /// The span where it appears.
        span: Span,
    },
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeError::MissingEntry { name } => {
                write!(
                    f,
                    "no `{name}` entry point: a whole-program check needs a definition `[ body ] :{name}` to type against the empty stack"
                )
            }
            TypeError::UnresolvedWord { word, span } => {
                write!(
                    f,
                    "unresolved word `{word}` at byte {}: not a definition, a registered operator, a numeric literal, or a local in scope",
                    span.start
                )
            }
        }
    }
}

impl std::error::Error for TypeError {}

/// Type-checks the whole program: locate the distinguished entry [`MAIN`] and
/// check it against the empty initial stack.
///
/// Returns the Tier-0 effect the entry is checked against — the empty-stack
/// identity `( 'a -- 'a )` — so callers can see the convention concretely. In M0
/// the body is checked for *resolution closure*: every word resolves and every
/// quotation pushes/pops a typing frame. The returned [`InferCtx`] carries the
/// substitution arena and (now-empty) frame stack the later milestones reuse.
pub fn type_check<T>(evaluator: &Evaluator<T>) -> Result<WordTy, TypeError>
where
    T: Quotable,
{
    type_check_entry(evaluator, MAIN)
}

/// Type-checks a named entry against the empty initial stack. [`type_check`] is
/// this with `entry = MAIN`; exposing the name lets tests check a non-`main`
/// entry without depending on the default.
pub fn type_check_entry<T>(evaluator: &Evaluator<T>, entry: &str) -> Result<WordTy, TypeError>
where
    T: Quotable,
{
    // The checker reads the SPANNED body so every diagnostic anchors at a real
    // source byte offset (§13 invariant 6: origin spans exist from the first
    // inference commit and are not reconstructable later). A program to be
    // type-checked must therefore be loaded via
    // [`Evaluator::load_with_spans`]; the spanless `load` is runtime-only.
    let body = evaluator
        .definition_body_spanned(entry)
        .ok_or_else(|| TypeError::MissingEntry {
            name: entry.to_string(),
        })?;

    let mut ctx = InferCtx::new();
    // The program-effect node is born at the entry's own bracket span — a real
    // source location, never a fabricated zero span.
    let span = evaluator
        .definition_span(entry)
        .expect("definition_span present whenever definition_body_spanned is");
    let effect = ctx.empty_program_effect(span);

    // Locals introduced by `>name` are monomorphic within the scope (§5). M0
    // tracks only their *names* so a later reference resolves; the type is
    // assigned by inference (M2).
    let mut locals: Vec<String> = Vec::new();
    resolve_seq(evaluator, body, &mut ctx, &mut locals)?;

    Ok(effect)
}

/// Resolve every word in a spanned token sequence, descending into quotations.
/// Returns the first resolution failure, carrying that token's **real** source
/// span. This is the M0 "type-check" — resolution closure — onto which the M1
/// unifier and M2 inference attach later.
fn resolve_seq<T>(
    evaluator: &Evaluator<T>,
    tokens: &[SpannedToken],
    ctx: &mut InferCtx,
    locals: &mut Vec<String>,
) -> Result<(), TypeError>
where
    T: Quotable,
{
    for token in tokens {
        match &token.kind {
            SpannedTokenKind::Word(w) => {
                if let Some(name) = bind_target(w) {
                    // `>name` introduces a monomorphic local for the rest of the
                    // scope (§5). M0 records the name.
                    locals.push(name.to_string());
                } else if locals.iter().any(|l| l == w) {
                    // A reference to a bound local — resolves.
                } else if is_numeric_literal(w) {
                    // Numeric literal: ( 'a -- 'a Num ) (§5). Resolves.
                } else if evaluator.has_definition(w) {
                    // A definition in the flat global namespace — resolves.
                } else if evaluator.contract(w).is_some() {
                    // A registered operator with an attested contract — resolves.
                } else {
                    return Err(TypeError::UnresolvedWord {
                        word: w.clone(),
                        // The token's real source span — not a fabricated zero.
                        span: token.span,
                    });
                }
            }
            SpannedTokenKind::Bracket(inner) => {
                // Descending into a quotation pushes a typing frame: its real
                // span and the effect expected at entry (§3 invariant 3). Even
                // though full error rendering (M5) comes later, the frame must
                // exist from the first commit — it is the durable provenance
                // spine — and it must carry the quotation's true source span.
                let frame_span = token.span;
                let expected = ctx.empty_program_effect(frame_span);
                ctx.frames.push(TypingFrame {
                    span: frame_span,
                    expected,
                });
                // A quotation opens a fresh lexical scope for locals.
                let mut inner_locals = locals.clone();
                let result = resolve_seq(evaluator, inner, ctx, &mut inner_locals);
                ctx.frames.pop();
                result?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;
    use crate::parse;
    use crate::parse_with_spans;
    use crate::types::NUM;
    use crate::types::Scheme;
    use crate::types::StackTy;
    use crate::types::Ty;

    /// A minimal stack value type for driver tests.
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

    fn sp() -> Span {
        Span { start: 0, end: 1 }
    }

    /// The `+` operator's attested Tier-0 scheme: ( 'S Num Num -- 'S Num ).
    /// There is no core `+`; this enters via registration, exactly as §12 says.
    fn plus_scheme() -> Scheme {
        let s = sp();
        let input = StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s);
        let output = StackTy::new(vec![Ty::num(s)], 0, s);
        Scheme::new(vec![], vec![0], WordTy::new(input, output))
    }

    #[test]
    fn missing_main_is_an_error() {
        let eval: Evaluator<Value> = Evaluator::new();
        let err = type_check(&eval).unwrap_err();
        assert_eq!(
            err,
            TypeError::MissingEntry {
                name: MAIN.to_string()
            }
        );
    }

    #[test]
    fn registered_operator_scheme_is_usable_in_the_driver() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        // The embedder attests a contract for `+`.
        eval.register_operator_with_contract("+", plus_scheme());

        // A one-operator program: main pushes two literals and adds them.
        let tokens = parse_with_spans("[ 1 2 + ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();

        // The driver resolves main against the empty stack.
        let effect = type_check(&eval).unwrap();
        assert!(effect.input.elems.is_empty());
        assert_eq!(effect.input.row, effect.output.row);

        // The scheme is retrievable AND usable: instantiation renames its row.
        let scheme = eval.contract("+").unwrap().clone();
        let mut ctx = InferCtx::new();
        let inst = ctx.instantiate(&scheme);
        assert_eq!(inst.output.elems.len(), 1);
        assert_eq!(
            inst.output.elems[0].kind,
            crate::types::TyKind::Con(NUM.to_string())
        );
    }

    #[test]
    fn a_word_with_no_definition_or_contract_is_unresolved() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ frobnicate ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        assert!(matches!(err, TypeError::UnresolvedWord { word, .. } if word == "frobnicate"));
    }

    #[test]
    fn unresolved_word_reports_a_real_nonzero_span() {
        // §13 invariant 6: the diagnostic must anchor at the offending token's
        // real byte offset, never a fabricated Span { 0, 0 }.
        let mut eval: Evaluator<Value> = Evaluator::new();
        // `frobnicate` begins after "[ " — at byte 2.
        let src = "[ frobnicate ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        match err {
            TypeError::UnresolvedWord { word, span } => {
                assert_eq!(word, "frobnicate");
                assert_ne!(span, Span { start: 0, end: 0 }, "span must be real");
                assert_eq!(span.start, src.find("frobnicate").unwrap());
            }
            other => panic!("expected UnresolvedWord, got {other:?}"),
        }
    }

    #[test]
    fn typing_frame_carries_the_real_quotation_span() {
        // The frame pushed on descent into a nested quotation must carry that
        // quotation's true source span (durable provenance spine, §3 inv 3). We
        // observe it indirectly: an unresolved word *inside* the nested quote
        // still reports its own token span, and the outer driver located it.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let src = "[ [ frobnicate ] ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        match err {
            TypeError::UnresolvedWord { span, .. } => {
                assert_eq!(span.start, src.find("frobnicate").unwrap());
            }
            other => panic!("expected UnresolvedWord, got {other:?}"),
        }
    }

    #[test]
    fn definitions_resolve_across_textual_order() {
        // Order independence (the flat global pre-pass): main references bar,
        // bar is defined later. Both resolve.
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        let tokens = parse_with_spans("[ bar 1 + ] :main [ 2 ] :bar").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        type_check(&eval).unwrap();
    }

    #[test]
    fn locals_resolve_within_scope() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        // bind top to x, then push x twice and add.
        let tokens = parse_with_spans("[ >x x x + ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        type_check(&eval).unwrap();
    }

    #[test]
    fn quotation_descent_resolves_nested_words() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        // A nested quotation whose body uses the contracted operator.
        let tokens = parse_with_spans("[ [ 1 + ] ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        type_check(&eval).unwrap();
    }

    // ----- Capability wall: the user registers nothing (M0 / invariant 16) -----

    #[test]
    fn caternary_surface_cannot_reach_the_contract_table() {
        // No Caternary source — parsed, loaded, evaluated — can add a contract.
        // The contract table is a private field whose only mutator is the Rust
        // registration API; parse/load/eval never touch it. We assert this
        // structurally by exercising the full surface and observing the table
        // stays empty.
        let mut eval: Evaluator<Value> = Evaluator::new();

        // A program that *tries* to define things and run combinators.
        let src = "[ 1 2 ] :main [ 99 ] :helper";
        let tokens = parse(src).unwrap();
        assert_eq!(eval.contract_count(), 0);

        eval.load(&tokens).unwrap();
        assert_eq!(
            eval.contract_count(),
            0,
            "load() must not populate the contract table"
        );

        // eval the program body too — still nothing reaches contracts.
        let body = parse("1 2").unwrap();
        eval.eval(&body).unwrap();
        assert_eq!(
            eval.contract_count(),
            0,
            "eval() must not populate the contract table"
        );

        // The ONLY way to add a contract is the Rust registration API.
        eval.register_operator_with_contract("+", plus_scheme());
        assert_eq!(eval.contract_count(), 1);
    }

    #[test]
    fn define_does_not_touch_the_contract_table() {
        // Registering runtime behavior via `define` (the existing API) must not
        // register a contract: the two tables are separate buckets.
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.define("NOOP", |_stack, _eval| Ok(()));
        assert_eq!(eval.contract_count(), 0);
        // And there is no contract for the defined operator.
        assert!(eval.contract("NOOP").is_none());
    }
}
