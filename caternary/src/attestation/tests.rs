//! M14 acceptance tests (§12 M14): one global ledger / grep-assume parity, the
//! operator table as the enumerable modulo, the whole-program attestation hash
//! (stable across rebuilds; selective per-obligation content-key invalidation with
//! warm-compile reuse), and the CI-gate / free-runtime structural guarantees.

use super::*;

use crate::BinOp;
use crate::Definition;
use crate::Evaluator;
use crate::ExploratoryCache;
use crate::Pred;
use crate::SmtLibSolver;
use crate::Solver;
use crate::Span;
use crate::Token;
use crate::Verdict;
use crate::check_program;
use crate::obligation_sub_hash;
use crate::parse;
use crate::parse_with_spans;
use crate::types::Scheme;
use crate::types::StackTy;
use crate::types::Ty;
use crate::types::WordTy;

// ----- test scaffolding -----------------------------------------------------

/// A minimal stack value type for driver tests (mirrors the check.rs harness).
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

/// `+ : ( 'S Num Num -- 'S Num )` — an embedder-registered contract.
fn plus_scheme() -> Scheme {
    let s = sp();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::num(s)], 0, s),
        ),
    )
}

/// Build an evaluator from program source, registering `+` as the only embedder
/// operator. The same source always produces an equivalent evaluator (a rebuild).
fn build(src: &str) -> Evaluator<Value> {
    let mut eval: Evaluator<Value> = Evaluator::new();
    eval.register_operator_with_contract("+", plus_scheme());
    let toks = parse_with_spans(src).unwrap();
    eval.load_with_spans(&toks).unwrap();
    eval
}

fn var(n: &str) -> Pred {
    Pred::Var(n.to_string())
}
fn num(n: &str) -> Pred {
    Pred::Num(n.to_string())
}
fn ge(a: &str, k: &str) -> Pred {
    Pred::Bin(BinOp::Ge, Box::new(var(a)), Box::new(num(k)))
}

// ===========================================================================
// (1) One global ledger — grep assume = the complete user trusted base
// ===========================================================================

#[test]
fn grep_assume_over_the_program_equals_the_one_global_ledger() {
    // (§12 M14 / invariant 15) `grep assume` over the whole program enumerates the
    // complete user trusted base, and it equals the one global ledger's accepted
    // entries — there is no per-module reconciliation, one `grep` covers it all.
    let foo_body = parse("opaque \"assume(result >= 0)\" sqrt").unwrap();
    let bar_body = parse("foo").unwrap();
    let baz_body = parse("opaque \"assume(other >= 0)\" sqrt").unwrap();
    let defs = vec![
        Definition {
            name: "foo".into(),
            body: foo_body.clone(),
            sig: None,
        },
        Definition {
            name: "bar".into(),
            body: bar_body.clone(),
            sig: None,
        },
        Definition {
            name: "baz".into(),
            body: baz_body.clone(),
            sig: None,
        },
    ];
    let lookup = |w: &str| match w {
        "sqrt" => crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").ok(),
        _ => None,
    };
    let ledger = check_program(&defs, &lookup, SmtLibSolver::new).unwrap();
    assert!(ledger.is_clean(), "rejections: {:?}", ledger.rejections());

    // Source-side grep over EVERY definition body (the whole program).
    let mut source_grep: Vec<String> = Vec::new();
    for d in &defs {
        source_grep.extend(grep_assume_tokens(&d.body));
    }
    source_grep.sort();

    // Ledger-side enumeration of the complete user trusted base.
    let mut ledger_grep = ledger.grep_assume();
    ledger_grep.sort();

    assert_eq!(
        source_grep, ledger_grep,
        "grep assume over the program must equal the one global ledger"
    );
    assert_eq!(
        ledger_grep,
        vec![
            "assume(other >= 0)".to_string(),
            "assume(result >= 0)".to_string()
        ]
    );
    assert_eq!(ledger.assumptions().len(), 2);
}

