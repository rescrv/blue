//! Shadow/runtime conformance with an independent concrete generator oracle.
//!
//! Each move advances a stack of kinds AND values. The harness checks production
//! gate acceptance, runtime output against Tier-0 types (including list elements),
//! complete output values/quotation bodies against that simulation, and shadow
//! claims against runtime observations. Registered refinements and definition
//! precedence are enabled in the compared shadow run. Ground integer/boolean
//! expressions and resolvable refinement facts are evaluated independently of the
//! solver; opaque outputs are still checked against the concrete generator.
//!
//! The compositional vocabulary includes shuffles, numeric operations, literal
//! and function quotations, CALL/DIP, sequence combinators, and IF. Closed boundary
//! fragments add locals/captures, helper calls, nested lists and quotations,
//! quotation construction, and further typed combinators. Copied quotations can
//! be used at different stack depths and types. Separate boundary cases observe
//! numeric domain errors and the known Tier-0 BI@ limitation explicitly.
//!
//! Seed vectors shrink to shorter valid programs. The main generator uses
//! representable integer arithmetic; numeric boundary tests separately cover
//! exactness above 2^53, i128 limits, fractions, and runtime/gate rejection classes.

use std::collections::{BTreeMap, BTreeSet};

use caternary::*;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// A minimal stack value type so `Evaluator<T>` can be instantiated. Mirrors the
// driver-test `Value` in `src/evaluator.rs`: scalars are their source lexemes,
// quotations are brackets, and sequences are brackets of pushes (the runtime
// realization of the dual-purpose literal).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Word(String),
    Bracket(Vec<QuoteItem<Value>>),
}

impl From<Token> for Value {
    fn from(token: Token) -> Self {
        match token {
            Token::Word(w) => Value::Word(w),
            Token::Bracket(b) => Value::Bracket(quote_items_from_tokens(&b)),
        }
    }
}

impl Quotable for Value {
    fn as_quotation(&self) -> Option<&[QuoteItem<Self>]> {
        match self {
            Value::Bracket(b) => Some(b),
            Value::Word(_) => None,
        }
    }

    fn from_quotation(items: Vec<QuoteItem<Self>>) -> Self {
        Value::Bracket(items)
    }

    fn to_tokens(&self) -> Vec<Token> {
        match self {
            Value::Word(w) => vec![Token::Word(w.clone())],
            Value::Bracket(b) => vec![Token::Bracket(quote_items_to_tokens(b))],
        }
    }

    fn is_truthy(&self) -> bool {
        // Comparisons render booleans as their lexemes (`numeric_cmp` pushes
        // `true`/`false` words); IF and FILTER branch on this.
        !matches!(self, Value::Word(w) if w == "false")
    }

    fn as_sequence(&self) -> Option<Vec<Self>> {
        match self {
            Value::Bracket(b) => Some(quote_items_to_values(b)),
            Value::Word(_) => None,
        }
    }

    fn from_sequence(elements: Vec<Self>) -> Self {
        Value::Bracket(elements.iter().cloned().map(QuoteItem::Push).collect())
    }
}

// ---------------------------------------------------------------------------
// The typed generator: simulated kinds, moves, and seed-directed selection.
// ---------------------------------------------------------------------------

/// Concrete simulated slots. Quotation uses instantiate their privately owned
/// variables, so copied quotations may be applied at different stack depths.
#[derive(Debug, Clone, PartialEq)]
enum K {
    Num(i128),
    Lit(Vec<i128>),
    FnMap(u8),
    FnFold,
    FnEach,
    FnFilter(u8),
    List(Vec<i128>),
    Bool(bool),
    Data(Value),
}

impl K {
    fn list_like(&self) -> bool {
        matches!(self, K::Lit(_) | K::List(_))
    }
    fn numeric(&self) -> bool {
        matches!(self, K::Num(_))
    }
    fn boolean(&self) -> bool {
        matches!(self, K::Bool(_))
    }
    fn num(&self) -> i128 {
        let K::Num(n) = self else {
            panic!("numeric move precondition")
        };
        *n
    }
    fn numbers(&self) -> Vec<i128> {
        match self {
            K::Lit(xs) | K::List(xs) => xs.clone(),
            _ => panic!("sequence move precondition"),
        }
    }
    /// Expected observable value, computed by the generator rather than either engine.
    fn value(&self) -> Value {
        let src = match self {
            K::Data(value) => return value.clone(),
            K::Num(n) => n.to_string(),
            K::Bool(b) => b.to_string(),
            K::Lit(xs) | K::List(xs) => format!(
                "[ {} ]",
                xs.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            K::FnMap(k) => format!("[ {} + ]", k % 10),
            K::FnFold => "[ + ]".into(),
            K::FnEach => "[ DROP ]".into(),
            K::FnFilter(k) => format!("[ {} > ]", k % 10),
        };
        Value::from(parse(&src).unwrap().remove(0))
    }
}

/// One generated move: the source tokens it emits and its simulated effect.
/// `emit` renders source; `apply` transforms the simulated stack. The two are
/// kept adjacent in one `match` so a vocabulary extension cannot update one
/// without the other staring at it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Move {
    Boundary(u8),
    PushNum(u8),
    PushLit(u8),
    PushFnMap(u8),
    PushFnFold,
    PushFnEach,
    PushFnFilter(u8),
    PushBool(u8),
    Dup,
    Drop,
    Swap,
    Over,
    Rot,
    Nip,
    Tuck,
    Add,
    Sub,
    Mul,
    Lt,
    Gt,
    Call,
    Dip,
    Map,
    Fold,
    Each,
    Filter,
    /// `[ a ] [ b ] IF` — arms of effect `( 'S -- 'S Num )`; the two 4-bit arm
    /// literals ride in the payload, so agreed and disagreed arms both occur.
    IfConst(u8),
    /// `[ a + ] [ b * ] IF` — arms of effect `( 'S Num -- 'S Num )`.
    IfXform(u8),
}

