//! Harness 1 — Type-Directed Term Generator (proptest).
//!
//! There is no reference implementation, so the oracle is the type system's own
//! claimed invariants. The generator manufactures that oracle — every term it
//! produces has a *known* type — turning "inferred == expected" from a
//! crash-check into a real assertion.
//!
//! ## The generator
//!
//! caternary's Tier-0 has a **single numeric type** (`Num`) plus `Bool`, and the
//! only words the *type checker* resolves are: numeric/boolean literals, the
//! registered scalar operators (`+ - * < > && || == != ~ not …`), and the
//! language-core primitives in [`caternary::core_scheme`]
//! (`DUP DROP SWAP OVER CALL IF DIP`). `ROT/NIP/TUCK/2DUP` are runtime-only and
//! have no Tier-0 scheme, so the generator never emits them.
//!
//! That base-value simplicity (no `Int`/`Float`/`Bool` axis to sample) lets the
//! generator be **concrete-stack** directed rather than effect-variable directed:
//! it simulates an abstract stack of base types (`Num`/`Bool`) starting from the
//! genuine top-level empty stack, only ever emitting a word whose precondition
//! the simulated stack satisfies. The simulated final stack is the oracle.
//! Because every position is a concrete `Con`, P1 needs no α-equivalence
//! normalization — the inferred output stack must be *exactly* the simulated one.
//!
//! ## The properties
//!
//! P1 (inference == oracle), P2 (substitution/resolution idempotence),
//! P3 (composition coherence, as a metamorphic "interface-only" relation),
//! P4 (generalization respects the environment), P5 (row neutrality),
//! P6 (definition-order independence). Where a property has no directly exposed
//! seam in caternary's public API (e.g. a standalone `generalize(τ, Γ)`), how it
//! is exercised instead is noted at the property.

use caternary::*;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// A minimal stack value type so `Evaluator<T>` can be instantiated. Mirrors the
// driver-test `Value` in `src/check.rs`.
// ---------------------------------------------------------------------------

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

fn new_eval() -> Evaluator<Value> {
    let mut eval: Evaluator<Value> = Evaluator::new();
    register_all_builtins(&mut eval);
    eval
}

/// Load `src` and infer the principal effect of `:main` against the empty stack.
fn checked_main(src: &str) -> Result<WordTy, TypeError> {
    let mut eval = new_eval();
    let toks = parse_with_spans(src).expect("generated source parses");
    eval.load_with_spans(&toks).expect("generated source loads");
    type_check(&eval)
}

// ---------------------------------------------------------------------------
// The type-directed term generator (concrete-stack form).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Base {
    Num,
    Bool,
}

