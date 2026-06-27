//! M9 — first-order VC generation from refinement signatures, with
//! counterexample surfacing (§10.5 / §12 M9).
//!
//! This drives the **public Tier-1 check entry** [`check_refinements`]: it pulls
//! each call site's demands/guarantees from attached refinement signatures, zips
//! the demand binders against the inferred shadow stack, gathers the known facts
//! (preceding words' published guarantees + live path conditions), and discharges
//! each demand through the negated-goal encoding — surfacing a concrete
//! counterexample model on failure.

use caternary::*;

fn report(title: &str, obligations: &[Obligation]) {
    println!("=== {title} ===");
    for o in obligations {
        let status = match o.verdict {
            Verdict::Unsat => "DISCHARGED (valid)".to_string(),
            Verdict::Sat => match &o.model {
                Some(m) => format!("FAILED — counterexample {m}"),
                None => "FAILED".to_string(),
            },
            Verdict::Unknown => "UNDECIDED (degraded; not accepted)".to_string(),
        };
        println!("  {} : {:?}  =>  {status}", o.word, o.goal);
    }
}

fn main() {
    let sqrt = parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 )").unwrap();
    let nonneg = parse_signature("nonneg : ( -- r: Num where r >= 0 )").unwrap();

    // (1) `x sqrt` with no fact bounding x: the demand x >= 0 fails, and the VC
    // surfaces a concrete negative counterexample for x.
    {
        let toks = parse("x sqrt").unwrap();
        let lookup = |w: &str| (w == "sqrt").then(|| sqrt.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        report("x sqrt  (insufficient facts)", &obs);
    }

    // (2) `nonneg sqrt`: the preceding word's guarantee (r >= 0) is published as a
    // fact, so sqrt's demand discharges (Unsat, accepted).
    {
        let toks = parse("nonneg sqrt").unwrap();
        let lookup = |w: &str| match w {
            "sqrt" => Some(sqrt.clone()),
            "nonneg" => Some(nonneg.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        report("nonneg sqrt  (guarantee supplies the fact)", &obs);
    }

    // (3) `x 0 > [ x sqrt ] [ 0 ] if`: the M8 path condition composes with M9 VC
    // generation — inside the x > 0 branch, the demand discharges.
    {
        let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
        let lookup = |w: &str| (w == "sqrt").then(|| sqrt.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        report("x 0 > [ x sqrt ] [ 0 ] if  (path condition)", &obs);
    }
}