#[test]
fn grep_assume_recurses_into_quotations() {
    // The source-side grep finds assumes nested inside quotations, so a program's
    // complete trusted base is enumerable regardless of bracket nesting.
    let toks = parse("[ a \"assume(p > 0)\" [ b \"assume(q > 0)\" ] ]").unwrap();
    let mut found = grep_assume_tokens(&toks);
    found.sort();
    assert_eq!(
        found,
        vec!["assume(p > 0)".to_string(), "assume(q > 0)".to_string()]
    );
}

// ===========================================================================
// (2) The operator table is the enumerable modulo of every proof
// ===========================================================================

#[test]
fn operator_table_enumerates_the_modulo() {
    // (§12 M14 / invariants 16/17) The operator table = language-core primitives
    // PLUS embedder registrations, and nothing else.
    let eval = build("[ 1 + ] :main");
    let table = OperatorTable::of(&eval);

    let core_names: Vec<String> = table.core().map(|c| c.name.clone()).collect();
    let expected_core = [
        "DUP", "DROP", "SWAP", "OVER", "ROT", "-ROT", "NIP", "TUCK", "2DUP", "2DROP", "2SWAP",
        "2OVER", "2ROT", "CALL", "DIP", "2DIP", "3DIP", "IF", "KEEP", "2KEEP", "3KEEP", "BI",
        "BI*", "BI@", "TRI", "TRI*", "TRI@", "COMPOSE",
    ];
    for p in expected_core.iter().copied() {
        assert!(
            core_names.contains(&p.to_string()),
            "missing core primitive {p}"
        );
        let entry = table.entries().iter().find(|e| e.name == p).unwrap();
        assert_eq!(entry.origin, OperatorOrigin::LanguageCore);
    }

    let embedder_names: Vec<String> = table.embedder().map(|c| c.name.clone()).collect();
    assert_eq!(embedder_names, vec!["+".to_string()]);
    let plus = table.entries().iter().find(|e| e.name == "+").unwrap();
    assert_eq!(plus.origin, OperatorOrigin::Embedder);
    assert_eq!(plus.scheme, plus_scheme());

    assert_eq!(table.core().count(), expected_core.len());
    assert_eq!(table.embedder().count(), 1);
    assert_eq!(table.len(), expected_core.len() + 1);
}

#[test]
fn different_embeddings_have_different_trusted_bases() {
    // (architecture / invariant 17) The trusted base is the API the host chose to
    // expose, attested; a sandbox and a systems embedding differ in their modulo.
    let sandbox: Evaluator<Value> = Evaluator::new();
    let sandbox_table = OperatorTable::of(&sandbox);
    assert_eq!(
        sandbox_table.embedder().count(),
        0,
        "sandbox exposes no host ops"
    );

    let mut systems: Evaluator<Value> = Evaluator::new();
    systems.register_operator_with_contract("syscall_read", plus_scheme());
    let systems_table = OperatorTable::of(&systems);
    assert_eq!(systems_table.embedder().count(), 1);
    assert_eq!(
        systems_table.embedder().next().unwrap().name,
        "syscall_read"
    );

    let sc: Vec<_> = sandbox_table.core().map(|c| c.name.clone()).collect();
    let yc: Vec<_> = systems_table.core().map(|c| c.name.clone()).collect();
    assert_eq!(sc, yc, "the core half is identical");
    assert_ne!(sandbox_table.names(), systems_table.names());
}

// ===========================================================================
// (3) Whole-program attestation hash — stable + content-addressed
// ===========================================================================

#[test]
fn attestation_hash_is_stable_across_rebuilds_of_unchanged_source() {
    // (§12 M14) The whole-program attestation hash is stable across rebuilds of
    // unchanged source (fresh inference, fresh internal variable ids).
    let src = "[ bar 1 + ] :foo [ 2 ] :bar [ foo ] :main";
    let h1 = attestation_hash(&build(src)).unwrap();
    let h2 = attestation_hash(&build(src)).unwrap();
    assert_eq!(h1, h2, "attestation hash must be stable across rebuilds");

    let cs = ContractSet::of(&build(src)).unwrap();
    assert_eq!(cs.attestation_hash(), h1);
}

