//! Harness 3 — Shadow/Runtime Data-Flow Conformance (proptest).
//!
//! §10.3 / invariant 7 states that the Tier-1 shadow evaluator owns **no
//! independent notion of arity**: every word moves data per its Tier 0 arrow,
//! so the shadow stack's shape mirrors the runtime stack's shape byte for byte.
//! The existing tests pin that per-operator (MAP at net −1, FOLD at net −2, the
//! shuffles). This harness pins the **class**: for every program in a
//! well-formed-by-construction vocabulary, the shadow stack after `verify_ctx`
//! and the runtime stack after `eval` must agree.
//!
//! ## The generator
//!
//! Seed-directed, typed-by-construction: a `Vec<u64>` of proptest-generated
//! seeds is deterministically mapped to moves, where each seed selects among
//! the moves whose *precondition the simulated stack currently satisfies*.
//! Every emitted program is therefore Tier-0 green and runtime-safe by
//! construction (no underflow, no type error, no division), while proptest's
//! shrinking of the seed vector shrinks the *program* — shorter vectors are
//! shorter programs, and the seed→move mapping keeps every shrunk prefix valid.
//!
//! The simulated kinds distinguish what the conformance relation needs:
//! `Num`, `Lit(n)` (a dual-purpose literal bracket of `n` numeric elements —
//! usable as a `List` by the sequence combinators and CALLable as a quotation,
//! the seam the HEAD commit opened), the function-quotation kinds (`FnMap :
//! (Num -- Num)`, `FnFold : (Num Num -- Num)`, `FnEach : (a -- )`), and `List`
//! (an opaque combinator output). The vocabulary is the three-layer
//! intersection — every word has a Tier 0 scheme (core or registered contract),
//! a shadow action (core shadow word, interpreted op, or `Opaque` via the
//! arrow), and a runtime builtin:
//!
//! - literals: numeric pushes, `true`/`false`, literal brackets, the four
//!   function quotations (`FnFilter : (Num -- Bool)` joined in with `Bool`)
//! - shuffles: `DUP DROP SWAP OVER ROT NIP TUCK`
//! - arithmetic and comparison: `+ - * < >` (`/` and `%` remain excluded — the
//!   generator would have to prove nonzero divisors; a refinement-aware
//!   extension can add them)
//! - combinators: `CALL` (on literal brackets and on each function kind),
//!   `DIP`, `MAP`, `FOLD`, `EACH`, `FILTER`, and `IF` in two arm families —
//!   `[ a ] [ b ] IF : ( 'S Bool -- 'S Num )` and
//!   `[ a + ] [ b * ] IF : ( 'S Num Bool -- 'S Num )` — with arm literals drawn
//!   from the seed so agreed and disagreed arms both occur.
//!
//! ## The property (three conformance relations, one per strength)
//!
//! For a generated body `P`, let `R` = runtime stack after `eval(P)` and `S` =
//! shadow stack after `verify_ctx(P)` under the **production gate resolver**
//! (`SigResolver::with_arrows` over the evaluator's registered contracts +
//! definition schemes, exactly as `check_whole_program` builds it):
//!
//! - **C0 (gate completeness):** `check_whole_program` accepts `[ P ] :main`.
//!   The pre-fix `ShadowWord::Var` arity bug violated exactly this — a
//!   Tier-0-green program died structurally in the shadow evaluator.
//! - **C1 (depth):** `|R| == |S|`. The direct observable of invariant 7.
//! - **C2 (kind, one-directional):** `S[i]` a quotation slot ⇒ `R[i]` a
//!   bracket value. The converse is deliberately not asserted: a sequence
//!   combinator's output is a bracket at runtime but an *opaque term* in the
//!   shadow (the shadow claims no structure it cannot prove) — opacity may
//!   weaken knowledge, never misstate kind.
//! - **C3 (value, where knowledge is claimed):** `S[i] = Term(Num(s))` ⇒
//!   `R[i]` is a numeric word equal to `s`. Fresh opaques are `Pred::Var`
//!   and interpreted-op results are `Pred::Bin`, so a `Pred::Num` slot is
//!   precisely the shadow claiming to *know the runtime value*; this is the
//!   §10.2 binding-correctness property — a refined demand zipped at slot `i`
//!   binds exactly the value the runtime would carry there.
//!
//! C3 is what makes the property sharper than a depth check: the pre-fix bug
//! class (mis-shuffled slots) can conspire to preserve depth while binding the
//! wrong term, which C3 catches whenever either the misplaced or the displaced
//! slot is a known numeric.
//!
//! Under `IF`, C3 is exactly the relation that demands the **branch join**
//! (§10.4, `ShadowStack::join_branch_states`): the shadow's post-IF stack may
//! keep a `Pred::Num` only where the branches agree on it, because the runtime
//! is free to take either branch. Advancing with one branch's concrete
//! post-state (the pre-join behavior) is refuted by any generated
//! `false [ a ] [ b ] IF` with `a != b` — and, one level up, let the gate
//! certify demands the runtime violates.

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