const ALL_MOVES: &[Move] = &[
    Move::Boundary(0),
    Move::PushNum(0),
    Move::PushLit(0),
    Move::PushFnMap(0),
    Move::PushFnFold,
    Move::PushFnEach,
    Move::PushFnFilter(0),
    Move::PushBool(0),
    Move::Dup,
    Move::Drop,
    Move::Swap,
    Move::Over,
    Move::Rot,
    Move::Nip,
    Move::Tuck,
    Move::Add,
    Move::Sub,
    Move::Mul,
    Move::Lt,
    Move::Gt,
    Move::Call,
    Move::Dip,
    Move::Map,
    Move::Fold,
    Move::Each,
    Move::Filter,
    Move::IfConst(0),
    Move::IfXform(0),
];

impl Move {
    /// Is this move legal on the simulated stack? Mirrors each word's Tier 0
    /// scheme (which is the runtime's contract, which is the shadow's arrow —
    /// the very coincidence the property asserts).
    fn legal(self, sim: &[K]) -> bool {
        let n = sim.len();
        let top = |i: usize| &sim[n - 1 - i];
        match self {
            Move::Boundary(_)
            | Move::PushNum(_)
            | Move::PushLit(_)
            | Move::PushFnMap(_)
            | Move::PushFnFold
            | Move::PushFnEach
            | Move::PushFnFilter(_)
            | Move::PushBool(_) => true,
            Move::Dup | Move::Drop => n >= 1,
            Move::Swap | Move::Nip | Move::Tuck | Move::Over => n >= 2,
            Move::Rot => n >= 3,
            Move::Add | Move::Sub | Move::Mul | Move::Lt | Move::Gt => {
                n >= 2
                    && top(0).numeric()
                    && top(1).numeric()
                    && match self {
                        Move::Add => top(1).num().checked_add(top(0).num()).is_some(),
                        Move::Sub => top(1).num().checked_sub(top(0).num()).is_some(),
                        Move::Mul => top(1).num().checked_mul(top(0).num()).is_some(),
                        _ => true,
                    }
            }
            Move::Call => {
                n >= 1
                    && match top(0) {
                        K::Lit(_) => true,
                        K::FnMap(k) => {
                            n >= 2
                                && top(1).numeric()
                                && top(1).num().checked_add(i128::from(k % 10)).is_some()
                        }
                        K::FnFilter(_) => n >= 2 && top(1).numeric(),
                        K::FnFold => {
                            n >= 3
                                && top(1).numeric()
                                && top(2).numeric()
                                && top(2).num().checked_add(top(1).num()).is_some()
                        }
                        K::FnEach => n >= 2,
                        _ => false,
                    }
            }
            Move::Dip => {
                n >= 3
                    && matches!(top(0), K::FnMap(_))
                    && top(2).numeric()
                    && top(2).num().checked_add(9).is_some()
            }
            Move::Map => n >= 2 && matches!(top(0), K::FnMap(_)) && top(1).list_like(),
            Move::Fold => {
                n >= 3
                    && matches!(top(0), K::FnFold)
                    && top(1).numeric()
                    && top(2).list_like()
                    && top(1)
                        .num()
                        .checked_add(top(2).numbers().iter().sum())
                        .is_some()
            }
            Move::Each => n >= 2 && matches!(top(0), K::FnEach) && top(1).list_like(),
            Move::Filter => n >= 2 && matches!(top(0), K::FnFilter(_)) && top(1).list_like(),
            // Both IF families emit their arms inline (freshly typed, so no
            // occurs concern); the condition must sit on top.
            Move::IfConst(_) => n >= 1 && top(0).boolean(),
            Move::IfXform(_) => {
                n >= 2
                    && top(0).boolean()
                    && top(1).numeric()
                    && top(1).num().checked_add(15).is_some()
                    && top(1).num().checked_mul(15).is_some()
            }
        }
    }