#[test]
fn attestation_hash_changes_when_the_contract_set_changes() {
    // The attestation hash is content-addressed over the contract set, not a
    // constant: a changed signature or a changed operator table changes it.
    let base = attestation_hash(&build("[ 1 + ] :foo [ foo ] :main")).unwrap();

    let changed = attestation_hash(&build("[ 1 + + ] :foo [ 3 foo ] :main")).unwrap();
    assert_ne!(base, changed, "a changed contract set must change the hash");

    let mut eval = build("[ 1 + ] :foo [ foo ] :main");
    eval.register_operator_with_contract("neg", plus_scheme());
    let with_op = attestation_hash(&eval).unwrap();
    assert_ne!(
        base, with_op,
        "a changed operator table must change the hash"
    );
}

#[test]
fn attestation_hash_ignores_internal_variable_ids() {
    // The canonical rendering renumbers variables in first-appearance order, so a
    // scheme's hash does not depend on the ids inference happened to assign — this
    // is *why* the hash is reproducible across rebuilds.
    let a = ContractSet::of(&build("[ DUP ] :twice [ twice ] :main")).unwrap();
    let b = ContractSet::of(&build("[ 7 DROP ] :noise [ DUP ] :twice [ twice ] :main")).unwrap();
    let ta = super::canonical_scheme(&a.definitions["twice"]);
    let tb = super::canonical_scheme(&b.definitions["twice"]);
    assert_eq!(
        ta, tb,
        "canonical signature must not depend on var-id allocation"
    );
}

// ===========================================================================
// (3b) Per-obligation keys — selective invalidation + warm-compile reuse
// ===========================================================================

#[test]
fn changing_one_obligation_invalidates_only_its_sub_hash() {
    // (§12 M14) Changing one obligation changes only its sub-hash; every other
    // obligation's sub-hash is unchanged (selective invalidation).
    let facts: Vec<Pred> = vec![ge("a", "0")];
    let g1 = ge("x", "0");
    let g2 = ge("y", "0");
    let g3 = ge("z", "0");

    let h1 = obligation_sub_hash(&g1, &facts);
    let h2 = obligation_sub_hash(&g2, &facts);
    let h3 = obligation_sub_hash(&g3, &facts);

    assert_eq!(h1, obligation_sub_hash(&g1, &facts), "sub-hash is stable");

    let g3b = Pred::Bin(BinOp::Ge, Box::new(var("z")), Box::new(num("1")));
    let h3b = obligation_sub_hash(&g3b, &facts);
    assert_ne!(h3, h3b, "the changed obligation's sub-hash must change");
    assert_eq!(h1, obligation_sub_hash(&g1, &facts), "g1 unchanged");
    assert_eq!(h2, obligation_sub_hash(&g2, &facts), "g2 unchanged");

    let facts2: Vec<Pred> = vec![ge("a", "1")];
    assert_ne!(
        h1,
        obligation_sub_hash(&g1, &facts2),
        "facts are part of the key"
    );
}

#[test]
fn warm_compile_reuses_unchanged_obligations_and_resolves_only_the_changed_one() {
    // (§12 M14) The discharge cache keyed on exact per-obligation content gives
    // warm-compile reuse: a recompile re-solves only the changed obligation.
    let g1 = ge("x", "0");
    let g2 = ge("y", "0");
    let g3 = ge("z", "0");

    let mut cache = ExploratoryCache::new();

    {
        let mut solver = SmtLibSolver::new();
        for g in [&g1, &g2, &g3] {
            let (_v, from_cache) = cache.discharge_obligation(&mut solver, g);
            assert!(!from_cache, "cold compile is all misses");
        }
    }
    assert_eq!(cache.solves(), 3);
    assert_eq!(cache.hits(), 0);

    let g3b = Pred::Bin(BinOp::Ge, Box::new(var("z")), Box::new(num("1")));
    {
        let mut solver = SmtLibSolver::new();
        let (_v1, c1) = cache.discharge_obligation(&mut solver, &g1);
        let (_v2, c2) = cache.discharge_obligation(&mut solver, &g2);
        let (_v3, c3) = cache.discharge_obligation(&mut solver, &g3b);
        assert!(c1, "g1 unchanged ⇒ warm cache hit");
        assert!(c2, "g2 unchanged ⇒ warm cache hit");
        assert!(!c3, "g3 changed ⇒ a fresh solve");
    }
    assert_eq!(cache.solves(), 4, "only the changed obligation re-solves");
    assert_eq!(cache.hits(), 2, "the two unchanged obligations are reused");
}