/// What the generator knows about a simulated stack slot — just enough to
/// state every move's precondition.
///
/// Quote kinds carry an **origin id**: `DUP`/`OVER`/`TUCK` copies share their
/// original's id because they share its *monomorphic type* (values are not
/// generalized). `CALL` and `DIP` bind the quotation's row variable against the
/// live stack, so a same-origin copy anywhere in that row makes the row contain
/// the quotation's own type — Tier 0's occurs check rejects it (correctly: the
/// classic `[ DUP CALL ] DUP CALL` shape). The runtime would execute such
/// programs happily; this is a *completeness* boundary of monomorphic Tier 0,
/// not a conformance divergence, so the generator stays inside it. `MAP`/
/// `FOLD`/`EACH` unify the function against an abstract element row, not the
/// live stack, so they carry no such restriction.
#[derive(Debug, Clone, Copy, PartialEq)]
enum K {
    /// A numeric scalar.
    Num,
    /// A literal bracket of `n` numeric elements (List-usable *and* CALLable),
    /// tagged with its origin id.
    Lit(usize, u32),
    /// `[ k + ] : (Num -- Num)` — a MAP/DIP/CALL-able transform.
    FnMap(u32),
    /// `[ + ] : (Num Num -- Num)` — a FOLD/CALL-able accumulator.
    FnFold(u32),
    /// `[ DROP ] : (a -- )` — an EACH/CALL-able consumer.
    FnEach(u32),
    /// `[ k > ] : (Num -- Bool)` — a FILTER/CALL-able predicate.
    FnFilter(u32),
    /// An opaque sequence-combinator output (`List Num` to Tier 0).
    List,
    /// A boolean (`true`/`false` literal or a comparison's result).
    Bool,
}

impl K {
    fn list_like(self) -> bool {
        matches!(self, K::Lit(_, _) | K::List)
    }

    /// The origin id, for quote kinds.
    fn origin(self) -> Option<u32> {
        match self {
            K::Lit(_, id) | K::FnMap(id) | K::FnFold(id) | K::FnEach(id) | K::FnFilter(id) => {
                Some(id)
            }
            K::Num | K::List | K::Bool => None,
        }
    }
}

/// Does any slot in `row` share `q`'s origin? (`q` must be a quote kind.)
fn origin_in_row(q: K, row: &[K]) -> bool {
    let id = q.origin().expect("caller passes a quote kind");
    row.iter().any(|k| k.origin() == Some(id))
}