    /// Advance both the kind and the independent concrete-value oracle.
    fn apply(self, sim: &mut Vec<K>) {
        let n = sim.len();
        match self {
            Move::Boundary(k) => sim.push(boundary_case(k).1),
            Move::PushNum(k) => sim.push(K::Num(i128::from(k % 10))),
            Move::PushLit(sz) => sim.push(K::Lit((1..=i128::from(sz % 4)).collect())),
            Move::PushFnMap(k) => sim.push(K::FnMap(k)),
            Move::PushFnFold => sim.push(K::FnFold),
            Move::PushFnEach => sim.push(K::FnEach),
            Move::PushFnFilter(k) => sim.push(K::FnFilter(k)),
            Move::PushBool(k) => sim.push(K::Bool(k % 2 == 0)),
            Move::Dup => sim.push(sim[n - 1].clone()),
            Move::Drop => {
                sim.pop();
            }
            Move::Swap => sim.swap(n - 1, n - 2),
            Move::Over => sim.push(sim[n - 2].clone()),
            Move::Rot => {
                let a = sim.remove(n - 3);
                sim.push(a);
            }
            Move::Nip => {
                sim.remove(n - 2);
            }
            Move::Tuck => sim.insert(n - 2, sim[n - 1].clone()),
            Move::Add | Move::Sub | Move::Mul | Move::Lt | Move::Gt => {
                let b = sim.pop().unwrap().num();
                let a = sim.pop().unwrap().num();
                sim.push(match self {
                    Move::Add => K::Num(a.checked_add(b).expect("bounded oracle arithmetic")),
                    Move::Sub => K::Num(a.checked_sub(b).expect("bounded oracle arithmetic")),
                    Move::Mul => K::Num(a.checked_mul(b).expect("bounded oracle arithmetic")),
                    Move::Lt => K::Bool(a < b),
                    _ => K::Bool(a > b),
                });
            }
            Move::Call => match sim.pop().unwrap() {
                K::Lit(xs) => sim.extend(xs.into_iter().map(K::Num)),
                K::FnMap(k) => {
                    let a = sim.pop().unwrap().num();
                    sim.push(K::Num(a + i128::from(k % 10)));
                }
                K::FnFold => {
                    let b = sim.pop().unwrap().num();
                    let a = sim.pop().unwrap().num();
                    sim.push(K::Num(a + b));
                }
                K::FnEach => {
                    sim.pop();
                }
                K::FnFilter(k) => {
                    let a = sim.pop().unwrap().num();
                    sim.push(K::Bool(a > i128::from(k % 10)));
                }
                _ => unreachable!("CALL precondition"),
            },
            Move::Dip => {
                let K::FnMap(k) = sim.pop().unwrap() else {
                    unreachable!()
                };
                let saved = sim.pop().unwrap();
                let a = sim.pop().unwrap().num();
                sim.extend([K::Num(a + i128::from(k % 10)), saved]);
            }
            Move::Map => {
                let K::FnMap(k) = sim.pop().unwrap() else {
                    unreachable!()
                };
                let xs = sim.pop().unwrap().numbers();
                sim.push(K::List(
                    xs.into_iter().map(|a| a + i128::from(k % 10)).collect(),
                ));
            }
            Move::Fold => {
                sim.pop();
                let init = sim.pop().unwrap().num();
                let xs = sim.pop().unwrap().numbers();
                sim.push(K::Num(init + xs.iter().sum::<i128>()));
            }
            Move::Each => {
                sim.pop();
                sim.pop();
            }
            Move::Filter => {
                let K::FnFilter(k) = sim.pop().unwrap() else {
                    unreachable!()
                };
                let xs = sim.pop().unwrap().numbers();
                sim.push(K::List(
                    xs.into_iter().filter(|&a| a > i128::from(k % 10)).collect(),
                ));
            }
            Move::IfConst(p) | Move::IfXform(p) => {
                let K::Bool(cond) = sim.pop().unwrap() else {
                    unreachable!()
                };
                let a = i128::from(p & 0xf);
                let b = i128::from((p >> 4) & 0xf);
                let value = if matches!(self, Move::IfConst(_)) {
                    if cond { a } else { b }
                } else {
                    let x = sim.pop().unwrap().num();
                    if cond { x + a } else { x * b }
                };
                sim.push(K::Num(value));
            }
        }
    }

    /// Render the move as source. Seeded pushes derive their lexemes from the
    /// seed so shrinking also shrinks the literals.
    fn emit(self, out: &mut String) {
        match self {
            Move::Boundary(k) => {
                out.push(' ');
                out.push_str(&boundary_case(k).0);
            }
            Move::PushNum(k) => out.push_str(&format!(" {}", k % 10)),
            Move::PushLit(sz) => {
                out.push_str(" [");
                for i in 0..(sz % 4) {
                    out.push_str(&format!(" {}", i + 1));
                }
                out.push_str(" ]");
            }
            Move::PushFnMap(k) => out.push_str(&format!(" [ {} + ]", k % 10)),
            Move::PushFnFold => out.push_str(" [ + ]"),
            Move::PushFnEach => out.push_str(" [ DROP ]"),
            Move::PushFnFilter(k) => out.push_str(&format!(" [ {} > ]", k % 10)),
            Move::PushBool(k) => out.push_str(if k % 2 == 0 { " true" } else { " false" }),
            Move::Dup => out.push_str(" DUP"),
            Move::Drop => out.push_str(" DROP"),
            Move::Swap => out.push_str(" SWAP"),
            Move::Over => out.push_str(" OVER"),
            Move::Rot => out.push_str(" ROT"),
            Move::Nip => out.push_str(" NIP"),
            Move::Tuck => out.push_str(" TUCK"),
            Move::Add => out.push_str(" +"),
            Move::Sub => out.push_str(" -"),
            Move::Mul => out.push_str(" *"),
            Move::Lt => out.push_str(" <"),
            Move::Gt => out.push_str(" >"),
            Move::Call => out.push_str(" CALL"),
            Move::Dip => out.push_str(" DIP"),
            Move::Map => out.push_str(" MAP"),
            Move::Fold => out.push_str(" FOLD"),
            Move::Each => out.push_str(" EACH"),
            Move::Filter => out.push_str(" FILTER"),
            Move::IfConst(p) => {
                out.push_str(&format!(" [ {} ] [ {} ] IF", p & 0xf, (p >> 4) & 0xf))
            }
            Move::IfXform(p) => {
                out.push_str(&format!(" [ {} + ] [ {} * ] IF", p & 0xf, (p >> 4) & 0xf))
            }
        }
    }
}

