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
use crate::types::StackTy;
use crate::types::Ty;
use crate::types::TypingFrame;
use crate::types::UnifyError;
use crate::types::WordTy;
use crate::types::core_scheme;
use crate::types::is_bool_literal;
use crate::types::is_numeric_literal;
use crate::types::respan_word;

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
    /// A word demands more stack values than are available — an **arity
    /// underflow** (§7, §12 M2). Names the offending word and gives the counts
    /// (`expected` values demanded vs `found` available) so the diagnostic is
    /// actionable; leaks no internal variable names (§7).
    Arity {
        /// The offending word.
        word: String,
        /// The span where it appears.
        span: Span,
        /// How many stack values the word consumes.
        expected: usize,
        /// How many were available below it.
        found: usize,
    },
    /// Two types failed to unify during inference, or a cyclic type was formed.
    /// Wraps the unifier's [`UnifyError`], which already carries the provenance
    /// pair (both origin spans) and never leaks internal variable names (§7).
    Mismatch(UnifyError),
    /// A definition refers to itself (directly or transitively) during the M2
    /// inline-inference of definition bodies. Real recursion is the SCC pass's
    /// job (M3); until then a recursive reference is rejected cleanly rather than
    /// looping forever.
    RecursiveDefinition {
        /// The definition caught referencing itself.
        name: String,
        /// The span of the offending reference.
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
            TypeError::Arity {
                word,
                span,
                expected,
                found,
            } => {
                write!(
                    f,
                    "stack underflow at `{word}` (byte {}): it needs {expected} value(s) but only {found} are available",
                    span.start
                )
            }
            TypeError::Mismatch(err) => write!(f, "{err}"),
            TypeError::RecursiveDefinition { name, span } => {
                write!(
                    f,
                    "recursive definition `{name}` at byte {}: mutual/self recursion is resolved by the SCC pass (not yet available)",
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
/// Returns the **inferred whole-program effect** (§5): the composition of every
/// word's stack arrow in `main`'s body, unified left-to-right, then required to
/// close against the empty initial stack. A program that demands inputs from the
/// empty stack underflows and is rejected (§12 M2).
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
    let mut locals: Vec<Local> = Vec::new();
    // `entry` is on the visiting stack so a `main` (or named entry) that calls
    // itself is caught as recursion (M3 territory) rather than looping.
    let mut visiting: Vec<String> = vec![entry.to_string()];

    // The program is the **top-level** sequence: the initial stack is empty
    // (§12), so any word demanding values from below the empty floor is an
    // underflow, attributed to that word (§7).
    let effect = infer_seq(evaluator, body, &mut ctx, &mut locals, &mut visiting, true)?;

    Ok(ctx.resolve_word_deep(&effect))
}

/// A named local (`>name`) and its monomorphic element type (§5). Each use of the
/// local yields this *same* `Ty` so all occurrences unify together.
struct Local {
    name: String,
    ty: Ty,
}

/// Infer the stack effect of a token sequence by **composition** (§5): start from
/// the identity arrow `( 'a -- 'a )` over one fresh shared row, and for each word
/// unify the accumulator's output against the word's input, then advance the
/// accumulator's output to the word's output. The sequence's effect is the
/// composition of its words.
///
/// `top_level` is `true` only for the whole-program entry, whose initial stack is
/// the *empty* stack (§12). At top level a word that consumes more values than
/// are currently available underflows and is rejected, named (§7); inside a
/// quotation or definition body the row tail is polymorphic, so consuming from it
/// is ordinary inference, not an error.
fn infer_seq<T>(
    evaluator: &Evaluator<T>,
    tokens: &[SpannedToken],
    ctx: &mut InferCtx,
    locals: &mut Vec<Local>,
    visiting: &mut Vec<String>,
    top_level: bool,
) -> Result<WordTy, TypeError>
where
    T: Quotable,
{
    // The identity arrow over one fresh shared row (§5). Both ends share the row
    // so "whatever is underneath" threads through untouched.
    let span = tokens
        .first()
        .map(|t| t.span)
        .unwrap_or(Span { start: 0, end: 0 });
    let row = ctx.fresh_row();
    let mut acc = WordTy::new(StackTy::empty(row, span), StackTy::empty(row, span));

    for token in tokens {
        // The per-token effect, and (for words) the name to blame on underflow.
        let (word_arrow, blame): (WordTy, Option<&str>) = match &token.kind {
            SpannedTokenKind::Word(w) => (
                word_effect(evaluator, w, token.span, ctx, locals, visiting)?,
                Some(w),
            ),
            SpannedTokenKind::Bracket(inner) => {
                // Descending into a quotation pushes a typing frame: its real
                // span and the effect expected at entry (§3 invariant 3, the
                // durable provenance spine).
                let frame_span = token.span;
                let expected = ctx.empty_program_effect(frame_span);
                ctx.frames.push(TypingFrame {
                    span: frame_span,
                    expected,
                });
                // A quotation opens a fresh lexical scope for locals and is a
                // *value*, not run here — so it is inferred as a non-top-level
                // body (its row tail is polymorphic).
                let mut inner_locals: Vec<Local> = Vec::new();
                let result = infer_seq(evaluator, inner, ctx, &mut inner_locals, visiting, false);
                ctx.frames.pop();
                let body_arrow = result?;
                // Quotation literal: ( 'a -- 'a Quote(P -- Q) ) (§5).
                let qrow = ctx.fresh_row();
                let quote = Ty::quote(body_arrow, frame_span);
                let arrow = WordTy::new(
                    StackTy::empty(qrow, frame_span),
                    StackTy::new(vec![quote], qrow, frame_span),
                );
                (arrow, None)
            }
        };

        // At top level the initial stack is empty (§12), so the program's
        // *before* stack — the accumulator input — must stay empty. Record its
        // current demand so we can attribute any increase to the word that
        // caused it (§7).
        let floor_before = if top_level {
            ctx.resolve_stack(&acc.input).elems.len()
        } else {
            0
        };

        // Compose: unify the accumulator's output against this word's input, then
        // advance the output to the word's output (§5).
        ctx.unify_stack(&acc.output, &word_arrow.input)
            .map_err(TypeError::Mismatch)?;
        acc = WordTy::new(acc.input, word_arrow.output);

        // Top-level underflow: if composing this word raised the program's
        // before-stack demand (it pulled values from below the empty floor —
        // either directly, like `DROP`, or via a combinator whose quotation
        // demands inputs, like `[ 1 + ] CALL`), reject it, naming the word and
        // giving counts (§7, §12 M2). A word inside a quotation/definition body
        // is exempt: there the row tail is genuinely polymorphic.
        if top_level && let Some(word) = blame {
            let floor_after = ctx.resolve_stack(&acc.input).elems.len();
            if floor_after > floor_before {
                return Err(TypeError::Arity {
                    word: word.to_string(),
                    span: token.span,
                    expected: floor_after,
                    found: floor_before,
                });
            }
        }
    }

    Ok(acc)
}

/// The stack effect of a single non-bracket word (§5): a binding `>name`, a use
/// of a bound local, a numeric or boolean literal, a registered operator, a
/// language-core primitive, or a definition (inlined for M2).
fn word_effect<T>(
    evaluator: &Evaluator<T>,
    w: &str,
    span: Span,
    ctx: &mut InferCtx,
    locals: &mut Vec<Local>,
    visiting: &mut Vec<String>,
) -> Result<WordTy, TypeError>
where
    T: Quotable,
{
    // `>name` : ( 'a t -- 'a ), introducing `name : t` (fresh `t`) for the rest
    // of the scope; the local is monomorphic (§5).
    if let Some(name) = bind_target(w) {
        let r = ctx.fresh_row();
        let t = Ty::var(ctx.fresh_ty(), span);
        let input = StackTy::new(vec![t.clone()], r, span);
        let output = StackTy::empty(r, span);
        locals.push(Local {
            name: name.to_string(),
            ty: t,
        });
        return Ok(WordTy::new(input, output));
    }

    // A use of a bound local: ( 'a -- 'a t ) with the *same* `t` every time
    // (innermost binding wins on shadowing) (§5).
    if let Some(local) = locals.iter().rev().find(|l| l.name == w) {
        let t = local.ty.clone();
        let r = ctx.fresh_row();
        return Ok(WordTy::new(
            StackTy::empty(r, span),
            StackTy::new(vec![Ty { kind: t.kind, span }], r, span),
        ));
    }

    // Numeric literal: ( 'a -- 'a Num ) (§5).
    if is_numeric_literal(w) {
        let r = ctx.fresh_row();
        return Ok(WordTy::new(
            StackTy::empty(r, span),
            StackTy::new(vec![Ty::num(span)], r, span),
        ));
    }

    // Boolean literal: ( 'a -- 'a Bool ) — the §2 `IF` scheme demands a Bool.
    if is_bool_literal(w) {
        let r = ctx.fresh_row();
        return Ok(WordTy::new(
            StackTy::empty(r, span),
            StackTy::new(vec![Ty::bool(span)], r, span),
        ));
    }

    // A registered operator (embedder attestation): instantiate its scheme with
    // fresh vars and re-anchor the spans at this call site (§5 `lookup`).
    if let Some(scheme) = evaluator.contract(w) {
        let scheme = scheme.clone();
        let inst = ctx.instantiate(&scheme);
        return Ok(respan_word(&inst, span));
    }

    // A language-core primitive (DUP/DROP/SWAP/OVER/CALL/IF), looked up under
    // its runtime UPPER_SNAKE_CASE spelling. The spec uses runtime spelling
    // directly; there is no lowercase translation layer.
    if let Some(scheme) = core_scheme(w) {
        let inst = ctx.instantiate(&scheme);
        return Ok(respan_word(&inst, span));
    }

    // A definition in the flat global namespace. For M2 we infer its body inline
    // (monomorphic); the SCC pass (M3) replaces this with proper generalization.
    // A `visiting` guard turns recursion into a clean error instead of a loop.
    if evaluator.has_definition(w) {
        if visiting.iter().any(|n| n == w) {
            return Err(TypeError::RecursiveDefinition {
                name: w.to_string(),
                span,
            });
        }
        let body = evaluator
            .definition_body_spanned(w)
            .expect("has_definition implies a spanned body when loaded with spans");
        visiting.push(w.to_string());
        let mut body_locals: Vec<Local> = Vec::new();
        let body_arrow = infer_seq(evaluator, body, ctx, &mut body_locals, visiting, false);
        visiting.pop();
        return body_arrow;
    }

    Err(TypeError::UnresolvedWord {
        word: w.to_string(),
        span,
    })
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
    use crate::types::TyKind;

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

    /// Infer the principal effect of a bare snippet (no `:main` wrapper, not the
    /// top-level program), with `+` registered. This is the §5 sequence-inference
    /// entry the M2 "infers ..." acceptance cases exercise: it returns the
    /// composed, fully-resolved arrow.
    fn infer_snippet(src: &str) -> Result<WordTy, TypeError> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        let tokens = parse_with_spans(src).unwrap();
        let mut ctx = InferCtx::new();
        let mut locals: Vec<Local> = Vec::new();
        let mut visiting: Vec<String> = Vec::new();
        let arrow = infer_seq(&eval, &tokens, &mut ctx, &mut locals, &mut visiting, false)?;
        Ok(ctx.resolve_word_deep(&arrow))
    }

    #[test]
    fn locals_infer_a_monomorphic_type() {
        // `>x x x +`: pop the top as a fresh monomorphic `t`, push it twice, add.
        // The two uses share `t`, and `+` forces `t = Num`, so the inferred
        // effect is ( 'a Num -- 'a Num ) (§5 named locals).
        let arrow = infer_snippet(">x x x +").unwrap();
        assert_eq!(arrow.input.elems.len(), 1, "consumes one value");
        assert_eq!(arrow.input.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.output.elems.len(), 1, "produces one value");
        assert_eq!(arrow.output.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.input.row, arrow.output.row, "same shared row tail");
    }

    // ----- §12 M2 acceptance: sequence inference -----

    #[test]
    fn m2_one_two_plus_infers_a_to_a_num() {
        // `1 2 +` infers ( 'a -- 'a Num ).
        let arrow = infer_snippet("1 2 +").unwrap();
        assert!(arrow.input.elems.is_empty(), "consumes nothing");
        assert_eq!(arrow.output.elems.len(), 1);
        assert_eq!(arrow.output.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.input.row, arrow.output.row, "same shared row tail");
    }

    #[test]
    fn m2_dup_infers_a_b_to_a_b_b() {
        // `DUP` infers ( 'a b -- 'a b b ): one element in, the same element twice.
        let arrow = infer_snippet("DUP").unwrap();
        assert_eq!(arrow.input.elems.len(), 1);
        assert_eq!(arrow.output.elems.len(), 2);
        // The lone input variable is duplicated: both outputs are the same var.
        let v = &arrow.input.elems[0].kind;
        assert!(matches!(v, TyKind::Var(_)));
        assert_eq!(arrow.output.elems[0].kind, *v);
        assert_eq!(arrow.output.elems[1].kind, *v);
        assert_eq!(arrow.input.row, arrow.output.row, "same shared row tail");
    }

    #[test]
    fn m2_quotation_literal_carries_its_inferred_arrow() {
        // `[ 1 + ]` infers ( 'a -- 'a (P Num -- P Num) ): the quote value carries
        // the arrow inferred for its body.
        let arrow = infer_snippet("[ 1 + ]").unwrap();
        assert!(arrow.input.elems.is_empty());
        assert_eq!(arrow.output.elems.len(), 1);
        match &arrow.output.elems[0].kind {
            TyKind::Quote(inner) => {
                // Body ( P Num -- P Num ): one Num in, one Num out, shared row.
                assert_eq!(inner.input.elems.len(), 1);
                assert_eq!(inner.input.elems[0].kind, TyKind::Con(NUM.into()));
                assert_eq!(inner.output.elems.len(), 1);
                assert_eq!(inner.output.elems[0].kind, TyKind::Con(NUM.into()));
                assert_eq!(inner.input.row, inner.output.row);
            }
            other => panic!("expected a Quote on the stack, got {other:?}"),
        }
        assert_eq!(arrow.input.row, arrow.output.row);
    }

    #[test]
    fn m2_quotation_then_call_infers_a_num_to_a_num() {
        // `[ 1 + ] CALL` infers ( 'a Num -- 'a Num ): running the quote consumes
        // and produces a Num.
        let arrow = infer_snippet("[ 1 + ] CALL").unwrap();
        assert_eq!(arrow.input.elems.len(), 1);
        assert_eq!(arrow.input.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.output.elems.len(), 1);
        assert_eq!(arrow.output.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.input.row, arrow.output.row);
    }

    #[test]
    fn m2_if_unifies_both_branches() {
        // `true [ 1 ] [ 2 ] IF` type-checks; both branches unify to a single
        // ( 'a -- 'a Num ) effect, so the whole thing infers ( 'a -- 'a Num ).
        let arrow = infer_snippet("true [ 1 ] [ 2 ] IF").unwrap();
        assert!(arrow.input.elems.is_empty(), "consumes nothing");
        assert_eq!(arrow.output.elems.len(), 1);
        assert_eq!(arrow.output.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(arrow.input.row, arrow.output.row);
    }

    #[test]
    fn m2_if_with_disagreeing_branches_is_a_mismatch() {
        // Branches producing different element types cannot unify against the
        // single ( 'S -- 'T ) the `IF` scheme shares — a typed mismatch (§7).
        let err = infer_snippet("true [ 1 ] [ true ] IF").unwrap_err();
        assert!(
            matches!(err, TypeError::Mismatch(_)),
            "disagreeing branches must be a typed mismatch, got {err:?}"
        );
    }

    #[test]
    fn m2_top_level_drop_underflows_naming_the_word() {
        // §12 M2 underflow: top-level `DROP` against the empty initial stack is
        // rejected with an arity error that NAMES the word and gives counts.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ DROP ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        match err {
            TypeError::Arity {
                word,
                expected,
                found,
                ..
            } => {
                assert_eq!(word, "DROP", "the arity error must name the word");
                assert_eq!(expected, 1);
                assert_eq!(found, 0);
            }
            other => panic!("expected an Arity underflow naming DROP, got {other:?}"),
        }
    }

    #[test]
    fn m2_top_level_call_demanding_input_underflows_naming_call() {
        // `[ 1 + ] CALL` infers ( 'a Num -- 'a Num ) (so it type-checks as a
        // sequence), but as a WHOLE PROGRAM against the empty stack it underflows:
        // running the quote needs a Num below it. The combinator-induced demand is
        // attributed to `CALL` (§7).
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        let tokens = parse_with_spans("[ [ 1 + ] CALL ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        match err {
            TypeError::Arity { word, found, .. } => {
                assert_eq!(word, "CALL");
                assert_eq!(found, 0);
            }
            other => panic!("expected an Arity underflow naming CALL, got {other:?}"),
        }
    }

    #[test]
    fn m2_arity_message_names_word_and_counts_no_var_leak() {
        // §7: the underflow message names the word, gives counts, and never leaks
        // an internal variable name.
        let err = TypeError::Arity {
            word: "DROP".into(),
            span: Span { start: 2, end: 6 },
            expected: 1,
            found: 0,
        };
        let text = err.to_string();
        assert!(text.contains("DROP"), "names the word: {text}");
        assert!(
            text.contains('1') && text.contains('0'),
            "gives counts: {text}"
        );
        assert!(!text.contains('\''), "no internal variable names: {text}");
    }

    #[test]
    fn m2_top_level_dup_drop_closes_against_empty_stack() {
        // A whole program that produces its own values before consuming them
        // closes against the empty *initial* stack: `1 DUP DROP` underflows
        // nowhere (DUP/DROP both see a value present). The program demands no
        // inputs, so its before-stack is empty.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ 1 DUP DROP ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let effect = type_check(&eval).unwrap();
        assert!(
            effect.input.elems.is_empty(),
            "consumes nothing from the empty initial stack"
        );
        // It leaves one Num behind (1 DUP DROP = one value): the after-stack is
        // a result, not required to be empty.
        assert_eq!(effect.output.elems.len(), 1);
        assert_eq!(effect.output.elems[0].kind, TyKind::Con(NUM.into()));
    }

    #[test]
    fn m2_core_scheme_rejects_lowercase_builtin_names() {
        // The spec and runtime both use UPPER_SNAKE_CASE names. Lowercase words
        // are ordinary unresolved words unless the user defines them.
        let err = infer_snippet("dup").unwrap_err();
        assert!(matches!(err, TypeError::UnresolvedWord { word, .. } if word == "dup"));
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
