//! M14 — whole-program ledger + attestation hash + CI gate / free runtime
//! (architecture section / §10.10 / §12 M14).
//!
//! This makes the final trust story inspectable end to end. It:
//!   1. registers an embedder operator and loads a whole program of definitions,
//!   2. prints the **one artifact attestation hash** of the contract set and
//!      shows it is stable across a rebuild of the same source,
//!   3. enumerates the **operator-table modulo** (language core + embedder),
//!   4. enumerates the **one global ledger** of `assume`s and shows it equals
//!      `grep assume` over the source, and
//!   5. demonstrates a **warm-compile cache hit** on an unchanged obligation
//!      (only the changed obligation re-solves).

use caternary::*;

/// A minimal stack value type so the program can both type-check and run.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Word(String),
    Bracket(Vec<Token>),
    Num(f64),
}

impl From<Token> for Value {
    fn from(token: Token) -> Self {
        match token {
            Token::Word(w) => match w.parse::<f64>() {
                Ok(n) => Value::Num(n),
                Err(_) => Value::Word(w),
            },
            Token::Bracket(b) => Value::Bracket(b),
        }
    }
}

impl Quotable for Value {
    fn as_quotation(&self) -> Option<&[Token]> {
        match self {
            Value::Bracket(b) => Some(b),
            _ => None,
        }
    }
    fn to_tokens(&self) -> Vec<Token> {
        match self {
            Value::Word(w) => vec![Token::Word(w.clone())],
            Value::Bracket(b) => vec![Token::Bracket(b.clone())],
            Value::Num(n) => vec![Token::Word(n.to_string())],
        }
    }
    fn as_sequence(&self) -> Option<Vec<Self>> {
        match self {
            Value::Bracket(b) => Some(b.iter().map(|t| Value::from(t.clone())).collect()),
            _ => None,
        }
    }
    fn from_sequence(elements: Vec<Self>) -> Self {
        Value::Bracket(elements.iter().flat_map(|v| v.to_tokens()).collect())
    }
}

/// A neutral Tier-0 scheme of the given pop/push arity over `Num`:
/// `( 'S Num…(pops) -- 'S Num…(pushes) )`.
fn num_scheme(pops: usize, pushes: usize) -> Scheme {
    let s = Span { start: 0, end: 1 };
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new((0..pops).map(|_| Ty::num(s)).collect(), 0, s),
            StackTy::new((0..pushes).map(|_| Ty::num(s)).collect(), 0, s),
        ),
    )
}

fn build(src: &str) -> Evaluator<Value> {
    let mut eval: Evaluator<Value> = Evaluator::new();
    // The embedder attests host operators: `+`, an opaque `mk` producer, and a
    // refined `sqrt` whose Tier-0 arrow ( Num -- Num ) carries a Tier-1 demand.
    eval.register_operator_with_contract("+", num_scheme(2, 1));
    eval.register_operator_with_contract("mk", num_scheme(0, 1));
    eval.register_operator_with_contract("sqrt", num_scheme(1, 1));
    eval.attach_refinement("sqrt : ( n: Num where n >= 0  --  r: Num )")
        .expect("sqrt refinement attaches");
    let toks = parse_with_spans(src).unwrap();
    eval.load_with_spans(&toks).unwrap();
    eval
}

fn ge(a: &str, k: &str) -> Pred {
    Pred::Bin(
        BinOp::Ge,
        Box::new(Pred::Var(a.into())),
        Box::new(Pred::Num(k.into())),
    )
}

fn main() {
    // A small whole program with a refined, `assume`-bearing definition:
    //   `foo`  produces an opaque value, asserts `result >= 0` over it (an
    //          `assume` the solver cannot otherwise discharge), and feeds it to
    //          the refined `sqrt` (which demands `n >= 0`);
    //   `main` runs `foo` and closes against the empty stack.
    let src = "[ mk \"assume(result >= 0)\" sqrt DROP ] :foo [ foo ] :main";
    let eval = build(src);

    // (1) The CI gate — a SINGLE build-time act (§10.10, invariant 19/20): one
    // call runs Tier 0 (shape safety) first and, only on green, Tier 1 +
    // operator-axiom discharge, returning the unified outcome (the whole-program
    // ledger). This replaces the pre-gate two-call `check` + `check_program`
    // pattern, so the Tier-0-gates-Tier-1 ordering is structural, not convention.
    let ledger = check_whole_program(&eval, SmtLibSolver::new)
        .expect("the program is checked: Tier 0 then Tier 1 in one act");
    println!("== caternary check: PASS (build-time gate — Tier 0 + Tier 1 in one call) ==\n");

    // (2) The one artifact attestation hash, stable across rebuilds.
    let h1 = attestation_hash(&eval).unwrap();
    let h2 = attestation_hash(&build(src)).unwrap();
    println!("whole-program attestation hash: {h1:#018x}");
    println!("rebuild of unchanged source:   {h2:#018x}");
    println!("stable across rebuilds:        {}\n", h1 == h2);

    // (3) The operator-table modulo of every proof.
    let table = OperatorTable::of(&eval);
    println!("operator table (the modulo of every proof):");
    for entry in table.entries() {
        println!("  [{:>8}] {}", entry.origin.tag(), entry.name);
    }
    println!();

    // (4) The one global ledger of assumes = grep assume over the source. This
    // is the SAME `ledger` the gate returned in (1) — the single act already
    // paid the Tier-1 cost; we just read it back here.
    let foo_body = eval.definition_body("foo").unwrap();
    println!("one global ledger — complete user trusted base:");
    for surface in ledger.grep_assume() {
        println!("  {surface}");
    }
    println!(
        "grep assume over source:       {:?}",
        grep_assume_tokens(foo_body)
    );
    println!("foo's honest status:           {}\n", ledger.status("foo"));

    // (5) Warm-compile reuse: only the changed obligation re-solves.
    let mut cache = ExploratoryCache::new();
    let obligations = [ge("x", "0"), ge("y", "0")];
    {
        let mut solver = SmtLibSolver::new();
        for g in &obligations {
            cache.discharge_obligation(&mut solver, g);
        }
    }
    println!(
        "cold compile: solves={} hits={}",
        cache.solves(),
        cache.hits()
    );
    {
        // Re-run with one unchanged + one changed obligation.
        let mut solver = SmtLibSolver::new();
        let _ = cache.discharge_obligation(&mut solver, &obligations[0]); // unchanged ⇒ hit
        let changed = ge("z", "0");
        let _ = cache.discharge_obligation(&mut solver, &changed); // new ⇒ solve
    }
    println!(
        "warm compile: solves={} hits={}",
        cache.solves(),
        cache.hits()
    );
    println!("(only the changed obligation re-solved; the unchanged one was reused)");

    // Per-obligation sub-hashes are stable fingerprints of the exact cache keys.
    let sub = obligation_sub_hash(&obligations[0], &[]);
    println!("\nper-obligation sub-hash of `x >= 0`: {sub:#018x}");
}