/// One generated move: the source tokens it emits and its simulated effect.
/// `emit` renders source; `apply` transforms the simulated stack. The two are
/// kept adjacent in one `match` so a vocabulary extension cannot update one
/// without the other staring at it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Move {
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
        let top = |i: usize| sim[n - 1 - i];
        match self {
            Move::PushNum(_)
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
                n >= 2 && top(0) == K::Num && top(1) == K::Num
            }
            // CALL binds the quotation's input row against everything beneath
            // it — a same-origin copy in that row trips the occurs check.
            Move::Call => {
                n >= 1
                    && match top(0) {
                        q @ K::Lit(_, _) => !origin_in_row(q, &sim[..n - 1]),
                        q @ K::FnMap(_) => {
                            n >= 2 && top(1) == K::Num && !origin_in_row(q, &sim[..n - 1])
                        }
                        q @ K::FnFold(_) => {
                            n >= 3
                                && top(1) == K::Num
                                && top(2) == K::Num
                                && !origin_in_row(q, &sim[..n - 1])
                        }
                        // `[ DROP ] : ('q a -- 'q)` is the one *polymorphic*
                        // function kind, and copies share a monomorphic type:
                        // an EACH on one copy pins `a := Num` (every generated
                        // list is `List Num`), and a CALL on a surviving copy
                        // inherits that pin. Requiring a `Num` beneath makes
                        // every use pin `a` identically, so shared copies never
                        // conflict. (Double-CALL row conflicts are already
                        // impossible: the occurs rule blocks a CALL while a
                        // same-origin copy sits in the bound row.)
                        q @ K::FnEach(_) => {
                            n >= 2 && top(1) == K::Num && !origin_in_row(q, &sim[..n - 1])
                        }
                        q @ K::FnFilter(_) => {
                            n >= 2 && top(1) == K::Num && !origin_in_row(q, &sim[..n - 1])
                        }
                        K::Num | K::List | K::Bool => false,
                    }
            }
            // `DIP : ('S x q -- 'T x)` with q = FnMap needs a Num beneath the
            // shielded slot; the quotation's row binds `'S` = the stack below
            // the shield, so the occurs constraint applies to `sim[..n-2]`.
            Move::Dip => {
                n >= 3
                    && matches!(top(0), K::FnMap(_))
                    && top(2) == K::Num
                    && !origin_in_row(top(0), &sim[..n - 2])
            }
            Move::Map => n >= 2 && matches!(top(0), K::FnMap(_)) && top(1).list_like(),
            Move::Fold => {
                n >= 3 && matches!(top(0), K::FnFold(_)) && top(1) == K::Num && top(2).list_like()
            }
            Move::Each => n >= 2 && matches!(top(0), K::FnEach(_)) && top(1).list_like(),
            Move::Filter => {
                n >= 2 && matches!(top(0), K::FnFilter(_)) && top(1).list_like()
            }
            // Both IF families emit their arms inline (freshly typed, so no
            // occurs concern); the condition must sit on top.
            Move::IfConst(_) => n >= 1 && top(0) == K::Bool,
            Move::IfXform(_) => n >= 2 && top(0) == K::Bool && top(1) == K::Num,
        }
    }

    /// Apply the move's simulated effect. Precondition: `self.legal(sim)`.
    /// `origin` mints a fresh id per pushed quotation (one monomorphic type
    /// per *push site occurrence*, exactly Tier 0's view).
    fn apply(self, sim: &mut Vec<K>, origin: &mut u32) {
        let mut fresh = || {
            *origin += 1;
            *origin
        };
        let n = sim.len();
        match self {
            Move::PushNum(_) => sim.push(K::Num),
            Move::PushLit(sz) => sim.push(K::Lit(usize::from(sz % 4), fresh())),
            Move::PushFnMap(_) => sim.push(K::FnMap(fresh())),
            Move::PushFnFold => sim.push(K::FnFold(fresh())),
            Move::PushFnEach => sim.push(K::FnEach(fresh())),
            Move::PushFnFilter(_) => sim.push(K::FnFilter(fresh())),
            Move::PushBool(_) => sim.push(K::Bool),
            Move::Dup => sim.push(sim[n - 1]),
            Move::Drop => {
                sim.pop();
            }
            Move::Swap => sim.swap(n - 1, n - 2),
            Move::Over => sim.push(sim[n - 2]),
            Move::Rot => {
                let a = sim.remove(n - 3);
                sim.push(a);
            }
            Move::Nip => {
                sim.remove(n - 2);
            }
            Move::Tuck => {
                let t = sim[n - 1];
                sim.insert(n - 2, t);
            }
            Move::Add | Move::Sub | Move::Mul => {
                sim.pop();
                sim.pop();
                sim.push(K::Num);
            }
            Move::Lt | Move::Gt => {
                sim.pop();
                sim.pop();
                sim.push(K::Bool);
            }
            Move::Call => match sim.pop().expect("legal checked") {
                K::Lit(sz, _) => sim.extend(std::iter::repeat_n(K::Num, sz)),
                K::FnMap(_) => {
                    // consumes the Num beneath, produces a Num: net zero.
                }
                K::FnFold(_) => {
                    sim.pop();
                }
                K::FnEach(_) => {
                    sim.pop();
                }
                K::FnFilter(_) => {
                    sim.pop();
                    sim.push(K::Bool);
                }
                K::Num | K::List | K::Bool => unreachable!("legal checked"),
            },
            Move::Dip => {
                // pops the quote; transforms the Num two-under (Num → Num, so
                // kind-invisible); keeps the shield.
                sim.pop();
            }
            Move::Map => {
                sim.pop();
                sim.pop();
                sim.push(K::List);
            }
            Move::Fold => {
                sim.pop();
                sim.pop();
                sim.pop();
                sim.push(K::Num);
            }
            Move::Each => {
                sim.pop();
                sim.pop();
            }
            Move::Filter => {
                sim.pop();
                sim.pop();
                sim.push(K::List);
            }
            Move::IfConst(_) => {
                sim.pop();
                sim.push(K::Num);
            }
            Move::IfXform(_) => {
                sim.pop();
                // the Num beneath is transformed Num -> Num: kind-invisible.
            }
        }
    }

    /// Render the move as source. Seeded pushes derive their lexemes from the
    /// seed so shrinking also shrinks the literals.
    fn emit(self, out: &mut String) {
        match self {
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
fn seeds_to_source(seeds: &[u64]) -> String {
    let mut sim: Vec<K> = Vec::new();
    let mut origin: u32 = 0;
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
        mv.apply(&mut sim, &mut origin);
        mv.emit(&mut src);
    }
    src.trim().to_string()
}

/// Thread the seed's payload byte into the payload-carrying moves.
fn with_payload(mv: Move, payload: u8) -> Move {
    match mv {
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
    let wrapped = format!("[ {src} ] :main");
    let tokens = parse_with_spans(&wrapped).expect("generated source parses");
    eval.load_with_spans(&tokens)
        .expect("generated source loads");
    eval
}

/// Run the shadow evaluator over `src` exactly as the whole-program gate does:
/// the production `SigResolver` over the evaluator's definition schemes and
/// registered contracts (with the language-core schemes as its built-in
/// fallback), a fresh embedded solver, an empty shadow stack.
fn shadow_stack_for(eval: &Evaluator<Value>, src: &str) -> Result<ShadowStack, ShadowError> {
    let schemes = definition_schemes(eval).expect("gate accepted, Tier 0 is green");
    let arrows = |w: &str| -> Option<WordTy> {
        if let Some(scheme) = schemes.get(w) {
            return Some(scheme.ty.clone());
        }
        eval.contract(w).map(|s| s.ty.clone())
    };
    let lookup = |_: &str| -> Option<RefinementSig> { None };
    let resolve = SigResolver::with_arrows(&lookup, &arrows);
    let mut stack = ShadowStack::new();
    let mut solver = SmtLibSolver::new();
    let mut ctx = VerifyCtx::new();
    let tokens = parse(src).expect("generated source parses");
    verify_ctx(&tokens, &mut stack, &mut solver, &resolve, &mut ctx)?;
    Ok(stack)
}

/// The conformance relations C1–C3 between one runtime stack and one shadow
/// stack. Returns a human-readable violation, or `None` if conformant.
fn conformance_violation(runtime: &[Value], shadow: &ShadowStack) -> Option<String> {
    // C1: depth.
    if runtime.len() != shadow.len() {
        return Some(format!(
            "C1 depth: runtime {} vs shadow {}",
            runtime.len(),
            shadow.len()
        ));
    }
    for (i, (rt, sh)) in runtime.iter().zip(shadow.slots().iter()).enumerate() {
        match sh {
            // C2: a shadow quotation slot must sit over a runtime bracket.
            Slot::Quote(_) => {
                if !matches!(rt, Value::Bracket(_)) {
                    return Some(format!(
                        "C2 kind at slot {i}: shadow quotation over runtime {rt:?}"
                    ));
                }
            }
            // C3: a shadow *known numeric* must equal the runtime value.
            Slot::Term(Pred::Num(s)) => {
                let claimed: f64 = s.parse().expect("shadow numeric lexeme parses");
                match rt {
                    Value::Word(w) if w.parse::<f64>() == Ok(claimed) => {}
                    other => {
                        return Some(format!(
                            "C3 value at slot {i}: shadow knows {s}, runtime holds {other:?}"
                        ));
                    }
                }
            }
            // Opaque (`Pred::Var`) and interpreted (`Pred::Bin`/`Un`/`App`)
            // terms claim no runtime-comparable knowledge.
            Slot::Term(_) => {}
        }
    }
    None
}

/// The whole three-way check for one generated body.
fn check_conformance(src: &str) -> Result<(), TestCaseError> {
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
    let shadow = shadow.unwrap();

    if let Some(violation) = conformance_violation(&runtime, &shadow) {
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
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// C0–C3 over the full vocabulary.
    #[test]
    fn shadow_conforms_to_runtime(seeds in proptest::collection::vec(any::<u64>(), 0..24)) {
        let src = seeds_to_source(&seeds);
        check_conformance(&src)?;
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
        let mut origin: u32 = 0;
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
            mv.apply(&mut sim, &mut origin);
            mv.emit(&mut src);
        }
        check_conformance(src.trim())?;
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
    check_conformance("[ 1 2 3 ] DUP [ 1 + ] MAP DROP CALL").expect("conformant");
}

#[test]
fn anchor_fold_preserves_the_slot_beneath() {
    // Net −2 through FOLD with a known numeric beneath: C3 asserts the shadow
    // still knows `7` at the surviving slot — the §10.2 binding-correctness
    // observable.
    check_conformance("7 [ 1 2 3 ] 0 [ + ] FOLD DROP").expect("conformant");
}

#[test]
fn anchor_each_drains_and_dip_shields() {
    check_conformance("7 [ 1 2 ] [ DROP ] EACH 5 [ 3 + ] DIP").expect("conformant");
}

#[test]
fn anchor_if_join_hides_branch_conditional_values() {
    // SOUNDNESS anchor (§10.4). The runtime takes the else branch; the shadow
    // must not claim the then-branch's `5` — pre-join, C3 refutes this program
    // directly (shadow `Num("5")` over runtime `Word("0")`).
    check_conformance("false [ 5 ] [ 0 ] IF").expect("conformant");
    check_conformance("true [ 5 ] [ 0 ] IF").expect("conformant");
}

#[test]
fn anchor_if_join_keeps_branch_agreed_values() {
    // The join costs nothing where the branches agree: the shadow still knows
    // `5` (C3 compares it against whichever branch the runtime ran), and the
    // slot beneath the condition survives untouched.
    check_conformance("7 false [ 5 ] [ 5 ] IF").expect("conformant");
}

#[test]
fn anchor_filter_and_predicate_call() {
    // FILTER's output is opaque to the shadow but depth-1 regardless of how
    // many elements survive the predicate at runtime; CALLing the predicate
    // directly produces a Bool the next IF can branch on.
    check_conformance("[ 1 2 3 ] [ 2 > ] FILTER DROP 9 [ 2 > ] CALL [ 1 ] [ 2 ] IF")
        .expect("conformant");
}