// ===========================================================================
// (4) CI gate; free runtime — no Z3, no shadow stack at runtime
// ===========================================================================

#[test]
fn checked_program_links_no_z3_symbols() {
    // (§12 M14 / invariant 20) A checked program links no solver. The native z3
    // backend (`Z3Solver`) is a build-time-only CHECK backend behind the optional
    // `z3` feature; it must be OFF by default so the default build — and a checked
    // program at runtime — links no solver. Assert that opt-in shape structurally.
    let manifest = include_str!("../../Cargo.toml");

    // z3 must be declared `optional` (so it is never in the default graph) ...
    let deps = manifest
        .split_once("[dependencies]")
        .expect("a [dependencies] section")
        .1;
    let z3_line = deps
        .lines()
        .find(|l| l.trim_start().starts_with("z3"))
        .expect("a z3 dependency line");
    assert!(
        z3_line.contains("optional = true"),
        "z3 must be an optional dependency, not a hard one: {z3_line}"
    );

    // ... gated behind a `z3` feature that the default feature set does NOT pull
    // in (so `caternary check` links no solver unless explicitly built --features z3).
    let features = manifest
        .split_once("[features]")
        .expect("a [features] section gating the solver")
        .1;
    assert!(
        features.contains("z3 = [\"dep:z3\"]"),
        "the z3 backend must be gated behind a `z3` feature"
    );
    let default_line = features
        .lines()
        .find(|l| l.trim_start().starts_with("default"))
        .expect("a `default` feature line");
    assert!(
        !default_line.contains("z3"),
        "the z3 feature must NOT be enabled by default: {default_line}"
    );
}

#[test]
fn evaluator_carries_no_shadow_stack_or_solver_at_runtime() {
    // (§12 M14 / invariants 14/20) The shadow stack and solver are compile-time
    // only: the shadow stack is NEVER a field of the runtime Evaluator. Lock it
    // structurally by reading the Evaluator struct definition.
    let src = include_str!("../evaluator.rs");
    let start = src
        .find("pub struct Evaluator<T> {")
        .expect("Evaluator struct");
    let body = &src[start..];
    let end = body.find("\n}").expect("struct body ends") + start;
    let struct_def = &src[start..end];
    for forbidden in [
        "ShadowStack",
        "SmtLibSolver",
        "ExploratoryCache",
        "Obligation",
    ] {
        assert!(
            !struct_def.contains(forbidden),
            "Evaluator must not carry `{forbidden}` at runtime:\n{struct_def}"
        );
    }
    assert!(
        !struct_def.contains("dyn Solver") && !struct_def.contains(": Solver"),
        "Evaluator must not carry a solver:\n{struct_def}"
    );
}

#[test]
fn check_is_the_build_time_gate_and_eval_runs_lean() {
    // (§10.10 / invariant 20) `caternary check` (the public type_check/check path)
    // is the build-time gate; a checked program runs with no verification
    // machinery and evaluation touches no solver.
    let mut eval = build("[ 1 1 + ] :main");
    crate::check(&eval).expect("the program is checked");
    eval.define("+", |stack: &mut Vec<Value>, _ev: &Evaluator<Value>| {
        stack.pop();
        Ok(())
    });
    let out = eval.eval(&parse("1 1 +").unwrap()).expect("runs lean");
    assert_eq!(out.len(), 1);
}

#[test]
fn attestation_hash_requires_a_checked_program() {
    // The attestation hash runs the same SCC generalization the gate runs, so an
    // ill-typed definition yields a TypeError rather than a hash (CI-gate
    // consistency): `+` demands `Num Num`, but `true` is `Bool`.
    let eval = build("[ true 1 + ] :main");
    assert!(attestation_hash(&eval).is_err());
}

#[allow(dead_code)]
fn _verdict_referenced(v: Verdict) -> bool {
    matches!(v, Verdict::Unsat | Verdict::Sat | Verdict::Unknown)
}

#[allow(dead_code)]
fn _solver_referenced<S: Solver>(_s: &S) {}