/// One legal move given the current simulated stack. Carries everything needed to
/// emit its source token(s) and advance the simulated stack.
#[derive(Clone, Copy, Debug)]
enum Move {
    PushNum,
    PushBool,
    Dup,
    Drop,
    Swap,
    Over,
    Arith(&'static str), // Num Num -> Num
    Cmp(&'static str),   // Num Num -> Bool
    Logic(&'static str), // Bool Bool -> Bool
    Eq(&'static str),    // a a -> Bool  (operands same base type)
    NumNot,              // Num -> Num   (`~`)
    BoolNot,             // Bool -> Bool (`not`)
}

/// Every move whose precondition the current stack `st` (top-of-stack last)
/// satisfies. Literals are always legal; everything else demands enough operands
/// of the right base type — which is exactly the word's Tier-0 input shape.
fn moves_for(st: &[Base]) -> Vec<Move> {
    let n = st.len();
    let mut v = vec![Move::PushNum, Move::PushBool];
    if n >= 1 {
        v.push(Move::Dup);
        v.push(Move::Drop);
        match st[n - 1] {
            Base::Num => v.push(Move::NumNot),
            Base::Bool => v.push(Move::BoolNot),
        }
    }
    if n >= 2 {
        v.push(Move::Swap);
        v.push(Move::Over);
        let (a, b) = (st[n - 2], st[n - 1]);
        if a == Base::Num && b == Base::Num {
            v.push(Move::Arith("+"));
            v.push(Move::Arith("-"));
            v.push(Move::Arith("*"));
            v.push(Move::Cmp("<"));
            v.push(Move::Cmp(">"));
        }
        if a == Base::Bool && b == Base::Bool {
            v.push(Move::Logic("&&"));
            v.push(Move::Logic("||"));
        }
        if a == b {
            v.push(Move::Eq("=="));
            v.push(Move::Eq("!="));
        }
    }
    v
}

/// Apply a move to the simulated stack and return the source token(s) it emits.
fn run_move(m: Move, st: &mut Vec<Base>) -> Vec<String> {
    match m {
        Move::PushNum => {
            st.push(Base::Num);
            vec!["0".to_string()]
        }
        Move::PushBool => {
            st.push(Base::Bool);
            vec!["true".to_string()]
        }
        Move::Dup => {
            let top = *st.last().expect("DUP precondition: nonempty");
            st.push(top);
            vec!["DUP".to_string()]
        }
        Move::Drop => {
            st.pop();
            vec!["DROP".to_string()]
        }
        Move::Swap => {
            let n = st.len();
            st.swap(n - 1, n - 2);
            vec!["SWAP".to_string()]
        }
        Move::Over => {
            let second = st[st.len() - 2];
            st.push(second);
            vec!["OVER".to_string()]
        }
        Move::Arith(op) => {
            st.pop();
            st.pop();
            st.push(Base::Num);
            vec![op.to_string()]
        }
        Move::Cmp(op) | Move::Eq(op) | Move::Logic(op) => {
            st.pop();
            st.pop();
            st.push(Base::Bool);
            vec![op.to_string()]
        }
        Move::NumNot => vec!["~".to_string()],
        Move::BoolNot => vec!["not".to_string()],
    }
}

/// Interpret a script of choices into a token stream, advancing `st`. Each choice
/// selects among the moves legal at that point, so the produced program is
/// well-typed by construction and `st` is its known final stack — the oracle.
fn simulate(choices: &[u16], st: &mut Vec<Base>) -> Vec<String> {
    let mut tokens = Vec::new();
    for &c in choices {
        let moves = moves_for(st);
        let m = moves[(c as usize) % moves.len()];
        tokens.extend(run_move(m, st));
    }
    tokens
}

/// Emit literals that reconstruct an exact base-type stack (bottom to top).
fn emit_literals(st: &[Base]) -> Vec<String> {
    st.iter()
        .map(|b| match b {
            Base::Num => "0".to_string(),
            Base::Bool => "true".to_string(),
        })
        .collect()
}

/// The expected `Con` name for a base type.
fn base_con(b: Base) -> &'static str {
    match b {
        Base::Num => NUM,
        Base::Bool => BOOL,
    }
}

/// Assert that a resolved stack's observed elements are exactly `expected`
/// concrete base types (top-of-stack last).
fn elems_match(elems: &[Ty], expected: &[Base]) -> bool {
    if elems.len() != expected.len() {
        return false;
    }
    elems
        .iter()
        .zip(expected)
        .all(|(e, b)| matches!(&e.kind, TyKind::Con(name) if name == base_con(*b)))
}

/// A bounded choice script.
fn choice_script() -> impl Strategy<Value = Vec<u16>> {
    prop::collection::vec(any::<u16>(), 0..24)
}

// ---------------------------------------------------------------------------
// P1 — Inference agrees with the oracle.
// ---------------------------------------------------------------------------

proptest! {
    /// A type-directed term has a known final stack; inference must reproduce it
    /// exactly. The top-level input is empty and its row threads to the output
    /// row (the identity tail), and the output's observed elements are precisely
    /// the simulated base-type stack.
    #[test]
    fn p1_inference_agrees_with_oracle(script in choice_script()) {
        let mut st = Vec::new();
        let tokens = simulate(&script, &mut st);
        let src = format!("[ {} ] :main", tokens.join(" "));
        let effect = checked_main(&src).expect("type-directed term must type-check");
        prop_assert!(effect.input.elems.is_empty(), "top-level input is empty");
        prop_assert_eq!(effect.input.row, effect.output.row, "identity tail threads");
        prop_assert!(
            elems_match(&effect.output.elems, &st),
            "inferred output {:?} != oracle {:?} for `{}`",
            effect.output.elems,
            st,
            src
        );
    }
}

// ---------------------------------------------------------------------------
// P2 — Substitution / resolution is idempotent.
// ---------------------------------------------------------------------------

fn p2_ty() -> impl Strategy<Value = Ty> {
    let s = Span { start: 0, end: 1 };
    prop_oneof![
        Just(Ty::num(s)),
        Just(Ty::bool(s)),
        (0u32..6).prop_map(move |v| Ty::var(v, s)),
    ]
}

fn p2_stack() -> impl Strategy<Value = StackTy> {
    let s = Span { start: 0, end: 1 };
    (prop::collection::vec(p2_ty(), 0..3), 0u32..6)
        .prop_map(move |(elems, row)| StackTy::new(elems, row, s))
}

proptest! {
    /// The flat-map trap: after inference produces a substitution `s`, applying it
    /// must be a fixpoint — `apply(s, t) == apply(s, apply(s, t))`. caternary's
    /// `apply` is deep resolution ([`InferCtx::resolve_ty_deep`] /
    /// `resolve_stack_deep`). We build a substitution by unifying random stacks
    /// (ignoring failures — the arena stays valid), then assert deep resolution is
    /// idempotent on a battery of probe types drawn from the same var pool. A
    /// non-fixpoint here means a binding `α ↦ τ` survived with `α ∈ ftv(τ)`: the
    /// occurs check leaked, or a binding was not propagated.
    #[test]
    fn p2_resolution_is_idempotent(
        unifs in prop::collection::vec((p2_stack(), p2_stack()), 0..24),
        probes in prop::collection::vec(p2_ty(), 1..8),
        probe_stacks in prop::collection::vec(p2_stack(), 1..6),
    ) {
        let mut ctx = InferCtx::new();
        for (a, b) in &unifs {
            let _ = ctx.unify_stack(a, b);
        }
        for t in &probes {
            let once = ctx.resolve_ty_deep(t);
            let twice = ctx.resolve_ty_deep(&once);
            prop_assert_eq!(once, twice, "deep resolution must be a fixpoint on types");
        }
        for s in &probe_stacks {
            let once = ctx.resolve_stack_deep(s);
            let twice = ctx.resolve_stack_deep(&once);
            prop_assert_eq!(once, twice, "deep resolution must be a fixpoint on stacks");
        }
    }
}

// ---------------------------------------------------------------------------
// P3 — Composition coherence (metamorphic, interface-only).
// ---------------------------------------------------------------------------

proptest! {
    /// Concatenativity's payoff: inference of a suffix `g` must depend only on the
    /// *interface stack* `A` it is handed, never on how `A` was produced. We split
    /// a generated program at an arbitrary point into prefix `f` (empty → A) and
    /// suffix `g`, then compare inference of `f g` against inference of `litA g`,
    /// where `litA` is literals reproducing `A`. Equal outputs witness that the
    /// stack-effect algebra composes coherently: `infer(f ++ g)` agrees with
    /// composing `infer(f)`'s result interface into `infer(g)`.
    #[test]
    fn p3_composition_coherence(script in choice_script(), split in any::<u16>()) {
        // Run the whole script, recording the stack at every prefix boundary.
        let mut st = Vec::new();
        let mut tokens = Vec::new();
        let mut boundary_stacks = vec![st.clone()];
        for &c in &script {
            let moves = moves_for(&st);
            let m = moves[(c as usize) % moves.len()];
            tokens.extend(run_move(m, &mut st));
            boundary_stacks.push(st.clone());
        }
        // Boundaries are between emitted tokens; choose one and split the *tokens*
        // at the same count of moves. Because a move can emit one token (all do
        // here), token index == move index.
        let k = (split as usize) % (tokens.len() + 1);
        let a_stack = &boundary_stacks[k];
        let f_tokens = &tokens[..k];
        let g_tokens = &tokens[k..];

        let prog_fg = format!("[ {} ] :main", concat_tokens(f_tokens, g_tokens));
        let lit_a = emit_literals(a_stack);
        let prog_litag = format!("[ {} ] :main", concat_tokens(&lit_a, g_tokens));

        let eff_fg = checked_main(&prog_fg).expect("f g type-checks");
        let eff_litag = checked_main(&prog_litag).expect("litA g type-checks");

        // The metamorphic core: same interface A ⇒ same composed base-type output,
        // whether A came from running `f` or from literals. (Origin spans
        // legitimately differ — the words sit at different byte offsets — so the
        // comparison is over base types, which is what "the effect" means here.)
        prop_assert!(
            elems_match(&eff_fg.output.elems, &st),
            "f g output disagrees with oracle"
        );
        prop_assert!(
            elems_match(&eff_litag.output.elems, &st),
            "composition depends on interface only: `{}` vs `{}`",
            prog_fg,
            prog_litag
        );
    }
}

fn concat_tokens(a: &[String], b: &[String]) -> String {
    let mut all: Vec<&str> = Vec::with_capacity(a.len() + b.len());
    all.extend(a.iter().map(String::as_str));
    all.extend(b.iter().map(String::as_str));
    all.join(" ")
}

// ---------------------------------------------------------------------------
// P4 — Generalization respects the environment.
// ---------------------------------------------------------------------------

/// Does any element type anywhere in a scheme's arrow mention a concrete base
/// type (`Con`)? A purely polymorphic shuffle never forces one; an operator does.
fn scheme_has_con(scheme: &Scheme) -> bool {
    word_has_con(&scheme.ty)
}
fn word_has_con(w: &WordTy) -> bool {
    stack_has_con(&w.input) || stack_has_con(&w.output)
}
fn stack_has_con(s: &StackTy) -> bool {
    s.elems.iter().any(ty_has_con)
}
fn ty_has_con(t: &Ty) -> bool {
    match &t.kind {
        TyKind::Con(_) => true,
        TyKind::Var(_) => false,
        TyKind::App(_, args) => args.iter().any(ty_has_con),
        TyKind::Quote(w) => word_has_con(w),
    }
}

/// A generic-shuffle body uses only `DUP/DROP/SWAP/OVER` — words that never pin a
/// base type. At definition level the stack tail is a polymorphic row, so any
/// sequence type-checks (operands come from the row).
fn shuffle_word() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("DUP"), Just("DROP"), Just("SWAP"), Just("OVER")]
}

proptest! {
    /// The let-polymorphism surface. A definition is generalized (M3 SCC pass) into
    /// a [`Scheme`]; generalizing a position the environment monomorphically
    /// constrains is the bug. We exercise both branches:
    ///
    /// * **Free ⇒ generalizes.** A pure shuffle forces no base type, so its scheme
    ///   must be `Con`-free — every value position is a quantified variable — and
    ///   it must quantify at least the row (the body is non-trivially polymorphic).
    /// * **Constrained ⇒ must NOT generalize.** Appending `+` pins the top two to
    ///   `Num`; that position must appear as `Con(Num)` in the scheme, i.e. it is
    ///   *not* generalized away.
    ///
    /// (caternary exposes generalization only at the definition boundary via
    /// [`definition_schemes`]; there is no standalone `generalize(τ, Γ)` seam, so
    /// the environment Γ is supplied as the operator constraint inside the body.)
    #[test]
    fn p4_generalization_respects_environment(
        body in prop::collection::vec(shuffle_word(), 0..8),
        constrain in any::<bool>(),
    ) {
        let mut words: Vec<&str> = body.clone();
        if constrain {
            // Force two Nums on top, then constrain them with `+`.
            words.push("0");
            words.push("0");
            words.push("+");
        }
        let src = format!("[ {} ] :w", words.join(" "));
        let mut eval = new_eval();
        let toks = parse_with_spans(&src).expect("parses");
        eval.load_with_spans(&toks).expect("loads");
        let schemes = definition_schemes(&eval).expect("generic body generalizes");
        let scheme = schemes.get("w").expect("w has a scheme");

        if constrain {
            prop_assert!(
                scheme_has_con(scheme),
                "a `+`-constrained position must NOT be generalized: {:?}",
                scheme
            );
        } else {
            prop_assert!(
                !scheme_has_con(scheme),
                "a pure shuffle must be fully polymorphic (Con-free): {:?}",
                scheme
            );
            prop_assert!(
                !scheme.rowvars.is_empty() || !scheme.tyvars.is_empty(),
                "a definition over a polymorphic stack must quantify something"
            );
            // Free vars round-trip to *fresh* ones: two instantiations don't alias.
            let mut ctx = InferCtx::new();
            let a = ctx.instantiate(scheme);
            let b = ctx.instantiate(scheme);
            if !scheme.rowvars.is_empty() {
                prop_assert_ne!(
                    a.input.row, b.input.row,
                    "distinct instantiations must not share a row var"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// P5 — Row neutrality.
// ---------------------------------------------------------------------------

proptest! {
    /// Prepend junk the term never touches; it must pass through via the row
    /// variable. A closed term (empty → S) operates only on what it pushes, so
    /// literals pushed *beneath* it must reappear unchanged below S. If this
    /// fails, the stack tail's row variable is not actually polymorphic — the term
    /// over-constrains the part of the stack it should not see.
    #[test]
    fn p5_row_neutrality(script in choice_script(), junk in prop::collection::vec(any::<bool>(), 0..5)) {
        let mut st = Vec::new();
        let body = simulate(&script, &mut st);

        let junk_bases: Vec<Base> = junk
            .iter()
            .map(|&b| if b { Base::Bool } else { Base::Num })
            .collect();
        let junk_tokens = emit_literals(&junk_bases);

        let base_src = format!("[ {} ] :main", body.join(" "));
        let with_src = format!(
            "[ {} ] :main",
            concat_tokens(&junk_tokens, &body)
        );

        let base = checked_main(&base_src).expect("body type-checks");
        let with = checked_main(&with_src).expect("junk-prefixed body type-checks");

        // base output == S; with output == junk ++ S (junk threaded via the row).
        let mut expected: Vec<Base> = junk_bases.clone();
        expected.extend(st.iter().copied());
        prop_assert!(
            elems_match(&base.output.elems, &st),
            "base output disagrees with oracle"
        );
        prop_assert!(
            elems_match(&with.output.elems, &expected),
            "junk did not thread untouched through the row: got {:?}, want {:?}",
            with.output.elems,
            expected
        );
    }
}

// ---------------------------------------------------------------------------
// P6 — Definition-order independence (whole-program model).
// ---------------------------------------------------------------------------

/// Zero every origin span in a value, so schemes can be compared structurally.
/// Origin spans are tied to source byte offsets, which legitimately move when
/// definitions are reordered; the *types* are what must be order-independent.
const ZERO_SPAN: Span = Span { start: 0, end: 0 };

fn norm_ty(t: &Ty) -> Ty {
    let kind = match &t.kind {
        TyKind::Var(v) => TyKind::Var(*v),
        TyKind::Con(n) => TyKind::Con(n.clone()),
        TyKind::App(n, args) => TyKind::App(n.clone(), args.iter().map(norm_ty).collect()),
        TyKind::Quote(w) => TyKind::Quote(Box::new(norm_word(w))),
    };
    Ty {
        kind,
        span: ZERO_SPAN,
    }
}
fn norm_stack(s: &StackTy) -> StackTy {
    StackTy {
        elems: s.elems.iter().map(norm_ty).collect(),
        row: s.row,
        span: ZERO_SPAN,
    }
}
fn norm_word(w: &WordTy) -> WordTy {
    let mut nw = WordTy::new(norm_stack(&w.input), norm_stack(&w.output));
    nw.refinement = w.refinement.clone();
    nw
}
fn norm_scheme(s: &Scheme) -> Scheme {
    Scheme::new(s.tyvars.clone(), s.rowvars.clone(), norm_word(&s.ty))
}
fn norm_schemes(
    m: &std::collections::HashMap<String, Scheme>,
) -> std::collections::BTreeMap<String, Scheme> {
    m.iter().map(|(k, v)| (k.clone(), norm_scheme(v))).collect()
}

proptest! {
    /// The flat whole-program pre-pass makes source order irrelevant for
    /// independent definitions. We generate several independent definitions
    /// (each a closed term referencing only builtins/literals — never another
    /// definition), load them in a generated order and in a permuted order, and
    /// assert [`definition_schemes`] returns the identical name → scheme map. A
    /// failure means the pre-pass leaks order-dependent state.
    #[test]
    fn p6_definition_order_independence(
        bodies in prop::collection::vec(choice_script(), 1..5),
        perm_seed in prop::collection::vec(any::<u32>(), 1..5),
    ) {
        // Build (name, source-fragment) pairs; name is fixed to the body's index
        // so the shuffle moves *fragments*, never relabels a body.
        let frags: Vec<(String, String)> = bodies
            .iter()
            .enumerate()
            .map(|(i, script)| {
                let mut st = Vec::new();
                let tokens = simulate(script, &mut st);
                let name = format!("w{i}");
                let frag = format!("[ {} ] :{}", tokens.join(" "), name);
                (name, frag)
            })
            .collect();

        // Original order, then a permutation by a stable sort on a seed.
        let mut shuffled: Vec<usize> = (0..frags.len()).collect();
        let seed: Vec<u32> = (0..frags.len())
            .map(|i| *perm_seed.get(i).unwrap_or(&(i as u32)))
            .collect();
        shuffled.sort_by_key(|&i| (seed[i], i));

        let src_orig = frags
            .iter()
            .map(|(_, f)| f.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let src_perm = shuffled
            .iter()
            .map(|&i| frags[i].1.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let schemes_orig = {
            let mut e = new_eval();
            let t = parse_with_spans(&src_orig).expect("orig parses");
            e.load_with_spans(&t).expect("orig loads");
            definition_schemes(&e).expect("orig checks")
        };
        let schemes_perm = {
            let mut e = new_eval();
            let t = parse_with_spans(&src_perm).expect("perm parses");
            e.load_with_spans(&t).expect("perm loads");
            definition_schemes(&e).expect("perm checks")
        };

        prop_assert_eq!(
            norm_schemes(&schemes_orig),
            norm_schemes(&schemes_perm),
            "definition order must not change inferred schemes (modulo origin spans)"
        );
    }
}
