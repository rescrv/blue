//! M13 — SMT-LIB2 emission parity (§10.9 / §12 M13).
//!
//! §10.9 calls the text-emission mode "your debugging window." This example makes
//! it inspectable end to end: it prints the SMT-LIB2 scripts the solver-agnostic
//! [`SmtLibSolver`] seam emits across the Tier 1 surface — path conditions + an
//! `assume` boundary, and a higher-order subsumption boundary — alongside the
//! verdicts the seam's `check()` returned for each decision.
//!
//! These emitted scripts are the **parity reference** a future native `Z3Solver`
//! (implementing the same four-method [`Solver`] trait) would assert its native
//! verdict against. Native parity is the deferred "if wired" path
//! ([`M13_NATIVE_PARITY_NOTE`]) because the `z3` crate cannot resolve/build
//! offline; the text mode is what M13 delivers and verifies today.

use caternary::*;

fn binder(name: &str) -> Binder {
    Binder {
        name: name.into(),
        ty: "Num".into(),
        span: RefineSpan { start: 0, end: 0 },
        quote: None,
    }
}

fn gt(a: &str, k: &str) -> Pred {
    Pred::Bin(
        BinOp::Gt,
        Box::new(Pred::Var(a.into())),
        Box::new(Pred::Num(k.into())),
    )
}

fn print_script(title: &str, script: &str) {
    println!("=== {title} ===");
    println!("{script}");
}

fn main() {
    println!("{M13_NATIVE_PARITY_NOTE}\n");

    // ---- Scenario 1: path condition + an `assume` boundary -----------------
    //
    // `x 0 > [ x sqrt ] [ 0 ] if` exercises the M8 path-condition `if` (the
    // sqrt demand discharges under x > 0), and `opaque assume(x>=0) sqrt`
    // exercises the M12 strict drop-and-re-run plus the dependent obligation.
    let span = || Span { start: 0, end: 0 };
    let sqrt = |w: &str| -> VerifyWord {
        if w == "sqrt" {
            return VerifyWord::Call {
                binders: vec![binder("n")],
                demand: Some(Pred::Bin(
                    BinOp::Ge,
                    Box::new(Pred::Var("n".into())),
                    Box::new(Pred::Num("0".into())),
                )),
                out_binders: vec![binder("r")],
                guarantee: None,
                arrow: WordTy::new(
                    StackTy::new(vec![Ty::num(span())], 0, span()),
                    StackTy::new(vec![Ty::num(span())], 0, span()),
                ),
            };
        }
        if let Some(c) = core_shadow_word(w) {
            VerifyWord::Core(c)
        } else if let Some(op) = interpreted_op(w) {
            VerifyWord::Core(op)
        } else if is_numeric_literal(w) {
            VerifyWord::Core(ShadowWord::Num(w.to_string()))
        } else {
            VerifyWord::Core(ShadowWord::Var(w.to_string()))
        }
    };

    let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
    let mut solver = SmtLibSolver::new();
    let mut stack = ShadowStack::new();
    let mut obligations = Vec::new();
    verify(&toks, &mut stack, &mut solver, &sqrt, &mut obligations).unwrap();
    print_script("path-condition `if` (M8)", solver.script());
    for o in &obligations {
        println!("  obligation {:?} => {:?}", o.goal, o.verdict);
    }
    println!();

    let toks = parse("opaque assume(x>=0) sqrt").unwrap();
    let mut solver = SmtLibSolver::new();
    let mut stack = ShadowStack::new();
    let mut ctx = VerifyCtx::with_site("demo");
    verify_ctx(&toks, &mut stack, &mut solver, &sqrt, &mut ctx).unwrap();
    print_script(
        "`assume` drop-and-re-run + dependent obligation (M12)",
        solver.script(),
    );
    for a in ctx.assumes() {
        println!(
            "  assume {:?}: {:?} (exploratory {:?})",
            a.surface_pred, a.legality, a.exploratory
        );
    }
    for o in ctx.obligations() {
        println!("  obligation {:?} => {:?}", o.goal, o.verdict);
    }
    println!();

    // ---- Scenario 2: a higher-order subsumption boundary -------------------
    //
    // provided { demand n>0, guarantee r>5 } meets expected { demand n>5,
    // guarantee r>0 }: the checker emits exactly TWO directional implications —
    // covariant guarantee (provided_post ⟹ expected_post) and contravariant
    // demand (expected_pre ⟹ provided_pre, flipped) — both valid (Preserved).
    let provided = RefinementSig {
        name: "q".into(),
        demands: RefinementSide {
            binders: vec![binder("n")],
            predicate: Some(gt("n", "0")),
        },
        guarantees: RefinementSide {
            binders: vec![binder("r")],
            predicate: Some(gt("r", "5")),
        },
    };
    let expected = RefinementSig {
        name: "q".into(),
        demands: RefinementSide {
            binders: vec![binder("n")],
            predicate: Some(gt("n", "5")),
        },
        guarantees: RefinementSide {
            binders: vec![binder("r")],
            predicate: Some(gt("r", "0")),
        },
    };
    let mut solver = SmtLibSolver::new();
    let res = check_subsumption(&provided, &expected, &mut solver);
    print_script(
        "subsumption: two directional implications (M10)",
        solver.script(),
    );
    println!(
        "  guarantee (covariant) {:?} => {:?}",
        res.guarantee.implication, res.guarantee.verdict
    );
    println!(
        "  demand (contravariant) {:?} => {:?}",
        res.demand.implication, res.demand.verdict
    );
    println!("  outcome: {:?}", res.outcome());
}