/// Map a seed vector to a valid program: at each step, select uniformly among
/// the currently-legal moves; seeded pushes take their payload from the seed's
/// high bits so a single `u64` decides both *which* move and *what* literal.
fn seeds_to_source(seeds: &[u64]) -> (String, Vec<Value>) {
    let mut sim: Vec<K> = Vec::new();
    let mut src = String::new();
    for &seed in seeds {
        let legal: Vec<Move> = ALL_MOVES
            .iter()
            .copied()
            .filter(|m| m.legal(&sim))
            .collect();
        debug_assert!(!legal.is_empty(), "pushes are always legal");
        let payload = (seed >> 32) as u8;
        let mut mv = legal[(seed as usize) % legal.len()];
        mv = with_payload(mv, payload);
        mv.apply(&mut sim);
        mv.emit(&mut src);
    }
    (src.trim().to_string(), sim.iter().map(K::value).collect())
}

/// Thread the seed's payload byte into the payload-carrying moves.
fn with_payload(mv: Move, payload: u8) -> Move {
    match mv {
        Move::Boundary(_) => Move::Boundary(payload),
        Move::PushNum(_) => Move::PushNum(payload),
        Move::PushLit(_) => Move::PushLit(payload),
        Move::PushFnMap(_) => Move::PushFnMap(payload),
        Move::PushFnFilter(_) => Move::PushFnFilter(payload),
        Move::PushBool(_) => Move::PushBool(payload),
        Move::IfConst(_) => Move::IfConst(payload),
        Move::IfXform(_) => Move::IfXform(payload),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// The three-way harness: gate, runtime, shadow.
// ---------------------------------------------------------------------------

fn evaluator_for(src: &str) -> Evaluator<Value> {
    let mut eval: Evaluator<Value> = Evaluator::new();
    register_all_builtins(&mut eval);
    let wrapped = format!("[ 1 + ] :inc [ inc inc ] :twice [ {src} ] :main");
    let tokens = parse_with_spans(&wrapped).expect("generated source parses");
    eval.load_with_spans(&tokens)
        .expect("generated source loads");
    eval
}

/// Run the shadow evaluator over `src` exactly as the whole-program gate does:
/// the production `SigResolver` over the evaluator's definition schemes and
/// registered contracts (with the language-core schemes as its built-in
/// fallback), a fresh embedded solver, an empty shadow stack.
fn shadow_stack_for(
    eval: &Evaluator<Value>,
    src: &str,
) -> Result<(ShadowStack, Vec<Pred>), ShadowError> {
    let schemes = definition_schemes(eval).expect("gate accepted, Tier 0 is green");
    let arrows = |w: &str| -> Option<WordTy> {
        if let Some(scheme) = schemes.get(w) {
            return Some(scheme.ty.clone());
        }
        eval.contract(w).map(|s| s.ty.clone())
    };
    let lookup = |w: &str| eval.refinement(w).cloned();
    let definitions: BTreeSet<String> = eval.definition_names().map(str::to_owned).collect();
    let resolve = SigResolver::with_arrows_and_definitions(&lookup, &arrows, &definitions);
    let mut stack = ShadowStack::new();
    let mut solver = SmtLibSolver::new();
    let mut ctx = VerifyCtx::new();
    let tokens = parse(src).expect("generated source parses");
    verify_ctx(&tokens, &mut stack, &mut solver, &resolve, &mut ctx)?;
    assert!(ctx.obligations().iter().all(Obligation::is_discharged));
    Ok((stack, solver.live_facts()))
}

/// The conformance relations C1–C3 between one runtime stack and one shadow
/// stack. Returns a human-readable violation, or `None` if conformant.
fn conformance_violation(
    runtime: &[Value],
    shadow: &ShadowStack,
    facts: &[Pred],
) -> Option<String> {
    // C1: depth.
    if runtime.len() != shadow.len() {
        return Some(format!(
            "C1 depth: runtime {} vs shadow {}",
            runtime.len(),
            shadow.len()
        ));
    }
    // Resolve guarantees independently of the solver. Bind remaining final-slot
    // variables to actual runtime observations, then check every ground fact.
    let mut bindings = BTreeMap::new();
    resolve_fact_bindings(facts, &mut bindings);
    for (i, (rt, sh)) in runtime.iter().zip(shadow.slots()).enumerate() {
        match sh {
            Slot::Quote(_) if !matches!(rt, Value::Bracket(_)) => {
                return Some(format!("C2 kind at slot {i}: quotation over {rt:?}"));
            }
            Slot::Term(term) => {
                if let Some(claim) = concrete_term(term, &bindings) {
                    let agrees = match (&claim, rt) {
                        (Pred::Num(s), Value::Word(w)) => numeric_equal(s, w),
                        (Pred::Var(b), Value::Word(w)) => b == w,
                        _ => false,
                    };
                    if !agrees {
                        return Some(format!("C3 value at slot {i}: {claim:?} over {rt:?}"));
                    }
                } else if let (Pred::Var(name), Value::Word(w)) = (term, rt) {
                    let value = if w == "true" || w == "false" {
                        Pred::Var(w.clone())
                    } else {
                        Pred::Num(w.clone())
                    };
                    bindings.insert(name.clone(), value);
                }
            }
            _ => {}
        }
    }
    resolve_fact_bindings(facts, &mut bindings);
    for fact in facts {
        if concrete_term(fact, &bindings) == Some(Pred::Var("false".into())) {
            return Some(format!("C3 runtime violates refinement fact {fact:?}"));
        }
    }
    None
}

/// The whole three-way check for one generated body.
fn check_conformance(src: &str, expected: &[Value]) -> Result<(), TestCaseError> {
    let eval = evaluator_for(src);

    // C0: the production gate accepts every well-formed-by-construction
    // program. (This is what the ShadowWord::Var arity bug broke.)
    let ledger = check_whole_program(&eval, SmtLibSolver::new);
    prop_assert!(
        ledger.is_ok(),
        "C0 gate rejected a well-formed program: {:?}\n  source: {src}",
        ledger.err()
    );

    // Runtime.
    let tokens = parse(src).expect("generated source parses");
    let runtime = eval.eval(&tokens);
    prop_assert!(
        runtime.is_ok(),
        "runtime rejected a well-formed program: {:?}\n  source: {src}",
        runtime.err()
    );
    let runtime = runtime.unwrap();

    // Shadow, under the production resolver.
    let shadow = shadow_stack_for(&eval, src);
    prop_assert!(
        shadow.is_ok(),
        "shadow evaluator rejected a gate-green program: {:?}\n  source: {src}",
        shadow.err()
    );
    let (shadow, facts) = shadow.unwrap();

    let effect = type_check(&eval).expect("Tier 0 accepted");
    prop_assert!(
        runtime_has_types(&runtime, &effect.output.elems),
        "runtime output violates Tier 0: {src}"
    );
    prop_assert_eq!(
        runtime
            .iter()
            .flat_map(Value::to_tokens)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .flat_map(Value::to_tokens)
            .collect::<Vec<_>>(),
        "runtime differs from concrete generator oracle: {}",
        src
    );

    if let Some(violation) = conformance_violation(&runtime, &shadow, &facts) {
        prop_assert!(
            false,
            "shadow/runtime divergence — {violation}\n  source: {src}\n  runtime: {runtime:?}\n  shadow: {:?}",
            shadow.slots()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Properties.
// ---------------------------------------------------------------------------

proptest! {
    /// C0–C3 over the full vocabulary.
    #[test]
    fn shadow_conforms_to_runtime(seeds in proptest::collection::vec(any::<u64>(), 0..24)) {
        let (src, expected) = seeds_to_source(&seeds);
        check_conformance(&src, &expected)?;
    }

    /// The same relations under a generator biased toward the sequence
    /// combinators (every third move forced into the {CALL, DIP, MAP, FOLD,
    /// EACH} subset when one is legal): the opaque-arrow path is the seam the
    /// regression lived on, so it gets its own concentrated budget.
    #[test]
    fn shadow_conforms_under_combinator_pressure(
        seeds in proptest::collection::vec(any::<u64>(), 0..24)
    ) {
        let mut sim: Vec<K> = Vec::new();
            let mut src = String::new();
        for (i, &seed) in seeds.iter().enumerate() {
            let combinators = [
                Move::Call,
                Move::Dip,
                Move::Map,
                Move::Fold,
                Move::Each,
                Move::Filter,
                Move::IfConst(0),
                Move::IfXform(0),
            ];
            let pool: Vec<Move> = if i % 3 == 2 {
                let hot: Vec<Move> = combinators
                    .iter()
                    .copied()
                    .filter(|m| m.legal(&sim))
                    .collect();
                if hot.is_empty() {
                    ALL_MOVES.iter().copied().filter(|m| m.legal(&sim)).collect()
                } else {
                    hot
                }
            } else {
                ALL_MOVES.iter().copied().filter(|m| m.legal(&sim)).collect()
            };
            let payload = (seed >> 32) as u8;
            let mv = with_payload(pool[(seed as usize) % pool.len()], payload);
            mv.apply(&mut sim);
            mv.emit(&mut src);
        }
        check_conformance(src.trim(), &sim.iter().map(K::value).collect::<Vec<_>>())?;
    }
}

// ---------------------------------------------------------------------------
// Deterministic anchors: the known regression shapes, through the same
// three-way harness rather than bespoke assertions, so the harness itself is
// pinned to the bug class it was built for.
// ---------------------------------------------------------------------------

#[test]
fn anchor_dual_purpose_dup_map_then_call() {
    // The original repro: one DUP copy consumed as a List by MAP, the other
    // CALLed as a quotation. Pre-fix, MAP resolved to `ShadowWord::Var`
    // (net +1 vs the runtime's net −1) and the shadow data flow diverged.
    check_anchor("[ 1 2 3 ] DUP [ 1 + ] MAP DROP CALL", "1 2 3").expect("conformant");
}

#[test]
fn anchor_fold_preserves_the_slot_beneath() {
    // Net −2 through FOLD with a known numeric beneath: C3 asserts the shadow
    // still knows `7` at the surviving slot — the §10.2 binding-correctness
    // observable.
    check_anchor("7 [ 1 2 3 ] 0 [ + ] FOLD DROP", "7").expect("conformant");
}

#[test]
fn anchor_each_drains_and_dip_shields() {
    check_anchor("7 [ 1 2 ] [ DROP ] EACH 5 [ 3 + ] DIP", "10 5").expect("conformant");
}

#[test]
fn anchor_if_join_hides_branch_conditional_values() {
    // SOUNDNESS anchor (§10.4). The runtime takes the else branch; the shadow
    // must not claim the then-branch's `5` — pre-join, C3 refutes this program
    // directly (shadow `Num("5")` over runtime `Word("0")`).
    check_anchor("false [ 5 ] [ 0 ] IF", "0").expect("conformant");
    check_anchor("true [ 5 ] [ 0 ] IF", "5").expect("conformant");
}

#[test]
fn anchor_if_join_keeps_branch_agreed_values() {
    // The join costs nothing where the branches agree: the shadow still knows
    // `5` (C3 compares it against whichever branch the runtime ran), and the
    // slot beneath the condition survives untouched.
    check_anchor("7 false [ 5 ] [ 5 ] IF", "7 5").expect("conformant");
}

#[test]
fn anchor_filter_and_predicate_call() {
    // FILTER's output is opaque to the shadow but depth-1 regardless of how
    // many elements survive the predicate at runtime; CALLing the predicate
    // directly produces a Bool the next IF can branch on.
    check_anchor(
        "[ 1 2 3 ] [ 2 > ] FILTER DROP 9 [ 2 > ] CALL [ 1 ] [ 2 ] IF",
        "1",
    )
    .expect("conformant");
}

fn check_anchor(src: &str, expected: &str) -> Result<(), TestCaseError> {
    let values = parse(expected)
        .unwrap()
        .into_iter()
        .map(Value::from)
        .collect::<Vec<_>>();
    check_conformance(src, &values)
}

fn runtime_has_types(runtime: &[Value], types: &[Ty]) -> bool {
    fn has_type(value: &Value, ty: &Ty) -> bool {
        match (&ty.kind, value) {
            (TyKind::Con(n), Value::Word(w)) if n == NUM => {
                w.parse::<f64>().is_ok_and(f64::is_finite)
            }
            (TyKind::Con(n), Value::Word(w)) if n == BOOL => w == "true" || w == "false",
            (TyKind::App(n, args), Value::Bracket(body)) if n == "List" && args.len() == 1 => {
                quote_items_to_values(body)
                    .iter()
                    .all(|v| has_type(v, &args[0]))
            }
            (TyKind::Quote(_), Value::Bracket(_)) | (TyKind::Var(_), _) => true,
            _ => false,
        }
    }
    runtime.len() == types.len() && runtime.iter().zip(types).all(|(v, t)| has_type(v, t))
}

#[test]
fn concrete_oracle_rejects_wrong_values_and_types() {
    for (src, wrong) in [
        ("true", "false"),
        ("1 2 +", "false"),
        ("1 2 <", "false"),
        ("[ 1 2 ] [ 1 + ] MAP", "[ false ]"),
        ("false [ 5 ] [ 0 ] IF", "999"),
        ("[ 1 ]", "[ true ]"),
    ] {
        assert!(
            check_anchor(src, wrong).is_err(),
            "oracle accepted {src} => {wrong}"
        );
    }
    let eval = evaluator_for("[ 1 2 ] [ 1 + ] MAP");
    let ty = type_check(&eval).unwrap();
    let wrong = vec![Value::from(parse("[ false ]").unwrap().remove(0))];
    assert!(!runtime_has_types(&wrong, &ty.output.elems));
}

// Only closed, bounded integer/boolean expressions are evaluated here. Opaque
// terms remain unknown and are covered by the generator's concrete oracle.
fn concrete_term(term: &Pred, bindings: &BTreeMap<String, Pred>) -> Option<Pred> {
    let bool_term = |b: bool| Pred::Var(b.to_string());
    match term {
        Pred::Num(_) => Some(term.clone()),
        Pred::Var(w) if w == "true" || w == "false" => Some(term.clone()),
        Pred::Var(w) => bindings.get(w).cloned(),
        Pred::Un(op, a) => match (op, concrete_term(a, bindings)?) {
            (UnOp::Not, Pred::Var(b)) => Some(bool_term(b == "false")),
            (UnOp::Neg, Pred::Num(n)) => Some(Pred::Num(
                n.parse::<i128>().ok()?.checked_neg()?.to_string(),
            )),
            _ => None,
        },
        Pred::Bin(op, a, b) => {
            let a = concrete_term(a, bindings)?;
            let b = concrete_term(b, bindings)?;
            match (&a, &b) {
                (Pred::Num(a), Pred::Num(b)) => {
                    let a = a.parse::<i128>().ok()?;
                    let b = b.parse::<i128>().ok()?;
                    let n = match op {
                        BinOp::Add => a.checked_add(b)?,
                        BinOp::Sub => a.checked_sub(b)?,
                        BinOp::Mul => a.checked_mul(b)?,
                        BinOp::Div if a.checked_rem(b)? == 0 => a.checked_div(b)?,
                        BinOp::Eq => return Some(bool_term(a == b)),
                        BinOp::Lt => return Some(bool_term(a < b)),
                        BinOp::Gt => return Some(bool_term(a > b)),
                        BinOp::Le => return Some(bool_term(a <= b)),
                        BinOp::Ge => return Some(bool_term(a >= b)),
                        _ => return None,
                    };
                    Some(Pred::Num(n.to_string()))
                }
                (Pred::Var(a), Pred::Var(b)) => Some(bool_term(match op {
                    BinOp::Eq => a == b,
                    BinOp::And => a == "true" && b == "true",
                    BinOp::Or => a == "true" || b == "true",
                    BinOp::Implies => a == "false" || b == "true",
                    _ => return None,
                })),
                _ => None,
            }
        }
        Pred::App(_, _) => None,
    }
}

fn resolve_fact_bindings(facts: &[Pred], bindings: &mut BTreeMap<String, Pred>) {
    loop {
        let before = bindings.len();
        for fact in facts {
            if let Pred::Bin(BinOp::Eq, a, b) = fact {
                for (lhs, rhs) in [(a, b), (b, a)] {
                    if let Pred::Var(name) = lhs.as_ref()
                        && name != "true"
                        && name != "false"
                        && !bindings.contains_key(name)
                        && let Some(value) = concrete_term(rhs, bindings)
                    {
                        bindings.insert(name.clone(), value);
                    }
                }
            }
        }
        if bindings.len() == before {
            break;
        }
    }
}

#[test]
fn production_refinements_and_definition_precedence_are_observed() {
    let eval = evaluator_for("1 2 +");
    let (shadow, facts) = shadow_stack_for(&eval, "1 2 +").unwrap();
    assert!(matches!(shadow.top(), Some(Slot::Term(Pred::Var(_)))));
    assert!(!facts.is_empty());
    assert!(conformance_violation(&[Value::Word("4".into())], &shadow, &facts).is_some());
    assert!(conformance_violation(&[Value::Word("3".into())], &shadow, &facts).is_none());

    let mut eval = evaluator_for("DUP");
    eval.load_with_spans(&parse_with_spans("[ 7 ] :DUP").unwrap())
        .unwrap();
    check_whole_program(&eval, SmtLibSolver::new).unwrap();
    let (shadow, facts) = shadow_stack_for(&eval, "DUP").unwrap();
    let runtime = eval.eval(&parse("DUP").unwrap()).unwrap();
    assert_eq!(runtime, vec![Value::Word("7".into())]);
    assert!(conformance_violation(&runtime, &shadow, &facts).is_none());
}

/// Compare numeric lexemes exactly, including equivalent decimal/exponent forms.
/// Keeping the significand as digits avoids both f64 rounding and i128 overflow.
fn numeric_equal(a: &str, b: &str) -> bool {
    fn normalized(s: &str) -> Option<(bool, String, i64)> {
        let negative = s.starts_with('-');
        let s = s.strip_prefix(['-', '+']).unwrap_or(s);
        let (mantissa, exponent) = match s.split_once(['e', 'E']) {
            Some((m, e)) => (m, e.parse::<i64>().ok()?),
            None => (s, 0),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits = format!("{whole}{fraction}");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return Some((false, "0".into(), 0));
        }
        let trimmed = digits.trim_end_matches('0');
        let scale = exponent
            .checked_sub(i64::try_from(fraction.len()).ok()?)?
            .checked_add(i64::try_from(digits.len() - trimmed.len()).ok()?)?;
        Some((negative, trimmed.into(), scale))
    }
    match (normalized(a), normalized(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[test]
fn numeric_claims_preserve_exactness() {
    for (claimed, actual, agrees) in [
        ("9007199254740992", "9007199254740993", false),
        (
            "170141183460469231731687303715884105727",
            "170141183460469231731687303715884105726",
            false,
        ),
        ("1", "1.0", true),
        ("9007199254740993", "9.007199254740993e15", true),
        ("-0.0", "0", true),
        ("NaN", "NaN", false),
        ("inf", "inf", false),
    ] {
        let mut shadow = ShadowStack::new();
        shadow.push_term(Pred::Num(claimed.into()));
        let runtime = [Value::Word(actual.into())];
        assert_eq!(
            conformance_violation(&runtime, &shadow, &[]).is_none(),
            agrees,
            "{claimed} vs {actual}"
        );
    }
}

/// Closed boundary fragments compose with every stack prefix. Each source is
/// paired with its result by construction, without executing its body as oracle.
fn boundary_case(payload: u8) -> (String, K) {
    let k = i128::from(payload % 10);
    let literal = |src: String| K::Data(Value::from(parse(&src).unwrap().remove(0)));
    match payload % 20 {
        0 => (format!("[ {k} >x [ x 1 + ] CALL ] CALL"), K::Num(k + 1)),
        1 => (
            format!("[ {k} >x [ x ] [ false >x CALL ] CALL ] CALL"),
            K::Num(k),
        ),
        2 => (
            format!("[ {k} >x [ false >x [ x not ] CALL ] CALL ] CALL"),
            K::Bool(true),
        ),
        3 => (format!("{k} twice"), K::Num(k + 2)),
        4 => (format!("[ [ {k} ] ] CALL CALL"), K::Num(k)),
        5 => (format!("{k} [ 1 + ] CURRY CALL"), K::Num(k + 1)),
        6 => (
            format!("{k} [ 1 + ] [ 2 * ] COMPOSE CALL"),
            K::Num((k + 1) * 2),
        ),
        7 => (format!("{k} [ 1 + ] KEEP +"), K::Num(k * 2 + 1)),
        8 => (format!("{k} [ 1 + ] [ 2 * ] BI -"), K::Num(1 - k)),
        9 => (format!("{k} 2 [ 1 + ] [ 3 * ] BI* -"), K::Num(k - 5)),
        10 => (format!("{k} 2 [ 1 + ] BI@ +"), K::Num(k + 4)),
        11 => (format!("{k} true [ 1 + ] WHEN"), K::Num(k + 1)),
        12 => (format!("{k} false [ 2 * ] UNLESS"), K::Num(k * 2)),
        13 => (
            format!("[ [ {k} ] [ ] ] [ [ 1 + ] MAP ] MAP"),
            literal(format!("[ [ {} ] [ ] ]", k + 1)),
        ),
        14 => (
            "[ true false ] [ not ] MAP".into(),
            literal("[ false true ]".into()),
        ),
        15 => (
            format!("[ {k} ] [ 0 > ] MAP"),
            literal(format!("[ {} ]", k > 0)),
        ),
        16 => (format!("{k} 1 2DUP 2DROP +"), K::Num(k + 1)),
        17 => (
            format!("{k} true false [ 1 + ] 2DIP DROP DROP"),
            K::Num(k + 1),
        ),
        18 => (
            format!("{k} true false 5 [ 1 + ] 3DIP DROP DROP DROP"),
            K::Num(k + 1),
        ),
        _ => (
            format!("true [ [ {k} ] ] [ [ {} ] ] IF CALL", k + 1),
            K::Num(k),
        ),
    }
}

#[test]
fn boundary_families_have_concrete_observations() {
    for payload in 0..20 {
        let (src, expected) = boundary_case(payload);
        check_conformance(&src, &[expected.value()]).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
}

proptest! {
    /// Runtime rejection and Tier-0/gate rejection are distinct observations.
    /// These cases pin the documented Num-domain and rank-1 exceptions rather
    /// than silently excluding them from a claim that green means crash-free.
    #[test]
    fn numeric_boundaries_and_rejection_classes(
        n in prop::sample::select(vec![i128::MIN, i128::MIN + 1, -9007199254740993, -1, 0, 1, 9007199254740993, i128::MAX - 1, i128::MAX]),
        fraction in 0u8..10,
        invalid in 0usize..5,
    ) {
        check_anchor(&format!("{n} 0 +"), &n.to_string())?;
        let src = format!("{fraction}.5 ~");
        let eval = evaluator_for(&src);
        prop_assert!(type_check(&eval).is_ok());
        prop_assert!(check_whole_program(&eval, SmtLibSolver::new).is_ok());
        prop_assert!(eval.eval(&parse(&src).unwrap()).is_err());

        let (src, tier0, gate) = [
            ("170141183460469231731687303715884105727 1 +", true, true),
            ("1 2 [ SWAP ] BI@", true, false),
            ("true 1 +", false, false),
            ("DROP", false, false),
            ("1 0 /", true, false),
        ][invalid];
        let eval = evaluator_for(src);
        prop_assert_eq!(type_check(&eval).is_ok(), tier0);
        prop_assert_eq!(check_whole_program(&eval, SmtLibSolver::new).is_ok(), gate);
        prop_assert!(eval.eval(&parse(src).unwrap()).is_err());
    }
}

#[test]
fn copied_quotations_are_generated_and_observed_at_distinct_uses() {
    assert!(Move::Call.legal(&[K::Lit(vec![1]), K::Lit(vec![1])]));
    assert!(Move::Call.legal(&[K::Bool(true), K::FnEach]));
    for (src, expected) in [
        ("[ 1 ] DUP CALL", "[ 1 ] 1"),
        ("[ 1 ] DUP CALL SWAP CALL", "1 1"),
        ("[ DROP ] DUP 1 SWAP CALL true SWAP CALL", ""),
        ("true 5 [ 7 + ] DUP DIP ROT DROP CALL", "19"),
    ] {
        check_anchor(src, expected).unwrap();
    }
}
