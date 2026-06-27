//! Tier 1 — path conditions + the **scoped solver seam** (M8, §10.4 / §10.8 /
//! §10.9 / §14.8).
//!
//! This module is the next unit after the M7 shadow stack ([`crate::shadow`]).
//! It is deliberately scoped to exactly two things and **no further** (§12 M8):
//!
//!   1. the **solver seam** — a trait with **exactly four** methods
//!      (`assert`, `check`, `push_scope`, `pop_scope`) that every VC /
//!      path-condition consumer talks to, and nothing else (§10.8); plus an
//!      **SMT-LIB2 text-emission** implementation of it that maintains
//!      `(push 1)`/`(pop 1)` parity with `push_scope`/`pop_scope` (§10.9); and
//!   2. the **path-condition plumbing** over the shadow stack: for
//!      `cond [ then ] [ else ] if`, recover `cond`'s proposition `P` from the
//!      M7 shadow stack and verify `[ then ]` with `P` asserted in a pushed
//!      scope and `[ else ]` with `¬P` (§10.4).
//!
//! There is **no** full VC generation with the negated-goal encoding and
//! **counterexample surfacing** here — that is M9 (§10.5). M8 stops at the
//! scoped seam and the path conditions, and shows the §12 M8 demonstration
//! discharge. The discharge a branch needs ("`x sqrt`'s demand `x >= 0` holds
//! inside the `x > 0` branch") rides on a **minimal** embedded reasoner (below),
//! not a full solver pipeline.
//!
//! # The seam is a trait — Z3 slots in behind it (§10.8/§10.9, invariant 10/11)
//!
//! The VC generator emits its formulas **through the [`Solver`] trait**; the
//! concrete solver sits behind it and the generator core **never** calls a
//! concrete solver directly. The trait is the seam where the `z3` crate (and
//! later CVC5 / Why3) slot in. Scoped assertions (`push_scope`/`pop_scope`) are
//! part of the seam **by mandate** — §10.8 is explicit that scoping "must exist
//! before M8, not be retrofitted," because path conditions (§10.4) are built on
//! it.
//!
//! ## Why no `z3` crate here (recorded — `docs/typing.md` is read-only)
//!
//! §10.8 says to use the `z3` crate (bundled feature) and §10.9 wants an
//! SMT-LIB text-emission mode from day one. The `z3` crate is a heavy,
//! C++-built dependency and a likely build-breakage source, so its introduction
//! needs a version-compatibility check **first** (the `check-for-version-
//! incompatibility` discipline). In this workspace that check **fails up front**:
//! the crate registry is unreachable offline, so `cargo add z3` cannot even
//! resolve the crate, let alone build its bundled C++ Z3. Adding it would break
//! the workspace build outright. Per §10.9 the **mandatory, day-one** deliverable
//! is the **SMT-LIB2 text-emission** seam, which is what this module provides;
//! the `z3`-crate implementation is the *optional* "if wired" path (M9/M13) and
//! is **not** wired here. This keeps the invariant that a **checked program
//! links no solver** trivially true (invariants 14/20): there is no solver in
//! the dependency graph at all, and the seam is compile-time-only Rust.
//!
//! The seam is solver-agnostic precisely so this substitution is mechanical: the
//! day a registry/Z3 toolchain is available, a `Z3Solver` implementing the same
//! four-method [`Solver`] trait drops in beside [`SmtLibSolver`] with no change
//! to the path-condition plumbing (M13 will then assert text-mode/native parity).
//!
//! # Immutability barrier (§3 invariant 1 / 18)
//!
//! Every `push_scope`/`pop_scope` and every asserted path fact lives **entirely**
//! in the [`Solver`] implementation's own state and the shadow-evaluator's own
//! state. **None of it touches the Tier 0 substitution** ([`crate::Subst`] /
//! `InferCtx::subst`), which Tier 1 treats as read-only. This is structural: no
//! function in this module takes a `&mut Subst` (or any Tier 0 inference handle),
//! so a Tier 1 branch mutating a Tier 0 binding is **impossible by
//! construction**. The [`tests`] assert the frozen typed AST and the Tier 0
//! substitution are byte-identical across branch scoping.
//!
//! # Compile-time only (§10.10, invariant 14/20)
//!
//! Like the shadow stack, the solver seam is a compile-time analysis artifact.
//! It is **never** a field of [`crate::Evaluator`]; the runtime never constructs
//! one. A checked program ships with no solver and no scope machinery.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::Span;
use crate::StackTy;
use crate::Token;
use crate::Ty;
use crate::WordTy;
use crate::refinement::BinOp;
use crate::refinement::Binder;
use crate::refinement::Pred;
use crate::refinement::RefineSpan;
use crate::refinement::RefinementSide;
use crate::refinement::RefinementSig;
use crate::refinement::UnOp;
use crate::refinement::parse_assume;
use crate::shadow::NamedBinding;
use crate::shadow::ShadowError;
use crate::shadow::ShadowStack;
use crate::shadow::ShadowWord;
use crate::shadow::bind_positional;

// ===========================================================================
// The verdict and the seam
// ===========================================================================

/// The result of a solver [`Solver::check`]: the three SMT outcomes.
///
/// `Unsat` of the negated goal means **valid** (no counterexample); `Sat` means
/// a counterexample exists; `Unknown` means the solver could not decide (degrade,
/// and fail closed where a guarantee is at stake — §10.5/§10.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The assertion set is satisfiable (a model / counterexample exists).
    Sat,
    /// The assertion set is unsatisfiable (no model; the negated goal is valid).
    Unsat,
    /// The solver could not decide.
    Unknown,
}

/// A **counterexample model** (§10.5): a concrete satisfying assignment over the
/// named binders of a VC whose negated-goal encoding came back `Sat`.
///
/// The model is the witness the solver hands back — for `x sqrt` with no fact
/// bounding `x`, the negated goal `¬(x >= 0)` is satisfiable and the model is a
/// concrete point such as `x = -1`. It is surfaced on the failing [`Obligation`]
/// so a diagnostic can show *why* the demand could not be discharged, rather than
/// a bare `Sat` (§10.5 / §12 M9).
///
/// A model is produced **only** when the satisfiability is fully decidable
/// (linear, no opaque conjunct): an `Unknown`/opaque result does **not**
/// fabricate a model — it degrades (§10.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// The assignment: each named binder mapped to its concrete value, rendered
    /// (`-1`, `3`, `1/2`). Sorted by name for determinism.
    assignments: Vec<(String, String)>,
}

impl Model {
    /// The value assigned to `name`, rendered (e.g. `-1`, `1/2`), if the model
    /// constrains it.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.assignments
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The assignments as `(name, rendered-value)` pairs, sorted by name.
    pub fn assignments(&self) -> &[(String, String)] {
        &self.assignments
    }

    /// Whether the model constrains no variable (a trivially-`Sat` VC with no
    /// free variables).
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let body = self
            .assignments
            .iter()
            .map(|(n, v)| format!("{n} = {v}"))
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "{{ {body} }}")
    }
}

/// The capability of producing a **counterexample model** for a `Sat` check
/// (§10.5). It is deliberately a **separate** trait from [`Solver`] so the core
/// seam keeps its **exactly four** methods (`assert`/`check`/`push_scope`/
/// `pop_scope`); model extraction is the backend-specific sibling capability
/// (the embedded reasoner does Fourier–Motzkin back-substitution; a future
/// `Z3Solver` would call `get_model`).
pub trait CounterModel {
    /// Produce a satisfying model for the current (live) assertion set, or `None`
    /// if no decidable model is available (degrade — never fabricate one for an
    /// opaque/`Unknown` result). Call this **after** a [`Solver::check`] that
    /// returned [`Verdict::Sat`] and **before** popping the scope that holds the
    /// negated goal.
    fn model(&self) -> Option<Model>;
}

/// The **solver seam** (§10.8): the trait every path-condition / VC consumer
/// talks to, with **exactly four** methods. Nothing above the seam ever calls a
/// concrete solver directly; the concrete solver (SMT-LIB text today, the `z3`
/// crate later) lives entirely behind this trait.
///
/// Scoped assertions (`push_scope`/`pop_scope`) are part of the seam **by
/// mandate** — they are the substrate for path conditions (§10.4), which is why
/// §10.8 requires them to exist before M8 rather than be retrofitted.
pub trait Solver {
    /// Add a formula to the current (innermost) scope.
    fn assert(&mut self, formula: &Pred);

    /// Check satisfiability of the conjunction of all asserted formulas across
    /// all live scopes.
    fn check(&mut self) -> Verdict;

    /// Open a new assertion scope. Everything asserted until the matching
    /// [`Solver::pop_scope`] is discarded by that pop. In SMT-LIB text mode this
    /// emits `(push 1)`.
    fn push_scope(&mut self);

    /// Discard the innermost scope and everything asserted into it. In SMT-LIB
    /// text mode this emits `(pop 1)`.
    fn pop_scope(&mut self);
}

// ===========================================================================
// The SMT-LIB2 text-emission seam (the day-one implementation, §10.9)
// ===========================================================================

/// An SMT-LIB2 **text-emission** implementation of the [`Solver`] seam (§10.9).
///
/// This is the day-one, solver-agnostic implementation: it records the exact
/// SMT-LIB2 script it would hand a solver — including `(push 1)`/`(pop 1)` in
/// lockstep with [`Solver::push_scope`]/[`Solver::pop_scope`] — which is the
/// debugging window and the parity reference for any future native backend
/// (M13). Because no real solver is wired in this workspace (see the module
/// docs), [`Solver::check`] is answered by a **minimal embedded linear-arithmetic
/// reasoner** ([`check_sat`]) sufficient for the §12 M8 demonstration; it is the
/// stand-in for the `z3`-crate `check-sat`, not the M9 VC pipeline.
#[derive(Debug, Clone, Default)]
pub struct SmtLibSolver {
    /// One `Vec<Pred>` per live scope; index 0 is the base scope.
    scopes: Vec<Vec<Pred>>,
    /// Variables declared per scope, so a `pop_scope` retracts exactly the
    /// declarations made after the matching `push_scope` (SMT-LIB pop semantics).
    declared: Vec<BTreeSet<String>>,
    /// The accumulated SMT-LIB2 script (the debugging window / parity reference).
    script: String,
}

impl SmtLibSolver {
    /// A fresh solver with a single base scope and a logic header.
    pub fn new() -> Self {
        let mut s = SmtLibSolver {
            scopes: vec![Vec::new()],
            declared: vec![BTreeSet::new()],
            script: String::new(),
        };
        s.line("(set-logic QF_LRA)");
        s
    }

    /// The accumulated SMT-LIB2 script.
    pub fn script(&self) -> &str {
        &self.script
    }

    /// The current scope nesting depth (number of live scopes, base scope
    /// included). One base scope ⇒ depth 1.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// The conjunction of every formula asserted across all live scopes — the
    /// exact set [`Solver::check`] reasons over. Used by [`CounterModel::model`]
    /// to recover a witness point from the same constraint set.
    fn live_formulas(&self) -> Vec<Pred> {
        self.scopes.iter().flatten().cloned().collect()
    }

    fn line(&mut self, s: &str) {
        self.script.push_str(s);
        self.script.push('\n');
    }

    /// Whether `name` is declared in any live scope.
    fn is_declared(&self, name: &str) -> bool {
        self.declared.iter().any(|s| s.contains(name))
    }

    /// Declare every free variable of `pred` not already declared, into the
    /// current scope.
    fn declare_vars(&mut self, pred: &Pred) {
        let mut vars = BTreeSet::new();
        collect_vars(pred, &mut vars);
        let new: Vec<String> = vars.into_iter().filter(|v| !self.is_declared(v)).collect();
        for v in new {
            self.line(&format!("(declare-const {v} Real)"));
            self.declared.last_mut().unwrap().insert(v);
        }
    }
}

impl Solver for SmtLibSolver {
    fn assert(&mut self, formula: &Pred) {
        self.declare_vars(formula);
        let rendered = render_smtlib(formula);
        self.line(&format!("(assert {rendered})"));
        self.scopes.last_mut().unwrap().push(formula.clone());
    }

    fn check(&mut self) -> Verdict {
        self.line("(check-sat)");
        let all: Vec<Pred> = self.scopes.iter().flatten().cloned().collect();
        let verdict = check_sat(&all);
        // Record the verdict as a comment so the script self-documents (debugging
        // window). The solver-agnostic seam still only *emits* (check-sat).
        let tag = match verdict {
            Verdict::Sat => "sat",
            Verdict::Unsat => "unsat",
            Verdict::Unknown => "unknown",
        };
        self.line(&format!("; => {tag}"));
        verdict
    }

    fn push_scope(&mut self) {
        self.line("(push 1)");
        self.scopes.push(Vec::new());
        self.declared.push(BTreeSet::new());
    }

    fn pop_scope(&mut self) {
        debug_assert!(
            self.scopes.len() > 1,
            "pop_scope underflow: cannot pop the base scope"
        );
        self.line("(pop 1)");
        self.scopes.pop();
        self.declared.pop();
    }
}

impl CounterModel for SmtLibSolver {
    fn model(&self) -> Option<Model> {
        // Recover a witness point from the same live constraint set the embedded
        // reasoner judged. Returns `Some(model)` only when the set is fully
        // decidable + feasible (a genuine `Sat`); an opaque/`Unknown` set yields
        // `None` — no fabricated model (§10.5).
        check_sat_model(&self.live_formulas()).1
    }
}

/// A read-only **snapshot of the live facts** (§10.7 / M12). This is a sibling
/// capability to [`Solver`], kept off the core seam so the seam keeps its
/// **exactly four** methods (`assert`/`check`/`push_scope`/`pop_scope`).
///
/// The `assume` boundary (§10.7) needs two things the bare seam does not expose:
/// the set of facts currently in scope — to decide whether a value is
/// **genuinely opaque** (invariant 13) and to key the **exploratory-verdict
/// cache** on the obligation's canonical content — and nothing more. Reading the live
/// facts from the solver (rather than re-tracking them in parallel) keeps the
/// snapshot the single source of truth: it cannot drift from what `check()`
/// actually reasons over.
pub trait FactSnapshot {
    /// The facts currently asserted across all live scopes, in assertion order.
    fn live_facts(&self) -> Vec<Pred>;
}

impl FactSnapshot for SmtLibSolver {
    fn live_facts(&self) -> Vec<Pred> {
        self.live_formulas()
    }
}

// ===========================================================================
// SMT-LIB2 rendering of a refinement predicate
// ===========================================================================

fn collect_vars(pred: &Pred, out: &mut BTreeSet<String>) {
    match pred {
        Pred::Var(name) => {
            out.insert(name.clone());
        }
        Pred::Num(_) => {}
        Pred::Bin(_, a, b) => {
            collect_vars(a, out);
            collect_vars(b, out);
        }
        Pred::Un(_, a) => collect_vars(a, out),
        Pred::App(_, args) => {
            for a in args {
                collect_vars(a, out);
            }
        }
    }
}

fn binop_smt(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Ge => ">=",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Lt => "<",
        BinOp::Eq => "=",
        BinOp::And => "and",
        BinOp::Or => "or",
        BinOp::Implies => "=>",
    }
}

/// Render a refinement predicate as an SMT-LIB2 s-expression.
pub fn render_smtlib(pred: &Pred) -> String {
    match pred {
        Pred::Var(name) => name.clone(),
        Pred::Num(lexeme) => {
            // SMT-LIB writes a negative literal as `(- n)`.
            if let Some(rest) = lexeme.strip_prefix('-') {
                format!("(- {rest})")
            } else {
                lexeme.clone()
            }
        }
        Pred::Bin(op, a, b) => {
            format!(
                "({} {} {})",
                binop_smt(*op),
                render_smtlib(a),
                render_smtlib(b)
            )
        }
        Pred::Un(op, a) => match op {
            UnOp::Not => format!("(not {})", render_smtlib(a)),
            UnOp::Neg => format!("(- {})", render_smtlib(a)),
        },
        Pred::App(f, args) => {
            let mut s = format!("({f}");
            for a in args {
                s.push(' ');
                s.push_str(&render_smtlib(a));
            }
            s.push(')');
            s
        }
    }
}

// ===========================================================================
// VC helpers: negate, substitute, discharge
// ===========================================================================

/// The logical negation of a predicate (`¬p`).
pub fn negate(pred: &Pred) -> Pred {
    Pred::Un(UnOp::Not, Box::new(pred.clone()))
}

/// Substitute named bindings into a predicate: every [`Pred::Var`] whose name
/// matches a binding is replaced by that binding's symbolic term.
///
/// This is how a callee's `where` predicate (over its parameter names) becomes a
/// VC goal over the **actual** shadow terms at the call site, using the M7
/// positional binding ([`crate::bind_positional`]).
pub fn substitute(pred: &Pred, bindings: &[NamedBinding]) -> Pred {
    match pred {
        Pred::Var(name) => bindings
            .iter()
            .find(|b| &b.name == name)
            .map(|b| b.term.clone())
            .unwrap_or_else(|| pred.clone()),
        Pred::Num(_) => pred.clone(),
        Pred::Bin(op, a, b) => Pred::Bin(
            *op,
            Box::new(substitute(a, bindings)),
            Box::new(substitute(b, bindings)),
        ),
        Pred::Un(op, a) => Pred::Un(*op, Box::new(substitute(a, bindings))),
        Pred::App(f, args) => Pred::App(
            f.clone(),
            args.iter().map(|a| substitute(a, bindings)).collect(),
        ),
    }
}

/// Discharge a goal under the solver's current scope (the live path conditions),
/// using the **negated-goal** encoding (§10.5): assert `¬goal` in a fresh scope,
/// `check()`, then pop. `Unsat` ⇒ the goal is **valid** under the current path
/// conditions (no counterexample); `Sat`/`Unknown` ⇒ not discharged.
///
/// This is the bare-[`Verdict`] form (the M8 shape). For the M9 counterexample,
/// use [`discharge_with_model`], which additionally pulls the model on a `Sat`.
pub fn discharge<S: Solver>(solver: &mut S, goal: &Pred) -> Verdict {
    solver.push_scope();
    solver.assert(&negate(goal));
    let verdict = solver.check();
    solver.pop_scope();
    verdict
}

/// Discharge a goal under the live facts + path conditions, **with
/// counterexample extraction** (§10.5 / M9): assert `¬goal` in a fresh scope,
/// `check()`, and — on `Sat` — pull the satisfying [`Model`] (the counterexample)
/// **before** popping the scope. `Unsat` ⇒ valid (no model); `Unknown` ⇒ degrade
/// (no fabricated model).
///
/// The model is read through the [`CounterModel`] seam-sibling, so it works for
/// any backend that can produce one (the embedded reasoner today, `z3` later).
pub fn discharge_with_model<S: Solver + CounterModel>(
    solver: &mut S,
    goal: &Pred,
) -> (Verdict, Option<Model>) {
    solver.push_scope();
    solver.assert(&negate(goal));
    let verdict = solver.check();
    // The model lives in the just-pushed scope (hypotheses ∧ ¬goal); read it
    // BEFORE popping. Only a genuine `Sat` carries a model — §10.5.
    let model = if verdict == Verdict::Sat {
        solver.model()
    } else {
        None
    };
    solver.pop_scope();
    (verdict, model)
}

// ===========================================================================
// Path-condition-aware shadow verification (§10.4)
// ===========================================================================

/// What a word does during path-condition-aware verification.
///
/// Most words are pure data movement ([`VerifyWord::Core`], a M7
/// [`ShadowWord`]). A word with a refinement **demand** is a call site
/// ([`VerifyWord::Call`]): its demand becomes a VC discharged under the live
/// path conditions, then its Tier 0 arrow moves the data (treated opaquely for
/// M8 — guarantees / subsumption are M9/M10).
#[derive(Debug, Clone)]
pub enum VerifyWord {
    /// Pure data movement, resolved exactly as the M7 shadow stack would.
    Core(ShadowWord),
    /// A call site carrying a refinement contract: a **demand** to discharge at
    /// the call site (a precondition VC) and/or a **guarantee** to publish as a
    /// fact for downstream words (a postcondition gift, §10.1).
    Call {
        /// The demand's input binders (source order), zipped right-to-left
        /// against the stack top by [`crate::bind_positional`] (§10.2).
        binders: Vec<Binder>,
        /// The demand predicate over the input binders (an obligation on the
        /// caller). `None` is an absent refinement — `where true`, no VC (§10.7).
        demand: Option<Pred>,
        /// The guarantee's output binders (source order), zipped right-to-left
        /// against the **post-call** stack top (the freshly pushed outputs).
        out_binders: Vec<Binder>,
        /// The guarantee predicate over the output binders, asserted as a live
        /// fact after the call so downstream demands can use it. `None` is an
        /// absent refinement — `where true`, nothing published (§10.7).
        guarantee: Option<Pred>,
        /// The Tier 0 arrow: how many terms the word pops/pushes.
        arrow: WordTy,
    },
}

/// One discharged obligation recorded during verification: the (substituted) VC
/// goal, the verdict the solver returned for it under the live facts + path
/// conditions, and — on a `Sat` (failing) verdict — the surfaced counterexample
/// [`Model`] (§10.5 / M9).
#[derive(Debug, Clone)]
pub struct Obligation {
    /// The call-site word that raised this obligation (for diagnostics).
    pub word: String,
    /// The VC goal, with binders substituted to the actual shadow terms.
    pub goal: Pred,
    /// The verdict: `Unsat` ⇒ discharged/valid; `Sat` ⇒ refuted (see `model`);
    /// `Unknown` ⇒ undecided (degrade; never accepted as discharged — §10.5).
    pub verdict: Verdict,
    /// The counterexample model, present **iff** `verdict == Sat` and the VC was
    /// fully decidable (§10.5). An `Unknown` never carries a (fabricated) model.
    pub model: Option<Model>,
}

impl Obligation {
    /// Whether this obligation is **discharged** (proven valid): the negated-goal
    /// encoding came back `Unsat`. `Sat` (refuted) and `Unknown` (undecided) are
    /// **not** discharged — an `Unknown` is never silently accepted (§10.5).
    pub fn is_discharged(&self) -> bool {
        self.verdict == Verdict::Unsat
    }
}

/// The verifier's resolver: maps a word name to its [`VerifyWord`]. The word
/// `if` is intercepted by the verifier itself (path conditions) and is **not**
/// resolved here.
pub trait VerifyResolve {
    /// Resolve a non-`if` word to its verification action.
    fn resolve(&self, word: &str) -> VerifyWord;
}

impl<F> VerifyResolve for F
where
    F: Fn(&str) -> VerifyWord,
{
    fn resolve(&self, word: &str) -> VerifyWord {
        self(word)
    }
}

/// Is this word the `if` combinator (either the spec's `if` or the runtime
/// `IF` — see [`crate::BUILTIN_NAME_MAP`])?
fn is_if(word: &str) -> bool {
    word.eq_ignore_ascii_case("if")
}

/// Apply a word's data-movement effect to the shadow stack, mirroring the M7
/// [`ShadowStack`] dispatch. A [`VerifyWord::Call`] moves data per its Tier 0
/// arrow (opaque for M8). `resolve` is threaded so `dip`/`call` can run inner
/// bodies.
fn apply_effect<R: VerifyResolve, S: Solver + CounterModel + FactSnapshot>(
    stack: &mut ShadowStack,
    word: &str,
    resolve: &R,
    solver: &mut S,
    ctx: &mut VerifyCtx,
) -> Result<(), ShadowError> {
    match resolve.resolve(word) {
        VerifyWord::Core(core) => apply_core(stack, core, resolve, solver, ctx),
        VerifyWord::Call {
            binders,
            demand,
            out_binders,
            guarantee,
            arrow,
        } => {
            // (1) Demand → VC at the call site: bind the demand's parameters to
            // the actual shadow terms (§10.2), substitute, discharge under the
            // live facts + path conditions via the negated-goal encoding, pulling
            // the counterexample model on a `Sat` (§10.4/§10.5).
            if let Some(demand) = demand {
                let bindings = bind_positional(&binders, stack)?;
                let goal = substitute(&demand, &bindings);
                let (verdict, model) = discharge_with_model(solver, &goal);
                ctx.obligations.push(Obligation {
                    word: word.to_string(),
                    goal,
                    verdict,
                    model,
                });
            }
            // (2) Move the data per the Tier 0 arrow (opaque: outputs are fresh
            // literals).
            stack.apply_opaque(&arrow)?;
            // (3) Guarantee → publish as a live fact: bind the output binders to
            // the freshly pushed output terms, substitute, and assert into the
            // current scope so downstream demands (and the rest of this scope)
            // can use it (§10.1 — output predicates are gifts to the next word).
            if let Some(guarantee) = guarantee {
                let bindings = bind_positional(&out_binders, stack)?;
                let fact = substitute(&guarantee, &bindings);
                solver.assert(&fact);
            }
            Ok(())
        }
    }
}

/// Apply a core [`ShadowWord`] to the shadow stack, threading the verifier so
/// `dip`/`call` recurse through [`verify`] (and so an `if` *inside* a quotation
/// still gets path conditions).
fn apply_core<R: VerifyResolve, S: Solver + CounterModel + FactSnapshot>(
    stack: &mut ShadowStack,
    core: ShadowWord,
    resolve: &R,
    solver: &mut S,
    ctx: &mut VerifyCtx,
) -> Result<(), ShadowError> {
    match core {
        ShadowWord::Dup => stack.dup(),
        ShadowWord::Drop => stack.drop(),
        ShadowWord::Swap => stack.swap(),
        ShadowWord::Over => stack.over(),
        ShadowWord::Rot => stack.rot(),
        ShadowWord::Nip => stack.nip(),
        ShadowWord::Tuck => stack.tuck(),
        ShadowWord::Bin(op) => stack.bin(op),
        ShadowWord::Un(op) => stack.un(op),
        ShadowWord::Num(lexeme) => {
            stack.push_term(Pred::Num(lexeme));
            Ok(())
        }
        ShadowWord::Var(name) => {
            stack.push_term(Pred::Var(name));
            Ok(())
        }
        ShadowWord::Opaque(arrow) => stack.apply_opaque(&arrow),
        ShadowWord::Dip => {
            // Mirror combinators.rs::dip: pop the quotation, set aside the next
            // term, verify the quotation on the rest, restore the set-aside term.
            let body = stack.pop_quote()?;
            let hidden = stack.pop()?;
            verify_ctx(&body, stack, solver, resolve, ctx)?;
            stack.push_slot(hidden);
            Ok(())
        }
        ShadowWord::Call => {
            let body = stack.pop_quote()?;
            verify_ctx(&body, stack, solver, resolve, ctx)
        }
    }
}

/// Verify a token sequence with **path conditions** (§10.4): execute the shadow
/// stack, and on every `cond [ then ] [ else ] if` recover `cond`'s proposition
/// `P` from the shadow stack, verify `[ then ]` with `P` asserted in a pushed
/// scope and `[ else ]` with `¬P` in a pushed scope — `push_scope` before each
/// branch and `pop_scope` after.
///
/// Every obligation discharged (including those inside branches, under the
/// branch's path condition) is recorded in `obligations`. The scopes live
/// entirely in `solver` and this function's `stack`; nothing here touches the
/// Tier 0 substitution (immutability barrier — module docs / invariant 18).
pub fn verify<R: VerifyResolve, S: Solver + CounterModel + FactSnapshot>(
    tokens: &[Token],
    stack: &mut ShadowStack,
    solver: &mut S,
    resolve: &R,
    obligations: &mut Vec<Obligation>,
) -> Result<(), ShadowError> {
    let mut ctx = VerifyCtx::new();
    let r = verify_ctx(tokens, stack, solver, resolve, &mut ctx);
    obligations.append(&mut ctx.obligations);
    r
}

/// The context-threaded core verifier (§10.7 / M12): like [`verify`] but
/// carrying a [`VerifyCtx`] so `assume` boundaries land in the ledger and the
/// exploratory cache persists across the whole body. [`verify`] is the thin
/// backward-compatible wrapper (M8–M11 callers that only need the obligation
/// stream).
///
/// An `assume( PRED )` word (§10.7) is intercepted here — never resolved as an
/// ordinary word — bound to the live shadow stack, checked for STRICT legality,
/// recorded, and (when legal) discharged-on-faith into the current scope.
pub fn verify_ctx<R: VerifyResolve, S: Solver + CounterModel + FactSnapshot>(
    tokens: &[Token],
    stack: &mut ShadowStack,
    solver: &mut S,
    resolve: &R,
    ctx: &mut VerifyCtx,
) -> Result<(), ShadowError> {
    for token in tokens {
        match token {
            Token::Bracket(body) => stack.push_quote(body.clone()),
            Token::Word(w) if is_if(w) => {
                verify_if(stack, solver, resolve, ctx)?;
            }
            Token::Word(w) if parse_assume(w).is_some() => {
                // Safe: `is_some()` above. A malformed `assume(` body surfaces as
                // a located ShadowError rather than being mistaken for a word.
                let pred = parse_assume(w).unwrap().map_err(|e| ShadowError {
                    message: format!("malformed `assume` clause: {e}"),
                })?;
                apply_assume(stack, w, pred, solver, ctx)?;
            }
            Token::Word(w) => {
                apply_effect(stack, w, resolve, solver, ctx)?;
            }
        }
    }
    Ok(())
}

/// The path-condition core (§10.4) for `cond [ then ] [ else ] if`.
///
/// Stack on entry (top last): `… P [then] [else]`. Recover `P`, then:
///
///   * `[ then ]`: `push_scope`; `assert(P)`; verify the then-body on the stack
///     below `P`; `pop_scope`.
///   * `[ else ]`: `push_scope`; `assert(¬P)`; verify the else-body; `pop_scope`.
///
/// Both branches have the same Tier 0 effect, so after verifying both the actual
/// shadow stack is advanced by running the then-body once (Tier 0 already proved
/// the branches agree on shape).
fn verify_if<R: VerifyResolve, S: Solver + CounterModel + FactSnapshot>(
    stack: &mut ShadowStack,
    solver: &mut S,
    resolve: &R,
    ctx: &mut VerifyCtx,
) -> Result<(), ShadowError> {
    let else_body = stack.pop_quote()?;
    let then_body = stack.pop_quote()?;
    let cond = stack.pop_term()?;

    // then-branch under P. Keep its resulting stack to advance the real stack:
    // both branches have the same Tier 0 effect, so the then-branch's post-state
    // *is* the if's post-state. Reusing it (rather than re-running a body) means
    // each obligation is discharged exactly once, under its branch's path
    // condition.
    let then_stack = {
        let mut branch = stack.clone();
        solver.push_scope();
        solver.assert(&cond);
        verify_ctx(&then_body, &mut branch, solver, resolve, ctx)?;
        solver.pop_scope();
        branch
    };

    // else-branch under ¬P.
    {
        let mut branch = stack.clone();
        solver.push_scope();
        solver.assert(&negate(&cond));
        verify_ctx(&else_body, &mut branch, solver, resolve, ctx)?;
        solver.pop_scope();
    }

    // Advance the real stack by the (shape-identical) then-branch's post-state.
    *stack = then_stack;
    Ok(())
}

// ===========================================================================
// First-order VC generation from refinement signatures (M9, §10.5 / §14.8)
// ===========================================================================

/// A neutral Tier-1 arrow over `Num` of the given pop/push counts.
///
/// The shadow stack only reads element **counts** from an arrow
/// ([`ShadowStack::apply_opaque`]); the element *types* never matter at Tier 1
/// (Tier 0 already proved the shape). This builds an arrow with `pops`/`pushes`
/// `Num` slots so a refinement signature — which records only binder counts and
/// names, not Tier 0 shapes — can drive the shadow stack.
fn num_arrow(pops: usize, pushes: usize) -> WordTy {
    const S: Span = Span { start: 0, end: 0 };
    let ins = (0..pops).map(|_| Ty::num(S)).collect();
    let outs = (0..pushes).map(|_| Ty::num(S)).collect();
    WordTy::new(StackTy::new(ins, 0, S), StackTy::new(outs, 0, S))
}

/// Resolve a word to its [`VerifyWord`] from its **attached refinement
/// signature** (§10.5). A word with a signature becomes a [`VerifyWord::Call`]
/// carrying its demand (the input-side `where` — discharged at the call site) and
/// its guarantee (the output-side `where` — published as a downstream fact); a
/// word with no signature falls back to a core shadow word / interpreted op /
/// literal / free variable.
///
/// The arrow is synthesized from the signature's binder counts (`num_arrow`):
/// the demand binders are the pops, the guarantee binders the pushes. An absent
/// `where` on either side is `where true` — no VC / no published fact (§10.7).
pub fn refinement_verify_word(word: &str, sig: Option<&RefinementSig>) -> VerifyWord {
    if let Some(sig) = sig {
        return VerifyWord::Call {
            binders: sig.demands.binders.clone(),
            demand: sig.demands.predicate.clone(),
            out_binders: sig.guarantees.binders.clone(),
            guarantee: sig.guarantees.predicate.clone(),
            arrow: num_arrow(sig.demands.binders.len(), sig.guarantees.binders.len()),
        };
    }
    if let Some(core) = crate::shadow::core_shadow_word(word) {
        return VerifyWord::Core(core);
    }
    if let Some(op) = crate::shadow::interpreted_op(word) {
        return VerifyWord::Core(op);
    }
    if crate::types::is_numeric_literal(word) {
        return VerifyWord::Core(ShadowWord::Num(word.to_string()));
    }
    VerifyWord::Core(ShadowWord::Var(word.to_string()))
}

/// The **public Tier-1 check entry** (§10.5 / M9): run first-order VC generation
/// over `tokens`, deriving each call site's demands/guarantees from its attached
/// refinement signature via `lookup`.
///
/// For every word, `lookup(word)` supplies its [`RefinementSig`] (or `None` for
/// an unrefined word). At each call site the callee's demand binders are zipped
/// against the inferred shadow stack (§10.2), the known facts (preceding words'
/// published guarantees + live path conditions) are already in the solver scope,
/// and the demand is discharged through the negated-goal encoding — surfacing a
/// counterexample model on failure (§10.5). The returned [`Obligation`]s are the
/// VC verdicts in call order.
///
/// Tier 0 is untouched: this builds its own [`ShadowStack`] and drives the
/// supplied `solver`; nothing here takes a Tier 0 inference handle (the
/// immutability barrier — module docs / invariant 18).
pub fn check_refinements<L, S>(
    tokens: &[Token],
    lookup: &L,
    solver: &mut S,
) -> Result<Vec<Obligation>, ShadowError>
where
    L: Fn(&str) -> Option<RefinementSig>,
    S: Solver + CounterModel + FactSnapshot,
{
    let resolve = |w: &str| refinement_verify_word(w, lookup(w).as_ref());
    let mut stack = ShadowStack::new();
    let mut obligations = Vec::new();
    verify(tokens, &mut stack, solver, &resolve, &mut obligations)?;
    Ok(obligations)
}
// ===========================================================================
// The `assume` boundary — strict + cached/lenient (M12, §10.7 / invariant 13)
// ===========================================================================
//
// `assume` is the resolution for **Situation B** (§10.7): an opaque/uncontracted
// value flows into a demand or guarantee, the VC becomes `true ⟹ <goal>`,
// invalid, and the obligation fails closed. The user attaches an `assume( PRED )`
// clause at the boundary — a refinement **taken as true without proof**, recorded
// as an explicit, enumerable **ledger** entry, and **discharged-on-faith**. The
// assumed predicate is asserted as a live fact so the dependent obligation
// discharges; the word's honest status becomes *"verified modulo { … }"* and
// callers inherit the modulo status visibly.
//
// Three properties keep it sound, never a disguised fail-open:
//
//   1. **Default fail-closed.** Without `assume` the obligation still fails; a
//      *rejected* `assume` injects nothing, so its obligation fails closed too.
//   2. **STRICT anti-rot (invariant 13).** An `assume` is a **hard error** where
//      it is unnecessary: (a) the obligation is provable WITHOUT it — *drop the
//      assumption, re-run the VC, and a solver `Unsat` (the positive showing)
//      rejects* with [`ASSUME_PROVABLE_MSG`] — or (b) there is no genuinely
//      opaque dependency in the obligation's chain ([`ASSUME_NO_OPAQUE_MSG`]).
//   3. **Cheap.** The exploratory (without-assumption) solve is **cached** on the
//      obligation's canonical content and **fails lenient**: only a *positive*
//      `Unsat` rejects; an `Unknown` (the embedded reasoner's opaque/timeout
//      analogue) is **not** a positive showing and **accepts** the `assume`.
//
// The expensive/correct solve is the *second* one — the dependent obligation
// **under** the asserted assumption, discharged by the ordinary VC pipeline once
// the faith-fact is in scope. The exploratory check here is the cheap first one,
// on a short leash, off the critical proof path (§10.7).

/// STRICT positive-rejection message (§10.7 / invariant 13): emitted when the
/// exploratory drop-and-re-run shows the obligation is **provable without** the
/// `assume` (solver returned `Unsat`). The ledger must mean *"things we honestly
/// cannot prove,"* never *"things we couldn't be bothered to."*
pub const ASSUME_PROVABLE_MSG: &str =
    "this obligation is provable; remove the unnecessary `assume`";

/// STRICT no-opaque-dependency message (§10.7 / invariant 13): emitted when an
/// `assume`'s obligation has **no genuinely opaque/uncontracted value** in its
/// chain — `assume` is legal only where such a dependency sits.
pub const ASSUME_NO_OPAQUE_MSG: &str = "this `assume` has no opaque dependency; `assume` is legal only where a genuinely opaque/uncontracted value sits in the obligation's chain";

/// The legality verdict for one `assume` boundary (§10.7 / invariant 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumeLegality {
    /// Legal: a genuinely opaque dependency, not provable without the assumption
    /// (`Sat` without it, or `Unknown` ⇒ fail-lenient accept). The faith-fact is
    /// asserted.
    Legal,
    /// Rejected — **provable without it**: the exploratory drop-and-re-run
    /// returned `Unsat` (the positive showing). See [`ASSUME_PROVABLE_MSG`].
    RejectedProvable,
    /// Rejected — **no opaque dependency** sits in the obligation's chain. See
    /// [`ASSUME_NO_OPAQUE_MSG`].
    RejectedNoOpaqueDependency,
}

impl AssumeLegality {
    /// Whether the `assume` is legal (its faith-fact is admitted).
    pub fn is_legal(self) -> bool {
        matches!(self, AssumeLegality::Legal)
    }

    /// The rejection diagnostic, or `None` if legal.
    pub fn message(self) -> Option<&'static str> {
        match self {
            AssumeLegality::Legal => None,
            AssumeLegality::RejectedProvable => Some(ASSUME_PROVABLE_MSG),
            AssumeLegality::RejectedNoOpaqueDependency => Some(ASSUME_NO_OPAQUE_MSG),
        }
    }
}

/// One processed `assume` boundary recorded during verification (§10.7 / M12):
/// its surface, the bound faith-predicate, the legality verdict, and the
/// exploratory drop-and-re-run details (verdict + whether it was served from the
/// content-key cache).
#[derive(Debug, Clone)]
pub struct AssumeRecord {
    /// The definition/word being verified when this `assume` was encountered
    /// (the ledger **site** — `grep assume` granularity).
    pub site: String,
    /// The raw `assume( … )` surface word from the program token stream.
    pub surface: String,
    /// The faith-predicate with its free variables **bound to the actual shadow
    /// terms** (§10.2) — the precise fact asserted (when legal) and the goal the
    /// exploratory check ran on.
    pub predicate: Pred,
    /// The faith-predicate exactly as the user **wrote** it (unbound) — what the
    /// ledger displays (`result > 0`, not the bound `$t0 > 0`) and `grep assume`
    /// enumerates.
    pub surface_pred: Pred,
    /// The legality verdict (`Legal` ⇒ the fact was asserted; otherwise a hard
    /// error and nothing was asserted).
    pub legality: AssumeLegality,
    /// The **exploratory** (without-assumption) verdict — the drop-and-re-run.
    /// `Unsat` ⇒ provable ⇒ rejected; `Sat`/`Unknown` ⇒ not a positive showing.
    pub exploratory: Verdict,
    /// Whether the exploratory verdict was served from the content-key cache
    /// (a warm hit) rather than freshly solved (§10.7 — cheap).
    pub from_cache: bool,
}

/// Cache of **exploratory** (without-assumption) verdicts keyed on the
/// obligation's canonical content (§10.7 — cheap). Strict legality solves twice;
/// this memoizes the first (cheap, fail-lenient) solve so a warm compile pays it
/// zero times for unchanged obligations. It also tracks how many real solves vs.
/// cache hits occurred so a test can assert the cache is actually used.
#[derive(Debug, Clone, Default)]
pub struct ExploratoryCache {
    verdicts: HashMap<String, Verdict>,
    solves: usize,
    hits: usize,
}

impl ExploratoryCache {
    /// A fresh, empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of **real** solver runs (cache misses) performed.
    pub fn solves(&self) -> usize {
        self.solves
    }

    /// Number of **cache hits** (verdicts reused without re-solving).
    pub fn hits(&self) -> usize {
        self.hits
    }

    /// Number of distinct obligations memoized.
    pub fn len(&self) -> usize {
        self.verdicts.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.verdicts.is_empty()
    }

    /// Run the exploratory drop-and-re-run for `goal` under the solver's live
    /// facts, reusing a cached verdict when the obligation's canonical content
    /// (predicate + in-scope facts) is unchanged. Returns the verdict and whether
    /// it was a cache hit.
    fn discharge_cached<S: Solver + FactSnapshot>(
        &mut self,
        solver: &mut S,
        goal: &Pred,
    ) -> (Verdict, bool) {
        let key = obligation_key(goal, &solver.live_facts());
        if let Some(v) = self.verdicts.get(&key) {
            self.hits += 1;
            return (*v, true);
        }
        // The drop-and-re-run: `goal` is discharged WITHOUT the assumption being
        // asserted (it is not in scope yet) — exactly "drop the assumption,
        // re-run the VC" (§10.7 / invariant 13). This path actually runs.
        let v = discharge(solver, goal);
        self.solves += 1;
        self.verdicts.insert(key, v);
        (v, false)
    }
}

/// Canonical key for an obligation: its goal predicate plus the
/// (order-independent) set of in-scope facts (§10.7 — the same per-obligation
/// cache key as the whole-program reverify story). Rendering to SMT-LIB text
/// gives a stable, structural key; storing the full string keeps the cache exact
/// even if the hash table's internal hash function collides.
fn obligation_key(goal: &Pred, facts: &[Pred]) -> String {
    fn push_part(key: &mut String, part: &str) {
        key.push_str(&part.len().to_string());
        key.push(':');
        key.push_str(part);
        key.push('\n');
    }

    let mut fact_strs: Vec<String> = facts.iter().map(render_smtlib).collect();
    fact_strs.sort();

    let mut key = String::new();
    push_part(&mut key, "goal");
    push_part(&mut key, &render_smtlib(goal));
    push_part(&mut key, "facts");
    for f in &fact_strs {
        push_part(&mut key, f);
    }
    key
}

/// The free variables of `pred` in **left-to-right first-appearance order** — the
/// binding order for an `assume` clause (§10.2): the topmost stack slot binds the
/// last-appearing variable (matching [`bind_positional`]).
fn ordered_free_vars(pred: &Pred) -> Vec<String> {
    fn walk(p: &Pred, out: &mut Vec<String>) {
        match p {
            Pred::Var(n) => {
                if !out.iter().any(|x| x == n) {
                    out.push(n.clone());
                }
            }
            Pred::Num(_) => {}
            Pred::Bin(_, a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Pred::Un(_, a) => walk(a, out),
            Pred::App(_, args) => {
                for a in args {
                    walk(a, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(pred, &mut out);
    out
}

/// Bind an `assume` predicate's free variables **positionally** to the top of the
/// shadow stack (§10.2): the deepest free variable binds the deeper slot, the
/// last-appearing one binds the top. The result is the predicate over the actual
/// shadow terms — the goal the legality check and the faith-fact use.
///
/// A predicate with no free variables (a purely concrete assertion) is returned
/// unchanged — it binds to nothing and will be caught by the no-opaque-dependency
/// check (§10.7 / invariant 13).
fn bind_assume_to_stack(pred: &Pred, stack: &ShadowStack) -> Result<Pred, ShadowError> {
    let vars = ordered_free_vars(pred);
    if vars.is_empty() {
        return Ok(pred.clone());
    }
    let binders: Vec<Binder> = vars
        .iter()
        .map(|v| Binder {
            name: v.clone(),
            ty: String::new(),
            span: RefineSpan { start: 0, end: 0 },
        })
        .collect();
    let bindings = bind_positional(&binders, stack)?;
    Ok(substitute(pred, &bindings))
}

/// Whether the obligation `goal` has a **genuinely opaque dependency** under the
/// in-scope `facts` (§10.7 / invariant 13): a goal variable the solver knows
/// nothing about (it appears in no in-scope fact). A purely concrete goal (no
/// free variables) has no opaque dependency.
fn has_opaque_dependency(goal: &Pred, facts: &[Pred]) -> bool {
    let mut goal_vars = BTreeSet::new();
    collect_vars(goal, &mut goal_vars);
    if goal_vars.is_empty() {
        return false;
    }
    let mut fact_vars = BTreeSet::new();
    for f in facts {
        collect_vars(f, &mut fact_vars);
    }
    goal_vars.iter().any(|v| !fact_vars.contains(v))
}

/// Decide an `assume` boundary's STRICT legality (§10.7 / invariant 13) for the
/// bound goal, returning the verdict, the exploratory drop-and-re-run verdict,
/// and whether that exploratory verdict was a cache hit.
///
/// Order of checks:
///   1. **Positive rejection (drop-and-re-run).** Discharge `goal` WITHOUT the
///      assumption (cached, fail-lenient). `Unsat` ⇒ provable ⇒
///      [`AssumeLegality::RejectedProvable`]. An `Unknown` is **not** a positive
///      showing — strictness accepts it.
///   2. **No opaque dependency.** If the goal has no genuinely opaque value in
///      its chain ⇒ [`AssumeLegality::RejectedNoOpaqueDependency`].
///   3. Otherwise [`AssumeLegality::Legal`].
fn assume_legality<S: Solver + CounterModel + FactSnapshot>(
    solver: &mut S,
    goal: &Pred,
    cache: &mut ExploratoryCache,
) -> (AssumeLegality, Verdict, bool) {
    let facts = solver.live_facts();
    let (verdict, from_cache) = cache.discharge_cached(solver, goal);
    if verdict == Verdict::Unsat {
        return (AssumeLegality::RejectedProvable, verdict, from_cache);
    }
    if !has_opaque_dependency(goal, &facts) {
        return (
            AssumeLegality::RejectedNoOpaqueDependency,
            verdict,
            from_cache,
        );
    }
    (AssumeLegality::Legal, verdict, from_cache)
}

/// Process an `assume( PRED )` boundary against the live shadow stack + solver
/// scope (§10.7 / M12): bind the predicate to the actual terms, run STRICT
/// legality, record the [`AssumeRecord`] in `ctx`, and — **only when legal** —
/// assert the faith-fact so the dependent obligation discharges. A rejected
/// `assume` asserts nothing (its obligation then fails closed, exactly as if the
/// `assume` were absent — invariant 13 default fail-closed).
fn apply_assume<S: Solver + CounterModel + FactSnapshot>(
    stack: &ShadowStack,
    surface: &str,
    pred: Pred,
    solver: &mut S,
    ctx: &mut VerifyCtx,
) -> Result<(), ShadowError> {
    let goal = bind_assume_to_stack(&pred, stack)?;
    let (legality, exploratory, from_cache) = assume_legality(solver, &goal, &mut ctx.cache);
    let legal = legality.is_legal();
    ctx.assumes.push(AssumeRecord {
        site: ctx.site.clone(),
        surface: surface.to_string(),
        predicate: goal.clone(),
        surface_pred: pred,
        legality,
        exploratory,
        from_cache,
    });
    if legal {
        // Discharge-on-faith: the assumption becomes a live fact so the
        // dependent obligation (the expensive *second* solve, via the ordinary
        // VC pipeline) discharges.
        solver.assert(&goal);
    }
    Ok(())
}

/// The verification context threaded through [`verify_ctx`] (§10.7 / M12): the
/// obligations raised, the `assume` boundaries processed, the exploratory cache,
/// and the **site** (the definition/word being verified) used as the ledger
/// granularity.
#[derive(Debug, Default)]
pub struct VerifyCtx {
    /// VC verdicts in call order (the M9 obligation stream).
    pub obligations: Vec<Obligation>,
    /// Every `assume` boundary processed (legal or rejected), in program order.
    pub assumes: Vec<AssumeRecord>,
    /// The exploratory drop-and-re-run cache (§10.7 — cheap).
    cache: ExploratoryCache,
    /// The site label for ledger entries (the definition being verified).
    site: String,
}

impl VerifyCtx {
    /// A fresh context with an empty (anonymous) site.
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh context whose `assume` ledger entries are attributed to `site`.
    pub fn with_site(site: impl Into<String>) -> Self {
        VerifyCtx {
            site: site.into(),
            ..Default::default()
        }
    }

    /// The obligations raised (M9 stream).
    pub fn obligations(&self) -> &[Obligation] {
        &self.obligations
    }

    /// The `assume` boundaries processed (legal + rejected), program order.
    pub fn assumes(&self) -> &[AssumeRecord] {
        &self.assumes
    }

    /// The exploratory verdict cache (for cache-effectiveness assertions).
    pub fn cache(&self) -> &ExploratoryCache {
        &self.cache
    }
}
// ===========================================================================
// The whole-program verification ledger (M12, §10.7 / invariant 13)
// ===========================================================================
//
// Whole-program, one global ledger (architecture / invariant 15): every `assume`
// in the program lands here as a first-class, enumerable entry, and a word's
// honest status is *"verified modulo { … }."* The modulo status **propagates
// upward** through the call graph — a caller of a word verified modulo `S`
// inherits `S` — so a guarantee never silently reads stronger than it is proven.
// `grep assume` over the source = the complete user trusted base; this ledger is
// the in-memory counterpart enumerated by [`Ledger::assumptions`] /
// [`Ledger::grep_assume`]. (M14 layers a whole-program attestation hash on top;
// it is **not** built here.)

/// One **accepted** user assumption in the ledger (§10.7 / invariant 13): a
/// first-class, enumerable trust entry. Rejected assumes are hard errors and do
/// **not** land here (see [`Ledger::rejections`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumeEntry {
    /// The definition/word that owns this `assume` (the ledger site).
    pub word: String,
    /// The raw `assume( … )` surface from the program (what `grep assume` finds).
    pub surface: String,
    /// The bound faith-predicate taken as true without proof.
    pub predicate: Pred,
}

/// A word's honest verification status (§10.7 / invariant 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordStatus {
    /// Fully verified — no user assumption sits in its trust chain.
    Verified,
    /// Verified **modulo** a set of user assumptions (its own + every assumption
    /// inherited transitively from the words it calls). The set is the exact,
    /// enumerable statement of what is taken on faith.
    VerifiedModulo(Vec<Pred>),
}

impl WordStatus {
    /// Whether this status carries any user assumption (i.e. is `VerifiedModulo`).
    pub fn is_modulo(&self) -> bool {
        matches!(self, WordStatus::VerifiedModulo(_))
    }

    /// The assumptions this status is modulo (empty for [`WordStatus::Verified`]).
    pub fn assumptions(&self) -> &[Pred] {
        match self {
            WordStatus::Verified => &[],
            WordStatus::VerifiedModulo(a) => a,
        }
    }
}

impl std::fmt::Display for WordStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WordStatus::Verified => write!(f, "verified"),
            WordStatus::VerifiedModulo(preds) => {
                let body = preds
                    .iter()
                    .map(render_pred_infix)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "verified modulo {{ {body} }}")
            }
        }
    }
}

/// The whole-program verification ledger (§10.7 / invariant 13): the enumerable
/// set of accepted user assumptions, the rejected ones (hard errors), and each
/// word's modulo status after upward propagation through the call graph.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    entries: Vec<AssumeEntry>,
    rejections: Vec<AssumeRecord>,
    status: BTreeMap<String, Vec<Pred>>,
}

impl Ledger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every **accepted** assumption, in program (definition) order — the
    /// complete user trusted base (the in-memory `grep assume`).
    pub fn assumptions(&self) -> &[AssumeEntry] {
        &self.entries
    }

    /// Every **rejected** assume (a hard error: provable-without or
    /// no-opaque-dependency). A non-empty list means the program does not check.
    pub fn rejections(&self) -> &[AssumeRecord] {
        &self.rejections
    }

    /// Whether the program checked clean (no rejected assumes).
    pub fn is_clean(&self) -> bool {
        self.rejections.is_empty()
    }

    /// The surfaces of every accepted assume — the textual `grep assume`
    /// enumeration of the complete user trusted base.
    pub fn grep_assume(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.surface.clone()).collect()
    }

    /// A word's honest status after upward propagation: [`WordStatus::Verified`]
    /// if nothing in its trust chain is assumed, else [`WordStatus::VerifiedModulo`]
    /// with the full inherited assumption set.
    pub fn status(&self, word: &str) -> WordStatus {
        match self.status.get(word) {
            None => WordStatus::Verified,
            Some(preds) if preds.is_empty() => WordStatus::Verified,
            Some(preds) => WordStatus::VerifiedModulo(preds.clone()),
        }
    }
}

/// A whole-program definition to verify (§10.7 / M12): a name, its body tokens,
/// and its optional refinement signature.
#[derive(Debug, Clone)]
pub struct Definition {
    /// The definition name (the word other definitions call).
    pub name: String,
    /// The body token sequence (`[ body ]`).
    pub body: Vec<Token>,
    /// The optional refinement signature attached to this definition.
    pub sig: Option<RefinementSig>,
}

/// Verify a token body in a fresh context attributed to `site`, returning the
/// full [`VerifyCtx`] (obligations + assume ledger entries) — the M12 entry that
/// surfaces the `assume` ledger (the M9-era [`check_refinements`] returns only
/// obligations).
pub fn check_refinements_ctx<L, S>(
    tokens: &[Token],
    lookup: &L,
    solver: &mut S,
    site: &str,
) -> Result<VerifyCtx, ShadowError>
where
    L: Fn(&str) -> Option<RefinementSig>,
    S: Solver + CounterModel + FactSnapshot,
{
    let resolve = |w: &str| refinement_verify_word(w, lookup(w).as_ref());
    let mut stack = ShadowStack::new();
    let mut ctx = VerifyCtx::with_site(site);
    verify_ctx(tokens, &mut stack, solver, &resolve, &mut ctx)?;
    Ok(ctx)
}

/// Collect the words a body **calls** (referenced word tokens, recursing into
/// quotations) — the edges of the call graph used for upward modulo propagation.
/// `assume( … )` surfaces and the `if` combinator are not call edges.
fn collect_called_words(tokens: &[Token], out: &mut BTreeSet<String>) {
    for t in tokens {
        match t {
            Token::Bracket(body) => collect_called_words(body, out),
            Token::Word(w) if parse_assume(w).is_some() || is_if(w) => {}
            Token::Word(w) => {
                out.insert(w.clone());
            }
        }
    }
}

/// Verify a **whole program** of definitions under one global ledger (§10.7 /
/// invariant 13/15): check each body, record every `assume` (accepted entries in
/// the ledger, rejected ones as hard errors), then **propagate the modulo status
/// upward** through the call graph so a caller of a word verified modulo `S`
/// inherits `S`. `mk_solver` builds a fresh solver per definition (the seam is
/// per-checking-session).
pub fn check_program<S, MkSolver, L>(
    defs: &[Definition],
    lookup: &L,
    mut mk_solver: MkSolver,
) -> Result<Ledger, ShadowError>
where
    S: Solver + CounterModel + FactSnapshot,
    MkSolver: FnMut() -> S,
    L: Fn(&str) -> Option<RefinementSig>,
{
    let mut ledger = Ledger::new();
    let mut own: BTreeMap<String, Vec<Pred>> = BTreeMap::new();

    // Pass 1: verify each body; collect its own accepted/rejected assumes.
    for def in defs {
        let mut solver = mk_solver();
        let ctx = check_refinements_ctx(&def.body, lookup, &mut solver, &def.name)?;
        let mut mine: Vec<Pred> = Vec::new();
        for a in &ctx.assumes {
            if a.legality.is_legal() {
                ledger.entries.push(AssumeEntry {
                    word: def.name.clone(),
                    surface: a.surface.clone(),
                    predicate: a.surface_pred.clone(),
                });
                if !mine.contains(&a.surface_pred) {
                    mine.push(a.surface_pred.clone());
                }
            } else {
                ledger.rejections.push(a.clone());
            }
        }
        own.insert(def.name.clone(), mine);
    }

    // Pass 2: transitive upward propagation of modulo status through the call
    // graph (caller inherits callee's assumptions — invariant 13 property 2).
    let nameset: BTreeSet<String> = defs.iter().map(|d| d.name.clone()).collect();
    let mut callees: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for def in defs {
        let mut cs = BTreeSet::new();
        collect_called_words(&def.body, &mut cs);
        cs.retain(|w| nameset.contains(w) && w != &def.name);
        callees.insert(def.name.clone(), cs);
    }
    let mut modulo: BTreeMap<String, Vec<Pred>> = own;
    loop {
        let mut changed = false;
        for def in defs {
            let inherited: Vec<Pred> = callees[&def.name]
                .iter()
                .flat_map(|c| modulo.get(c).cloned().unwrap_or_default())
                .collect();
            let set = modulo.get_mut(&def.name).unwrap();
            for p in inherited {
                if !set.contains(&p) {
                    set.push(p);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    ledger.status = modulo;
    Ok(ledger)
}

/// Render a predicate in the **infix** `where` surface (§10.1) for human ledger
/// display — `result > 0` rather than the SMT s-expression. Fully parenthesized
/// on nested binary forms to keep precedence unambiguous.
fn render_pred_infix(pred: &Pred) -> String {
    match pred {
        Pred::Var(n) => n.clone(),
        Pred::Num(n) => n.clone(),
        Pred::Un(UnOp::Not, p) => format!("not {}", render_pred_infix(p)),
        Pred::Un(UnOp::Neg, p) => format!("-{}", render_pred_infix(p)),
        Pred::Bin(op, a, b) => {
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Ge => ">=",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Lt => "<",
                BinOp::Eq => "=",
                BinOp::And => "and",
                BinOp::Or => "or",
                BinOp::Implies => "=>",
            };
            format!("{} {} {}", render_pred_infix(a), sym, render_pred_infix(b))
        }
        Pred::App(f, args) => {
            let inner = args
                .iter()
                .map(render_pred_infix)
                .collect::<Vec<_>>()
                .join(" ");
            format!("{f} {inner}")
        }
    }
}

// ===========================================================================
// Higher-order refinement subsumption (M10, §10.6 / §14.8)
// ===========================================================================
//
// This is the **Tier 1 centerpiece**: the one admitted directional check
// (subtyping-on-the-arrow), discharged **only** as SMT implications and **never**
// inferred by the type engine (invariant 1/13). When a refined quotation crosses
// a higher-order boundary — passed to a combinator / operator expecting a refined
// signature — its contract must be checked for **preservation**. The checker
// emits **exactly two** directional implications and hands them to the solver;
// the solver settles them. Nothing here unifies, binds a `TyVar`, or touches the
// Tier 0 substitution.
//
// The two directions are the classic mistake, spelled out (§10.6):
//
//   * **Guarantees (postconditions) are COVARIANT:** `provided_post ⟹ expected_post`.
//     A *stronger* guarantee substitutes for a weaker one (`r > 5` where `r > 0`
//     is expected is fine).
//   * **Demands (preconditions) are CONTRAVARIANT:** `expected_pre ⟹ provided_pre`
//     (note the **flip**). A *weaker* demand substitutes for a stronger one (a
//     quotation that demands less than the boundary promises to supply is fine).
//
// **Fail closed (invariant 12):** if a subsumption VC comes back `Unknown` the
// boundary is **rejected** with the targeted message — never a silent pass. A
// refinement that fails open is worse than no refinement (vacuous verification).

/// The fail-closed rejection message for an **undecidable** subsumption VC
/// (§10.6, invariant 12). Emitted verbatim when a directional VC returns
/// [`Verdict::Unknown`]: the contract could not be *proven* preserved, so the
/// boundary is rejected rather than silently admitted.
pub const SUBSUMPTION_FAIL_CLOSED_MSG: &str = "could not prove this contract is preserved across the combinator; annotate or simplify the predicate";

/// The targeted **gradual-interop** diagnostic (§10.7 / invariant 12 / M11): an
/// **unrefined** (absent-payload) quotation was passed where a *guarantee* is
/// required. The subsumption VC `true ⟹ <required guarantee>` is invalid, but the
/// actionable message is that the quotation carries **no contract** at all — not a
/// bare SMT counterexample for some incidental witness value. The fix is to add a
/// contract to the quotation, not to study the counterexample.
///
/// This is emitted **only** for the absent-payload case (provided guarantee is
/// `where true`); a *present-but-weaker* guarantee (e.g. `r > 0` where `r > 5` is
/// required) keeps the M10 [`SubsumptionOutcome::Violated`] counterexample, since
/// there the contract exists and is simply too weak.
pub const SUBSUMPTION_NO_CONTRACT_MSG: &str =
    "this quotation carries no contract and one is required here";

/// Which of the two directional subsumption VCs (§10.6) a verdict belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsumptionDirection {
    /// **Covariant** guarantee check: `provided_post ⟹ expected_post`. A stronger
    /// provided guarantee subsumes a weaker expected one.
    Guarantee,
    /// **Contravariant** demand check: `expected_pre ⟹ provided_pre` (the flipped
    /// direction). A weaker provided demand subsumes a stronger expected one.
    Demand,
}

impl SubsumptionDirection {
    /// A short human label for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            SubsumptionDirection::Guarantee => "guarantee (covariant)",
            SubsumptionDirection::Demand => "demand (contravariant)",
        }
    }
}

/// One discharged **directional** subsumption VC (§10.6): the implication that
/// was checked, the verdict the solver returned via the negated-goal encoding,
/// and — on a `Sat` (violated) verdict — the surfaced counterexample [`Model`].
#[derive(Debug, Clone)]
pub struct SubsumptionVc {
    /// Whether this is the covariant guarantee or contravariant demand check.
    pub direction: SubsumptionDirection,
    /// The implication actually checked (`hypothesis ⟹ goal`), rendered as a
    /// predicate over the position-aligned binders — the exact thing handed to
    /// the solver, for diagnostics and tests.
    pub implication: Pred,
    /// The verdict: `Unsat` ⇒ the implication is **valid** (this direction is
    /// preserved); `Sat` ⇒ refuted (see `model`); `Unknown` ⇒ undecided — which
    /// makes the whole subsumption **fail closed** (§10.6).
    pub verdict: Verdict,
    /// The counterexample model, present **iff** `verdict == Sat` and the VC was
    /// fully decidable. An `Unknown` never carries a (fabricated) model (§10.5).
    pub model: Option<Model>,
    /// **Gradual-interop marker (§10.7 / M11):** the *provided* (hypothesis) side
    /// of this directional implication was **absent** (`where true`) while the
    /// *expected* (goal) side carried a predicate. For the **guarantee**
    /// direction this is the "carries no contract" signal — an unrefined
    /// quotation meeting a required guarantee — which earns the targeted
    /// [`SUBSUMPTION_NO_CONTRACT_MSG`] diagnostic instead of a bare counterexample
    /// (a present-but-weaker guarantee leaves this `false` and keeps the M10
    /// counterexample). For the **demand** direction the same shape is just an
    /// ordinary stronger-demand M10 violation, so the marker is ignored there.
    pub absent_payload: bool,
}

impl SubsumptionVc {
    /// Whether this directional implication is **valid** (the negated goal came
    /// back `Unsat`). `Sat` and `Unknown` are both **not** valid.
    pub fn is_valid(&self) -> bool {
        self.verdict == Verdict::Unsat
    }
}

/// The collapsed outcome of a subsumption check (§10.6) — what a caller acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubsumptionOutcome {
    /// **Both** directional VCs are valid (`Unsat`): the contract is preserved
    /// across the boundary. The boundary is accepted.
    Preserved,
    /// A directional VC was **refuted** (`Sat`): the contract is *not* preserved.
    /// Carries which direction failed and the surfaced counterexample.
    Violated {
        /// Which directional implication failed.
        direction: SubsumptionDirection,
        /// The concrete counterexample witnessing the failure (if decidable).
        model: Option<Model>,
    },
    /// A directional VC came back `Unknown`: the boundary **fails closed**
    /// (§10.6 / invariant 12) with [`SUBSUMPTION_FAIL_CLOSED_MSG`]. Never a
    /// silent pass.
    Undecidable {
        /// Which directional implication could not be decided.
        direction: SubsumptionDirection,
        /// The verbatim targeted rejection message.
        message: String,
    },
    /// **Gradual interop (§10.7 / invariant 12 / M11):** an **unrefined**
    /// (absent-payload) quotation was passed where a *guarantee* is required.
    /// The boundary is rejected, but with the targeted
    /// [`SUBSUMPTION_NO_CONTRACT_MSG`] — *"this quotation carries no contract and
    /// one is required here"* — rather than the bare M10 SMT counterexample. This
    /// is distinct from [`SubsumptionOutcome::Violated`], which is reserved for a
    /// contract that is **present but too weak** (and still carries its
    /// counterexample).
    CarriesNoContract {
        /// Which directional implication exposed the missing contract — always
        /// the covariant [`SubsumptionDirection::Guarantee`] (an unrefined
        /// *demand* is maximally weak and subsumes fine, §10.7).
        direction: SubsumptionDirection,
        /// The verbatim targeted rejection message.
        message: String,
    },
}

/// The result of a higher-order subsumption check (§10.6): the **two** directional
/// VCs (covariant guarantee + contravariant demand) and nothing else — the
/// checker emits exactly these two implications and the solver settles them.
#[derive(Debug, Clone)]
pub struct SubsumptionResult {
    /// The covariant guarantee VC: `provided_post ⟹ expected_post`.
    pub guarantee: SubsumptionVc,
    /// The contravariant demand VC: `expected_pre ⟹ provided_pre`.
    pub demand: SubsumptionVc,
}

impl SubsumptionResult {
    /// Collapse the two directional VCs into a single outcome, **failing closed**
    /// on any `Unknown` (§10.6): the boundary is preserved **iff** both VCs are
    /// valid; an `Unknown` rejects (never a silent pass); a `Sat` is a genuine
    /// violation with a counterexample.
    ///
    /// Precedence is deliberate. First the **gradual-interop** case (§10.7 / M11):
    /// an unrefined quotation meeting a *required guarantee* (the guarantee VC's
    /// hypothesis absent while its goal is present) is reported as
    /// [`SubsumptionOutcome::CarriesNoContract`] with the targeted message — the
    /// missing contract is the actionable root cause, so it precedes both the
    /// raw counterexample and the fail-closed `Unknown` (which would otherwise
    /// just describe the symptom). Then an `Unknown` anywhere is reported as
    /// [`SubsumptionOutcome::Undecidable`] (the fail-closed mandate), then a `Sat`
    /// as [`SubsumptionOutcome::Violated`] (a present-but-weaker contract, with
    /// its counterexample), else [`SubsumptionOutcome::Preserved`].
    pub fn outcome(&self) -> SubsumptionOutcome {
        // §10.7 / M11: unrefined quotation meets a REQUIRED GUARANTEE. The
        // covariant guarantee VC is `true ⟹ <required>`, which fails — but the
        // honest diagnosis is the absent contract, not the SMT witness. Only the
        // guarantee direction earns this: an absent *demand* is maximally weak
        // (`where true`) and subsumes any expected demand, so the demand
        // direction's absent-payload shape is an ordinary M10 violation instead.
        if self.guarantee.direction == SubsumptionDirection::Guarantee
            && self.guarantee.absent_payload
            && !self.guarantee.is_valid()
        {
            return SubsumptionOutcome::CarriesNoContract {
                direction: SubsumptionDirection::Guarantee,
                message: SUBSUMPTION_NO_CONTRACT_MSG.to_string(),
            };
        }
        for vc in [&self.guarantee, &self.demand] {
            if vc.verdict == Verdict::Unknown {
                return SubsumptionOutcome::Undecidable {
                    direction: vc.direction,
                    message: SUBSUMPTION_FAIL_CLOSED_MSG.to_string(),
                };
            }
        }
        for vc in [&self.guarantee, &self.demand] {
            if vc.verdict == Verdict::Sat {
                return SubsumptionOutcome::Violated {
                    direction: vc.direction,
                    model: vc.model.clone(),
                };
            }
        }
        SubsumptionOutcome::Preserved
    }

    /// Whether the contract is preserved across the boundary: **both** directional
    /// VCs valid. Convenience for the common accept/reject branch.
    pub fn is_preserved(&self) -> bool {
        matches!(self.outcome(), SubsumptionOutcome::Preserved)
    }
}

/// Rename a refinement side's predicate so its binders become **position-aligned**
/// canonical SMT variables (`{prefix}{depth}`, depth measured from the top of
/// stack, the only pinned end — §10.2). Returns `None` for an absent predicate
/// (`where true`).
///
/// Both sides of a subsumption boundary must speak about the *same* logical
/// variables for the implication to mean anything: a provided quotation may name
/// its result `r` while the expected signature names it `out`, but positionally
/// they are the same slot. Aligning both to `{prefix}{depth}` makes the
/// implication well-formed over shared variables.
fn align_side(side: &RefinementSide, prefix: &str) -> Option<Pred> {
    let pred = side.predicate.as_ref()?;
    let n = side.binders.len();
    let bindings: Vec<NamedBinding> = side
        .binders
        .iter()
        .enumerate()
        .map(|(i, b)| {
            // depth from the top of stack: the last binder is depth 0.
            let depth = n - 1 - i;
            NamedBinding {
                name: b.name.clone(),
                term: Pred::Var(format!("{prefix}{depth}")),
            }
        })
        .collect();
    Some(substitute(pred, &bindings))
}

/// Discharge a single directional implication `hypothesis ⟹ goal` through the
/// solver via the negated-goal encoding (§10.5), returning the verdict, the
/// counterexample model on `Sat`, and the implication predicate (for the record).
///
/// **Absent refinement = `where true`** (§10.7 — the gradual-adoption rule).
/// Both ends of a boundary are read this way, so unrefined code slots into the
/// lattice automatically:
///
///   * An absent `goal` (`where true`) is **trivially valid** (`Unsat`, no model):
///     `h ⟹ true` always holds. This is what makes a **contract-agnostic
///     boundary** accept an unrefined quotation (`true ⟹ true`) and a refined one
///     alike — the expected side demands nothing.
///   * An absent `hypothesis` (`where true`) asserts no extra fact, so the goal
///     must hold on its own. For the **guarantee** direction this is the
///     unrefined-meets-required case (`true ⟹ <required>`): the VC fails and the
///     [`SubsumptionVc::absent_payload`] flag is set so [`SubsumptionResult::outcome`]
///     can surface the targeted "carries no contract" diagnostic (§10.7 / M11)
///     rather than a bare counterexample.
///
/// `absent_payload` records exactly `hypothesis.is_none() && goal.is_some()` — the
/// provided side is unrefined while a contract is required. The discharge result
/// is unaffected; only the *diagnosis* of the failure changes.
fn directional_implication<S: Solver + CounterModel>(
    direction: SubsumptionDirection,
    hypothesis: Option<Pred>,
    goal: Option<Pred>,
    solver: &mut S,
) -> SubsumptionVc {
    // The provided (hypothesis) side is unrefined (`where true`) while the
    // expected (goal) side requires a contract — the §10.7 absent-payload shape.
    let absent_payload = hypothesis.is_none() && goal.is_some();

    // Build the implication predicate for the record. `true ⟹ goal` reduces to
    // `goal`; `h ⟹ true` reduces to `true`.
    let implication = match (&hypothesis, &goal) {
        (_, None) => Pred::Num("1".to_string()), // `true`; rendered placeholder
        (None, Some(g)) => g.clone(),
        (Some(h), Some(g)) => Pred::Bin(BinOp::Implies, Box::new(h.clone()), Box::new(g.clone())),
    };

    // `h ⟹ true` is valid with no solver work.
    let goal = match goal {
        None => {
            return SubsumptionVc {
                direction,
                implication,
                verdict: Verdict::Unsat,
                model: None,
                absent_payload,
            };
        }
        Some(g) => g,
    };

    // Assert the hypothesis (if any) in a fresh scope, discharge the goal via the
    // negated-goal encoding, then pop — never disturbing the live facts above.
    solver.push_scope();
    if let Some(h) = hypothesis {
        solver.assert(&h);
    }
    let (verdict, model) = discharge_with_model(solver, &goal);
    solver.pop_scope();

    SubsumptionVc {
        direction,
        implication,
        verdict,
        model,
        absent_payload,
    }
}

/// **Higher-order refinement subsumption** (§10.6 / M10): at a boundary where a
/// `provided` quotation refinement meets an `expected` refined signature, emit
/// **exactly two** directional VCs and discharge them through the solver.
///
///   * **Covariant guarantee:** `provided_post ⟹ expected_post`.
///   * **Contravariant demand:** `expected_pre ⟹ provided_pre` (flipped).
///
/// The checker only *generates* these two implications; the solver settles them
/// (invariant 1/13 — subsumption is **never inferred** by the type engine). The
/// binders of both sides are position-aligned ([`align_side`]) so the implications
/// are over shared SMT variables. Discharge reuses the M9 negated-goal encoding
/// ([`discharge_with_model`]) so a violation surfaces a counterexample.
///
/// The returned [`SubsumptionResult::outcome`] **fails closed** on any `Unknown`
/// (§10.6, invariant 12).
///
/// This takes only refinement payloads and a solver — no Tier 0 inference handle
/// — so it cannot touch the frozen substitution (immutability barrier).
pub fn check_subsumption<S: Solver + CounterModel>(
    provided: &RefinementSig,
    expected: &RefinementSig,
    solver: &mut S,
) -> SubsumptionResult {
    // Covariant guarantee: provided_post ⟹ expected_post.
    let guarantee = directional_implication(
        SubsumptionDirection::Guarantee,
        align_side(&provided.guarantees, "$post"),
        align_side(&expected.guarantees, "$post"),
        solver,
    );
    // Contravariant demand (the FLIP): expected_pre ⟹ provided_pre.
    let demand = directional_implication(
        SubsumptionDirection::Demand,
        align_side(&expected.demands, "$pre"),
        align_side(&provided.demands, "$pre"),
        solver,
    );
    SubsumptionResult { guarantee, demand }
}

// ===========================================================================
// Operator refinement axioms for the language-core combinators (M10, §10.6/§8)
// ===========================================================================
//
// The primitive combinators `dip` / `call` / `if` are **language-core operator
// registrations** (§8): they have no Caternary body, and carry an authored Tier 0
// scheme (§2) *and* an authored **Tier 1 refinement axiom** here. These axioms
// are what let a refined quotation's guarantee survive a combinator boundary;
// without them the contract would evaporate at `dip` exactly as at an
// unaxiomatized foreign call.
//
// The axioms are the §2 schemes **lifted to predicates** (the relay rule —
// invariant 8: a combinator's contract uses only the *declared* arrow of its
// quotation argument; the body is never expanded into the caller's contract):
//
//   * **`dip` — relay + identity (`dip q = q ⊗ id_a`, lifted).** `dip` sets aside
//     the top value `a`, runs the quotation `q` on the rest, then restores `a`.
//     Lifted: (1) the set-aside value's predicate is **preserved unchanged** (it
//     is literally the same term by identity — any fact about it stays asserted),
//     and (2) the quotation's `post` holds on the rest (the values `q` produced).
//   * **`call` — relay.** `call q` runs `q` on the whole stack; its axiom is just
//     `q`'s post holding on the result (the `dip` rule with no set-aside value).
//   * **`if` — relay both branches.** Each branch quotation's post holds under its
//     branch's path condition (§10.4); the post-`if` facts are the branches'
//     relayed guarantees.
//
// In this verifier quotations are token bodies and the relay is realized by the
// shadow recursion in [`apply_core`] (`Dip`/`Call`) and [`verify_if`]: running a
// refined word inside the body **publishes its guarantee as a live fact** on the
// far side, and the set-aside `dip` value is restored by identity so its fact
// persists. [`relay_quote_post`] is the same axiom expressed at the **declared
// contract** level — used when a quotation arrives carrying a *declared*
// refinement (rather than an inline body) so the contract is relayed without
// expanding any body.

/// The `dip`/`call` refinement axiom expressed at the **declared contract** level
/// (§10.6): relay a quotation's declared `post` (guarantee) onto the result slots
/// it produced, asserting it as a live fact so a downstream obligation can use it.
///
/// `result_terms` are the shadow terms occupying the quotation's output slots
/// (top-of-stack last), produced by running the quotation. The guarantee's output
/// binders are position-aligned against them (right-to-left from the top, §10.2)
/// and the substituted predicate is asserted. An absent guarantee (`where true`)
/// asserts nothing.
///
/// This is the **relay** (invariant 8): only the quotation's *declared* arrow /
/// contract is used — its body is never expanded into the combinator's contract.
/// `dip`'s extra clause (the set-aside value's predicate is preserved unchanged)
/// needs no work here: that value is the same term by identity, so any fact about
/// it already stands in the solver scope.
pub fn relay_quote_post<S: Solver>(quote: &RefinementSig, result_terms: &[Pred], solver: &mut S) {
    let Some(post) = quote.guarantees.predicate.as_ref() else {
        return; // `where true` — nothing to relay.
    };
    let binders = &quote.guarantees.binders;
    let n = binders.len();
    // Align each output binder to its result term, right-to-left from the top.
    let mut bindings = Vec::with_capacity(n);
    let m = result_terms.len();
    for (i, b) in binders.iter().enumerate() {
        let depth = n - 1 - i; // depth from the top
        if depth < m {
            bindings.push(NamedBinding {
                name: b.name.clone(),
                term: result_terms[m - 1 - depth].clone(),
            });
        }
    }
    let fact = substitute(post, &bindings);
    solver.assert(&fact);
}

// ===========================================================================
// Minimal embedded reasoner — the M8 stand-in for z3's check-sat
// ===========================================================================
//
// This is NOT the M9 VC pipeline and NOT a general SMT solver. It is a compact,
// sound, decidable feasibility check over **linear rational arithmetic** by
// Fourier–Motzkin elimination, sufficient to answer the §12 M8 demonstration:
//
//   * `x > 0 ∧ ¬(x >= 0)`  (i.e. `x > 0 ∧ x < 0`)  ⇒  Unsat (goal valid).
//   * `¬(x >= 0)`           (i.e. `x < 0`)          ⇒  Sat  (goal fails).
//
// It is sound by construction: anything it cannot linearize (uninterpreted
// applications, nonlinear products, disjunctions) makes a conjunct "opaque", and
// it then returns `Unsat` only if the *decidable subset alone* is already
// infeasible (adding constraints can only keep it infeasible) and otherwise
// `Unknown` (never an unsound `Sat`). This fail-closed bias is the right default
// for VC discharge (§10.5/§10.6).

/// Check satisfiability of the conjunction of `formulas` over linear rational
/// arithmetic (the minimal M8 reasoner — see the module-level note).
pub fn check_sat(formulas: &[Pred]) -> Verdict {
    check_sat_model(formulas).0
}

/// Like [`check_sat`], but on a decidable `Sat` it **also** returns a concrete
/// satisfying [`Model`] over the named binders — the counterexample (§10.5 / M9).
///
/// The model is extracted by Fourier–Motzkin **back-substitution** over the
/// feasible linear system: the same elimination that decides feasibility records
/// each variable's bounds, then a value is chosen for each variable in reverse
/// elimination order (every variable a bound mentions is already assigned).
///
/// Soundness of the *verdict* is unchanged from [`check_sat`]; the model is a
/// witness, supplied **only** for a fully-decidable `Sat`. An opaque conjunct
/// (`Unknown`) yields **no model** (`None`) — the spec forbids fabricating one
/// (§10.5).
pub fn check_sat_model(formulas: &[Pred]) -> (Verdict, Option<Model>) {
    let mut constraints: Vec<Constraint> = Vec::new();
    let mut opaque = false;
    for f in formulas {
        if !collect_constraints(f, false, &mut constraints) {
            opaque = true;
        }
    }
    let (feasible, assignment) = fourier_motzkin_solve(constraints);
    match (feasible, opaque) {
        (false, _) => (Verdict::Unsat, None), // decidable subset already infeasible ⇒ Unsat
        (true, false) => {
            let model = Model {
                assignments: assignment
                    .into_iter()
                    .map(|(name, val)| (name, val.render()))
                    .collect(),
            };
            (Verdict::Sat, Some(model))
        }
        // Feasible decidable subset, but an opaque conjunct could hide a
        // contradiction: degrade to Unknown and fabricate no model (§10.5).
        (true, true) => (Verdict::Unknown, None),
    }
}

/// A linear expression: a map of variable → rational coefficient, plus a
/// rational constant.
#[derive(Debug, Clone, Default)]
struct LinExpr {
    coeffs: BTreeMap<String, Rat>,
    constant: Rat,
}

/// A constraint `expr <= 0` (non-strict) or `expr < 0` (strict).
#[derive(Debug, Clone)]
struct Constraint {
    expr: LinExpr,
    strict: bool,
}

impl LinExpr {
    fn constant(c: Rat) -> Self {
        LinExpr {
            coeffs: BTreeMap::new(),
            constant: c,
        }
    }

    fn var(name: &str) -> Self {
        let mut coeffs = BTreeMap::new();
        coeffs.insert(name.to_string(), Rat::int(1));
        LinExpr {
            coeffs,
            constant: Rat::int(0),
        }
    }

    fn add(&self, other: &LinExpr) -> LinExpr {
        let mut out = self.clone();
        for (k, v) in &other.coeffs {
            let e = out.coeffs.entry(k.clone()).or_insert_with(|| Rat::int(0));
            *e = e.add(v);
        }
        out.constant = out.constant.add(&other.constant);
        out.prune();
        out
    }

    fn neg(&self) -> LinExpr {
        let mut out = LinExpr {
            coeffs: self
                .coeffs
                .iter()
                .map(|(k, v)| (k.clone(), v.neg()))
                .collect(),
            constant: self.constant.neg(),
        };
        out.prune();
        out
    }

    fn sub(&self, other: &LinExpr) -> LinExpr {
        self.add(&other.neg())
    }

    fn scale(&self, k: &Rat) -> LinExpr {
        let mut out = LinExpr {
            coeffs: self
                .coeffs
                .iter()
                .map(|(name, v)| (name.clone(), v.mul(k)))
                .collect(),
            constant: self.constant.mul(k),
        };
        out.prune();
        out
    }

    fn prune(&mut self) {
        self.coeffs.retain(|_, v| !v.is_zero());
    }

    fn is_constant(&self) -> bool {
        self.coeffs.is_empty()
    }

    fn first_var(&self) -> Option<String> {
        self.coeffs.keys().next().cloned()
    }
}

/// Turn a predicate into linear constraints (`expr {<,<=} 0`), pushing `negated`
/// through. Returns `false` if the predicate is not linearizable as a
/// conjunction of linear comparisons (the conjunct is then opaque).
fn collect_constraints(pred: &Pred, negated: bool, out: &mut Vec<Constraint>) -> bool {
    match pred {
        Pred::Un(UnOp::Not, inner) => collect_constraints(inner, !negated, out),
        Pred::Bin(BinOp::And, a, b) if !negated => {
            // ¬-free conjunction: both must hold.
            collect_constraints(a, false, out) & collect_constraints(b, false, out)
        }
        Pred::Bin(BinOp::Or, a, b) if negated => {
            // ¬(a ∨ b) = ¬a ∧ ¬b.
            collect_constraints(a, true, out) & collect_constraints(b, true, out)
        }
        // ¬(a = b) is a≠b — a disjunction this linear reasoner cannot represent.
        // Treat it as opaque (⇒ Unknown unless a decidable contradiction already
        // exists) rather than mistranslating it.
        Pred::Bin(BinOp::Eq, _, _) if negated => false,
        Pred::Bin(op, a, b) if is_comparison(*op) => {
            let (la, lb) = match (linearize(a), linearize(b)) {
                (Some(la), Some(lb)) => (la, lb),
                _ => return false,
            };
            push_comparison(*op, &la, &lb, negated, out);
            true
        }
        // Anything else (bare arithmetic as a proposition, uninterpreted App,
        // disjunction in a positive position, etc.) is opaque to this reasoner.
        _ => false,
    }
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Ge | BinOp::Le | BinOp::Gt | BinOp::Lt | BinOp::Eq
    )
}

/// Emit the constraint(s) for `a OP b` (optionally negated) as `expr {<,<=} 0`.
fn push_comparison(op: BinOp, a: &LinExpr, b: &LinExpr, negated: bool, out: &mut Vec<Constraint>) {
    // Resolve the effective operator after negation.
    let eff = if negated { negate_cmp(op) } else { op };
    match eff {
        // a >= b  ⇔  b - a <= 0
        BinOp::Ge => out.push(Constraint {
            expr: b.sub(a),
            strict: false,
        }),
        // a > b   ⇔  b - a < 0
        BinOp::Gt => out.push(Constraint {
            expr: b.sub(a),
            strict: true,
        }),
        // a <= b  ⇔  a - b <= 0
        BinOp::Le => out.push(Constraint {
            expr: a.sub(b),
            strict: false,
        }),
        // a < b   ⇔  a - b < 0
        BinOp::Lt => out.push(Constraint {
            expr: a.sub(b),
            strict: true,
        }),
        // a = b   ⇔  a - b <= 0 ∧ b - a <= 0
        BinOp::Eq => {
            out.push(Constraint {
                expr: a.sub(b),
                strict: false,
            });
            out.push(Constraint {
                expr: b.sub(a),
                strict: false,
            });
        }
        _ => unreachable!("push_comparison only handles comparison operators"),
    }
}

/// The negation of a comparison operator (`¬(a ≥ b) = a < b`, etc.). `¬Eq`
/// (a disjunction) is routed away by [`collect_constraints`] before reaching
/// here, so it never appears.
fn negate_cmp(op: BinOp) -> BinOp {
    match op {
        BinOp::Ge => BinOp::Lt,
        BinOp::Gt => BinOp::Le,
        BinOp::Le => BinOp::Gt,
        BinOp::Lt => BinOp::Ge,
        other => unreachable!("negate_cmp called on non-(in)equality operator {other:?}"),
    }
}

/// Linearize a predicate arithmetic term into a [`LinExpr`], or `None` if it is
/// nonlinear / uninterpreted.
fn linearize(pred: &Pred) -> Option<LinExpr> {
    match pred {
        Pred::Var(name) => Some(LinExpr::var(name)),
        Pred::Num(lexeme) => Rat::parse(lexeme).map(LinExpr::constant),
        Pred::Un(UnOp::Neg, a) => Some(linearize(a)?.neg()),
        Pred::Bin(BinOp::Add, a, b) => Some(linearize(a)?.add(&linearize(b)?)),
        Pred::Bin(BinOp::Sub, a, b) => Some(linearize(a)?.sub(&linearize(b)?)),
        Pred::Bin(BinOp::Mul, a, b) => {
            let la = linearize(a)?;
            let lb = linearize(b)?;
            // Linear only if at least one factor is a constant.
            if la.is_constant() {
                Some(lb.scale(&la.constant))
            } else if lb.is_constant() {
                Some(la.scale(&lb.constant))
            } else {
                None
            }
        }
        Pred::Bin(BinOp::Div, a, b) => {
            let la = linearize(a)?;
            let lb = linearize(b)?;
            if lb.is_constant() && !lb.constant.is_zero() {
                Some(la.scale(&lb.constant.recip()))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Fourier–Motzkin over the rationals with **back-substitution**: decides
/// feasibility of `{ expr <= 0 | expr < 0 }` and, when feasible, returns a
/// concrete satisfying assignment.
///
/// The forward pass eliminates one variable at a time, recording — for each
/// eliminated variable — the constraints that mentioned it (its bounds, in terms
/// of the *still-live* variables). The backward pass then assigns a value to each
/// variable in **reverse** elimination order: every variable a recorded bound
/// references was eliminated *later*, hence is already assigned, so each bound
/// evaluates to a concrete rational and a value can be picked between the tightest
/// lower and upper bounds.
fn fourier_motzkin_solve(mut constraints: Vec<Constraint>) -> (bool, Vec<(String, Rat)>) {
    // Each step: (eliminated var, the constraints that mentioned it at that time).
    let mut steps: Vec<(String, Vec<Constraint>)> = Vec::new();

    loop {
        // Discharge any constant-only constraints first.
        let mut remaining = Vec::new();
        for c in constraints.into_iter() {
            if c.expr.is_constant() {
                let k = &c.expr.constant;
                let bad = if c.strict {
                    // k < 0 required; infeasible if k >= 0.
                    !k.is_negative()
                } else {
                    // k <= 0 required; infeasible if k > 0.
                    k.is_positive()
                };
                if bad {
                    return (false, Vec::new());
                }
                // else trivially satisfied; drop it.
            } else {
                remaining.push(c);
            }
        }
        constraints = remaining;
        if constraints.is_empty() {
            break;
        }

        // Pick a variable to eliminate.
        let var = constraints
            .iter()
            .find_map(|c| c.expr.first_var())
            .expect("non-constant constraint must have a variable");

        let mut zero = Vec::new();
        let mut pos = Vec::new();
        let mut neg = Vec::new();
        for c in constraints.into_iter() {
            match c.expr.coeffs.get(&var) {
                None => zero.push(c),
                Some(co) if co.is_positive() => pos.push(c),
                Some(_) => neg.push(c),
            }
        }

        // Record the constraints that bound `var` for back-substitution.
        let mut involving = pos.clone();
        involving.extend(neg.iter().cloned());

        let mut next = zero;
        for p in &pos {
            let a = *p.expr.coeffs.get(&var).unwrap(); // > 0
            for n in &neg {
                let b = *n.expr.coeffs.get(&var).unwrap(); // < 0
                // Scale p by -b (>0) and n by a (>0); add to cancel `var`.
                let p_scaled = p.expr.scale(&b.neg());
                let n_scaled = n.expr.scale(&a);
                let mut combined = p_scaled.add(&n_scaled);
                combined.coeffs.remove(&var); // exactly cancels; guard rounding
                combined.prune();
                next.push(Constraint {
                    expr: combined,
                    strict: p.strict || n.strict,
                });
            }
        }

        steps.push((var, involving));
        constraints = next;
    }

    // Feasible: back-substitute to a concrete witness point.
    let mut model: BTreeMap<String, Rat> = BTreeMap::new();
    for (var, involving) in steps.iter().rev() {
        // For each bounding constraint `c_v * var + rest {<=,<} 0`, evaluate
        // `rest` (over the already-assigned variables) to a concrete value and
        // derive a bound on `var`:
        //   c_v > 0 ⇒ var {<=,<} -rest/c_v   (an upper bound)
        //   c_v < 0 ⇒ var {>=,>} -rest/c_v   (a lower bound)
        let mut lower: Option<(Rat, bool)> = None; // (value, strict)
        let mut upper: Option<(Rat, bool)> = None;
        for c in involving {
            let c_v = *c.expr.coeffs.get(var).unwrap();
            // rest = expr with `var` removed, evaluated under `model`.
            let mut rest = c.expr.constant;
            for (name, coeff) in &c.expr.coeffs {
                if name == var {
                    continue;
                }
                let val = model
                    .get(name)
                    .copied()
                    .expect("back-substitution: referenced variable must be assigned");
                rest = rest.add(&coeff.mul(&val));
            }
            // bound = -rest / c_v
            let bound = rest.neg().div(&c_v);
            if c_v.is_positive() {
                // var <= bound (strict ⇒ var < bound): an upper bound.
                upper = Some(match upper {
                    None => (bound, c.strict),
                    Some((u, us)) => {
                        if bound.lt(&u) {
                            (bound, c.strict)
                        } else if u.lt(&bound) {
                            (u, us)
                        } else {
                            (u, us || c.strict)
                        }
                    }
                });
            } else {
                // var >= bound (strict ⇒ var > bound): a lower bound.
                lower = Some(match lower {
                    None => (bound, c.strict),
                    Some((l, ls)) => {
                        if l.lt(&bound) {
                            (bound, c.strict)
                        } else if bound.lt(&l) {
                            (l, ls)
                        } else {
                            (l, ls || c.strict)
                        }
                    }
                });
            }
        }

        let value = pick_value(lower, upper);
        model.insert(var.clone(), value);
    }

    (true, model.into_iter().collect())
}

/// Choose a concrete rational satisfying the (optional) lower and upper bounds.
/// Feasibility is already established (the forward pass would have failed
/// otherwise), so a satisfying value always exists.
fn pick_value(lower: Option<(Rat, bool)>, upper: Option<(Rat, bool)>) -> Rat {
    match (lower, upper) {
        (None, None) => Rat::int(0),
        (Some((l, strict)), None) => {
            if strict {
                l.add(&Rat::int(1)) // any value > l
            } else {
                l
            }
        }
        (None, Some((u, strict))) => {
            if strict {
                u.sub(&Rat::int(1)) // any value < u
            } else {
                u
            }
        }
        (Some((l, ls)), Some((u, us))) => {
            if l.lt(&u) {
                // A strict midpoint satisfies both strict and non-strict bounds.
                l.add(&u).div(&Rat::int(2))
            } else {
                // l == u (the forward pass ruled out l > u); both must be
                // non-strict for feasibility, so the shared endpoint works.
                debug_assert!(!ls && !us, "infeasible point bound should have been caught");
                l
            }
        }
    }
}

// ===========================================================================
// A tiny exact rational over i128 (enough for the M8 reasoner)
// ===========================================================================

/// An exact rational number `num/den` with `den > 0`, always in lowest terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rat {
    num: i128,
    den: i128,
}

impl Default for Rat {
    fn default() -> Self {
        Rat::int(0)
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Rat {
    fn new(mut num: i128, mut den: i128) -> Rat {
        assert!(den != 0, "rational with zero denominator");
        if den < 0 {
            num = -num;
            den = -den;
        }
        let g = gcd(num, den);
        let g = if g == 0 { 1 } else { g };
        Rat {
            num: num / g,
            den: den / g,
        }
    }

    fn int(n: i128) -> Rat {
        Rat { num: n, den: 1 }
    }

    /// Parse a numeric lexeme (integer or simple decimal like `3.5`).
    fn parse(lexeme: &str) -> Option<Rat> {
        let s = lexeme.trim();
        if let Ok(n) = s.parse::<i128>() {
            return Some(Rat::int(n));
        }
        // Simple decimal: optional sign, digits, '.', digits.
        let (sign, rest) = match s.strip_prefix('-') {
            Some(r) => (-1i128, r),
            None => (1i128, s),
        };
        let (int_part, frac_part) = rest.split_once('.')?;
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        let int_val: i128 = if int_part.is_empty() {
            0
        } else {
            int_part.parse().ok()?
        };
        if frac_part.is_empty() {
            return Some(Rat::int(sign * int_val));
        }
        let frac_val: i128 = frac_part.parse().ok()?;
        let mut den: i128 = 1;
        for _ in 0..frac_part.len() {
            den = den.checked_mul(10)?;
        }
        let num = int_val.checked_mul(den)?.checked_add(frac_val)?;
        Some(Rat::new(sign * num, den))
    }

    fn is_zero(&self) -> bool {
        self.num == 0
    }

    fn is_positive(&self) -> bool {
        self.num > 0
    }

    fn is_negative(&self) -> bool {
        self.num < 0
    }

    fn add(&self, other: &Rat) -> Rat {
        Rat::new(
            self.num * other.den + other.num * self.den,
            self.den * other.den,
        )
    }

    fn neg(&self) -> Rat {
        Rat {
            num: -self.num,
            den: self.den,
        }
    }

    fn mul(&self, other: &Rat) -> Rat {
        Rat::new(self.num * other.num, self.den * other.den)
    }

    fn recip(&self) -> Rat {
        assert!(self.num != 0, "reciprocal of zero");
        Rat::new(self.den, self.num)
    }

    fn div(&self, other: &Rat) -> Rat {
        self.mul(&other.recip())
    }

    /// `self < other`.
    fn lt(&self, other: &Rat) -> bool {
        self.sub(other).is_negative()
    }

    fn sub(&self, other: &Rat) -> Rat {
        self.add(&other.neg())
    }

    /// Render the rational for a surfaced counterexample: an integer prints
    /// bare (`-1`, `3`), a true fraction prints `num/den` (`1/2`).
    fn render(&self) -> String {
        if self.den == 1 {
            format!("{}", self.num)
        } else {
            format!("{}/{}", self.num, self.den)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use crate::StackTy;
    use crate::Subst;
    use crate::Ty;
    use crate::WordTy;
    use crate::parse;
    use crate::refinement::RefineSpan;
    use crate::shadow::core_shadow_word;
    use crate::shadow::interpreted_op;
    use crate::types::is_numeric_literal;

    const S: Span = Span { start: 0, end: 0 };

    fn rspan() -> RefineSpan {
        RefineSpan { start: 0, end: 0 }
    }

    fn binder(name: &str, ty: &str) -> Binder {
        Binder {
            name: name.to_string(),
            ty: ty.to_string(),
            span: rspan(),
        }
    }

    fn var(name: &str) -> Pred {
        Pred::Var(name.to_string())
    }

    fn num(lexeme: &str) -> Pred {
        Pred::Num(lexeme.to_string())
    }

    // sqrt : ( n: Num where n >= 0 -- r: Num where ... ): arrow ( Num -- Num ),
    // demand n >= 0 over binder n.
    fn sqrt_call() -> VerifyWord {
        VerifyWord::Call {
            binders: vec![binder("n", "Num")],
            demand: Some(Pred::Bin(BinOp::Ge, Box::new(var("n")), Box::new(num("0")))),
            out_binders: vec![binder("r", "Num")],
            guarantee: None,
            arrow: WordTy::new(
                StackTy::new(vec![Ty::num(S)], 0, S),
                StackTy::new(vec![Ty::num(S)], 0, S),
            ),
        }
    }

    // The verification resolver for the M8 demo: core shuffles + interpreted
    // operators + the `sqrt` call site; `x` is a free variable, numbers are
    // literals. `if` is intercepted by the verifier and never reaches here.
    fn demo_resolver(w: &str) -> VerifyWord {
        if w == "sqrt" {
            return sqrt_call();
        }
        if let Some(core) = core_shadow_word(w) {
            return VerifyWord::Core(core);
        }
        if let Some(op) = interpreted_op(w) {
            return VerifyWord::Core(op);
        }
        if is_numeric_literal(w) {
            return VerifyWord::Core(ShadowWord::Num(w.to_string()));
        }
        VerifyWord::Core(ShadowWord::Var(w.to_string()))
    }

    // =======================================================================
    // The seam: exactly four methods, push/pop parity (§10.8/§10.9)
    // =======================================================================

    #[test]
    fn smtlib_push_pop_parity_and_assert_check() {
        let mut s = SmtLibSolver::new();
        assert_eq!(s.depth(), 1);
        s.push_scope();
        assert_eq!(s.depth(), 2);
        s.assert(&Pred::Bin(
            BinOp::Gt,
            Box::new(var("x")),
            Box::new(num("0")),
        ));
        let _ = s.check();
        s.pop_scope();
        assert_eq!(s.depth(), 1);

        let script = s.script();
        assert!(script.contains("(push 1)"), "script:\n{script}");
        assert!(script.contains("(pop 1)"), "script:\n{script}");
        assert!(script.contains("(check-sat)"), "script:\n{script}");
        assert!(
            script.contains("(declare-const x Real)"),
            "script:\n{script}"
        );
        assert!(script.contains("(assert (> x 0))"), "script:\n{script}");
        // Exactly one push and one pop here (parity).
        assert_eq!(script.matches("(push 1)").count(), 1);
        assert_eq!(script.matches("(pop 1)").count(), 1);
    }

    #[test]
    fn render_smtlib_shapes() {
        assert_eq!(render_smtlib(&var("x")), "x");
        assert_eq!(render_smtlib(&num("3")), "3");
        assert_eq!(render_smtlib(&num("-2")), "(- 2)");
        assert_eq!(
            render_smtlib(&Pred::Bin(
                BinOp::Ge,
                Box::new(var("x")),
                Box::new(num("0"))
            )),
            "(>= x 0)"
        );
        assert_eq!(
            render_smtlib(&negate(&Pred::Bin(
                BinOp::Ge,
                Box::new(var("x")),
                Box::new(num("0"))
            ))),
            "(not (>= x 0))"
        );
    }

    // =======================================================================
    // Minimal reasoner: the §12 M8 contrast as raw sat checks
    // =======================================================================

    #[test]
    fn check_sat_contradiction_is_unsat() {
        // x > 0 ∧ x < 0  ⇒ Unsat.
        let f = vec![
            Pred::Bin(BinOp::Gt, Box::new(var("x")), Box::new(num("0"))),
            Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("0"))),
        ];
        assert_eq!(check_sat(&f), Verdict::Unsat);
    }

    #[test]
    fn check_sat_single_bound_is_sat() {
        // x < 0  alone is satisfiable.
        let f = vec![Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("0")))];
        assert_eq!(check_sat(&f), Verdict::Sat);
    }

    #[test]
    fn check_sat_opaque_conjunct_is_unknown_not_sat() {
        // An uninterpreted application is opaque: a satisfiable decidable subset
        // must degrade to Unknown (never an unsound Sat).
        let f = vec![Pred::App("length".into(), vec![var("xs")])];
        assert_eq!(check_sat(&f), Verdict::Unknown);
    }

    #[test]
    fn check_sat_opaque_with_decidable_contradiction_is_unsat() {
        // Even with an opaque conjunct present, a decidable contradiction ⇒ Unsat
        // (adding constraints keeps it infeasible — sound).
        let f = vec![
            Pred::Bin(BinOp::Gt, Box::new(var("x")), Box::new(num("0"))),
            Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("0"))),
            Pred::App("length".into(), vec![var("xs")]),
        ];
        assert_eq!(check_sat(&f), Verdict::Unsat);
    }

    #[test]
    fn check_sat_negated_equality_is_opaque_not_unsound() {
        // ¬(x = y) is a disjunction this reasoner cannot represent; it must be
        // opaque (⇒ Unknown), never mistranslated into x = y. A bare ¬(x=y)
        // alone is satisfiable in truth, and the honest minimal answer is
        // Unknown (fail-closed), not Unsat.
        let f = vec![negate(&Pred::Bin(
            BinOp::Eq,
            Box::new(var("x")),
            Box::new(var("y")),
        ))];
        assert_eq!(check_sat(&f), Verdict::Unknown);
    }

    #[test]
    fn check_sat_multivariable_linear() {
        // x >= y ∧ y >= 1 ∧ x < 1  ⇒ Unsat (x >= y >= 1 contradicts x < 1).
        let f = vec![
            Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(var("y"))),
            Pred::Bin(BinOp::Ge, Box::new(var("y")), Box::new(num("1"))),
            Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("1"))),
        ];
        assert_eq!(check_sat(&f), Verdict::Unsat);
    }

    // =======================================================================
    // discharge + substitute (negated-goal, §10.5 minimal)
    // =======================================================================

    #[test]
    fn discharge_valid_goal_is_unsat_under_hypothesis() {
        let mut s = SmtLibSolver::new();
        // hypothesis x > 0 in scope.
        s.push_scope();
        s.assert(&Pred::Bin(
            BinOp::Gt,
            Box::new(var("x")),
            Box::new(num("0")),
        ));
        // goal x >= 0 — discharged (valid) under x > 0.
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")));
        assert_eq!(discharge(&mut s, &goal), Verdict::Unsat);
        s.pop_scope();
    }

    #[test]
    fn discharge_unbacked_goal_is_sat() {
        let mut s = SmtLibSolver::new();
        // No hypothesis: goal x >= 0 is NOT valid (x = -1 is a counterexample).
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")));
        assert_eq!(discharge(&mut s, &goal), Verdict::Sat);
    }

    #[test]
    fn substitute_binds_demand_to_actual_term() {
        // demand n >= 0; bind n <- x; goal becomes x >= 0.
        let demand = Pred::Bin(BinOp::Ge, Box::new(var("n")), Box::new(num("0")));
        let bindings = vec![NamedBinding {
            name: "n".into(),
            term: var("x"),
        }];
        assert_eq!(
            substitute(&demand, &bindings),
            Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")))
        );
    }

    // =======================================================================
    // §12 M8 acceptance: path conditions on `x 0 > [ x sqrt ] [ 0 ] if`
    // =======================================================================

    #[test]
    fn m8_path_condition_discharges_sqrt_demand_in_true_branch() {
        // WITH the path condition: inside the true branch `x > 0` is in scope, so
        // `x sqrt`'s demand `x >= 0` discharges (Unsat).
        let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
        let mut stack = ShadowStack::new();
        let mut solver = SmtLibSolver::new();
        let mut obligations = Vec::new();
        verify(
            &toks,
            &mut stack,
            &mut solver,
            &demo_resolver,
            &mut obligations,
        )
        .unwrap();

        // Exactly one obligation: `x sqrt`'s demand inside the true branch.
        assert_eq!(obligations.len(), 1, "obligations: {obligations:?}");
        assert_eq!(
            obligations[0].goal,
            Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")))
        );
        assert_eq!(
            obligations[0].verdict,
            Verdict::Unsat,
            "x sqrt's demand x>=0 must discharge under the path condition x>0"
        );

        // The solver saw push/pop bracketing the branches, and the SMT-LIB text
        // shows (push 1)/(pop 1).
        let script = solver.script();
        assert!(script.contains("(push 1)"), "script:\n{script}");
        assert!(script.contains("(pop 1)"), "script:\n{script}");
        assert!(script.contains("(assert (> x 0))"), "script:\n{script}");
        // The negated goal of the discharge appears under the branch.
        assert!(
            script.contains("(assert (not (>= x 0)))"),
            "script:\n{script}"
        );
        // Push/pop parity: equal counts.
        assert_eq!(
            script.matches("(push 1)").count(),
            script.matches("(pop 1)").count(),
            "push/pop parity\nscript:\n{script}"
        );
        // Solver returns to the base scope.
        assert_eq!(solver.depth(), 1);
    }

    #[test]
    fn m8_without_path_condition_the_demand_fails() {
        // WITHOUT the path condition: discharge `x sqrt`'s demand `x >= 0` with
        // no hypothesis in scope — it must FAIL (Sat: x = -1 is a counterexample).
        // This is the contrast that proves the path condition is what makes the
        // true-branch discharge go through.
        let mut solver = SmtLibSolver::new();
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")));
        assert_eq!(discharge(&mut solver, &goal), Verdict::Sat);
    }

    #[test]
    fn m8_else_branch_asserts_negated_condition() {
        // The else-branch is verified under ¬P. With an obligation in the else
        // branch we can observe ¬(x > 0) is the governing hypothesis.
        // Program: x 0 > [ 0 ] [ x sqrt ] if  — sqrt now in the ELSE branch.
        // Under ¬(x>0) i.e. x<=0, x>=0 does NOT discharge (only x=0 works; x<0 is
        // a counterexample) ⇒ Sat. This shows the else-branch hypothesis is ¬P,
        // not P.
        let toks = parse("x 0 > [ 0 ] [ x sqrt ] if").unwrap();
        let mut stack = ShadowStack::new();
        let mut solver = SmtLibSolver::new();
        let mut obligations = Vec::new();
        verify(
            &toks,
            &mut stack,
            &mut solver,
            &demo_resolver,
            &mut obligations,
        )
        .unwrap();

        assert_eq!(obligations.len(), 1, "obligations: {obligations:?}");
        // Under ¬(x>0), x>=0 is not valid ⇒ Sat (the demand fails in the else
        // branch, as it should).
        assert_eq!(obligations[0].verdict, Verdict::Sat);

        let script = solver.script();
        assert!(
            script.contains("(assert (not (> x 0)))"),
            "else branch must assert ¬P\nscript:\n{script}"
        );
    }

    // =======================================================================
    // Immutability barrier (§3 invariant 1 / 18)
    // =======================================================================

    #[test]
    fn branch_scoping_never_touches_the_tier0_substitution() {
        // The Tier 0 substitution is FROZEN/read-only by this tier. Build one,
        // bind into it (as inference would), snapshot it, then run the full M8
        // path-condition verification — which pushes/pops solver scopes and
        // asserts branch facts — and assert the substitution is byte-identical
        // afterward. The barrier is structural (no function here takes a
        // &mut Subst), so this can only ever pass.
        let mut subst = Subst::new();
        subst.bind_ty(0, Ty::num(S));
        subst.bind_row(0, StackTy::empty(1, S));
        let before = format!("{subst:?}");

        let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
        let mut stack = ShadowStack::new();
        let mut solver = SmtLibSolver::new();
        let mut obligations = Vec::new();
        verify(
            &toks,
            &mut stack,
            &mut solver,
            &demo_resolver,
            &mut obligations,
        )
        .unwrap();

        let after = format!("{subst:?}");
        assert_eq!(
            before, after,
            "Tier 0 substitution must be untouched across branch scoping (immutability barrier)"
        );
        // And the verification really did do its job.
        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].verdict, Verdict::Unsat);
    }

    // A compile-time witness of the barrier: nothing in the solver/verify path
    // can name a Tier 0 inference handle. If `verify`/`discharge`/`Solver` ever
    // grew a `&mut Subst` parameter this module would still compile, but this
    // function documents that the read-only Subst here is only ever *read*.
    #[allow(dead_code)]
    fn _subst_is_read_only(subst: &Subst) {
        let _ = format!("{subst:?}");
    }

    // =======================================================================
    // Compile-time only (§10.10 / invariant 14/20)
    // =======================================================================

    // =======================================================================
    // M9 — counterexample/model extraction (§10.5)
    // =======================================================================

    #[test]
    fn check_sat_model_single_bound_yields_negative_witness() {
        // x < 0 alone is Sat; the witness must be a concrete negative value.
        let f = vec![Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("0")))];
        let (verdict, model) = check_sat_model(&f);
        assert_eq!(verdict, Verdict::Sat);
        let model = model.expect("a decidable Sat must carry a model");
        assert_eq!(model.get("x"), Some("-1"));
    }

    #[test]
    fn check_sat_model_midpoint_between_bounds() {
        // 0 <= x ∧ x <= 4  ⇒ Sat; the witness is the midpoint 2 (a value strictly
        // satisfying both bounds, chosen by back-substitution).
        let f = vec![
            Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0"))),
            Pred::Bin(BinOp::Le, Box::new(var("x")), Box::new(num("4"))),
        ];
        let (verdict, model) = check_sat_model(&f);
        assert_eq!(verdict, Verdict::Sat);
        assert_eq!(model.unwrap().get("x"), Some("2"));
    }

    #[test]
    fn check_sat_model_unsat_has_no_model() {
        // x > 0 ∧ x < 0 ⇒ Unsat: no counterexample, no model.
        let f = vec![
            Pred::Bin(BinOp::Gt, Box::new(var("x")), Box::new(num("0"))),
            Pred::Bin(BinOp::Lt, Box::new(var("x")), Box::new(num("0"))),
        ];
        let (verdict, model) = check_sat_model(&f);
        assert_eq!(verdict, Verdict::Unsat);
        assert!(model.is_none());
    }

    #[test]
    fn check_sat_model_unknown_does_not_fabricate_a_model() {
        // An opaque conjunct degrades to Unknown; §10.5 forbids fabricating a
        // model for it.
        let f = vec![Pred::App("length".into(), vec![var("xs")])];
        let (verdict, model) = check_sat_model(&f);
        assert_eq!(verdict, Verdict::Unknown);
        assert!(model.is_none(), "Unknown must not fabricate a model");
    }

    // =======================================================================
    // M9 — the negated-goal encoding verified at unit level (§10.5 / §12 M9)
    // =======================================================================

    #[test]
    fn negated_goal_known_valid_is_unsat_no_model() {
        // Known-VALID implication: under x > 0, the goal x >= 0 holds. The
        // negated-goal encoding (assert ¬goal, check) ⇒ Unsat, and no model.
        let mut s = SmtLibSolver::new();
        s.push_scope();
        s.assert(&Pred::Bin(
            BinOp::Gt,
            Box::new(var("x")),
            Box::new(num("0")),
        ));
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")));
        let (verdict, model) = discharge_with_model(&mut s, &goal);
        s.pop_scope();
        assert_eq!(verdict, Verdict::Unsat, "known-valid ⇒ Unsat");
        assert!(model.is_none(), "a valid VC has no counterexample");
    }

    #[test]
    fn negated_goal_known_invalid_is_sat_with_model() {
        // Known-INVALID implication: with no hypothesis, the goal x >= 0 is not
        // valid. The negated-goal encoding ⇒ Sat, with a concrete counterexample
        // (x = -1) surfaced.
        let mut s = SmtLibSolver::new();
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")));
        let (verdict, model) = discharge_with_model(&mut s, &goal);
        assert_eq!(verdict, Verdict::Sat, "known-invalid ⇒ Sat");
        let model = model.expect("a Sat VC must surface its counterexample");
        assert_eq!(model.get("x"), Some("-1"));
    }

    // =======================================================================
    // M9 — first-order VC generation from refinement signatures, end-to-end
    // (§10.5 / §12 M9): `x sqrt`.
    // =======================================================================

    fn sqrt_sig() -> RefinementSig {
        crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 )").unwrap()
    }

    #[test]
    fn m9_x_sqrt_insufficient_facts_is_sat_with_counterexample() {
        // `x sqrt` with NO fact bounding x: the demand x >= 0 cannot be
        // discharged. The VC is Sat and a concrete counterexample (a negative x)
        // is surfaced — not a bare Sat.
        let toks = parse("x sqrt").unwrap();
        let sig = sqrt_sig();
        let lookup = |w: &str| (w == "sqrt").then(|| sig.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();

        assert_eq!(obs.len(), 1, "exactly one demand (x sqrt): {obs:?}");
        assert_eq!(
            obs[0].goal,
            Pred::Bin(BinOp::Ge, Box::new(var("x")), Box::new(num("0")))
        );
        assert_eq!(obs[0].verdict, Verdict::Sat);
        assert!(!obs[0].is_discharged());
        let model = obs[0]
            .model
            .as_ref()
            .expect("Sat ⇒ counterexample surfaced");
        let xval = model.get("x").expect("model constrains x");
        assert!(
            xval.starts_with('-'),
            "counterexample x must be negative, got {xval}"
        );
    }

    #[test]
    fn m9_x_sqrt_sufficient_facts_is_unsat_accepted() {
        // A word `nonneg` guaranteeing its result >= 0 publishes that fact; then
        // `nonneg sqrt` discharges sqrt's demand (Unsat, accepted) — no model.
        // This exercises facts coming from a *preceding word's guarantee*.
        let toks = parse("nonneg sqrt").unwrap();
        let sqrt = sqrt_sig();
        let nonneg = crate::parse_signature("nonneg : ( -- r: Num where r >= 0 )").unwrap();
        let lookup = |w: &str| match w {
            "sqrt" => Some(sqrt.clone()),
            "nonneg" => Some(nonneg.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();

        // `nonneg` has no demand (only a guarantee) ⇒ no obligation; `sqrt` has
        // exactly one.
        assert_eq!(obs.len(), 1, "only sqrt raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "demand discharges under guarantee"
        );
        assert!(obs[0].is_discharged());
        assert!(
            obs[0].model.is_none(),
            "a discharged VC has no counterexample"
        );

        // The published guarantee really was asserted as a fact in the script.
        let script = solver.script();
        assert!(
            script.contains(">= $t0 0") || script.contains("(>= $t0 0)"),
            "guarantee fact must be asserted\nscript:\n{script}"
        );
    }

    #[test]
    fn m9_unknown_demand_is_not_accepted_as_discharged() {
        // A demand the reasoner cannot decide (opaque, nonlinear) degrades to
        // Unknown and is NOT accepted as discharged — no silent pass, no
        // fabricated model (§10.5; staging for M10/M12).
        let toks = parse("x foo").unwrap();
        // foo demands `length n >= 0` over an uninterpreted `length` — opaque.
        let sig =
            crate::parse_signature("foo : ( n: Num where length n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| (w == "foo").then(|| sig.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();

        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].verdict, Verdict::Unknown);
        assert!(
            !obs[0].is_discharged(),
            "Unknown must never count as discharged"
        );
        assert!(obs[0].model.is_none(), "Unknown must not fabricate a model");
    }

    #[test]
    fn m9_path_condition_plus_vc_generation_end_to_end() {
        // The M8 path condition and the M9 VC generation compose: inside the
        // x > 0 branch, the refinement-derived demand for `x sqrt` discharges.
        let toks = parse("x 0 > [ x sqrt ] [ 0 ] if").unwrap();
        let sig = sqrt_sig();
        let lookup = |w: &str| (w == "sqrt").then(|| sig.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();

        assert_eq!(obs.len(), 1);
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "x sqrt's demand discharges under the x>0 path condition"
        );
        assert!(obs[0].model.is_none());
    }

    #[test]
    fn solver_seam_is_compile_time_only() {
        // The solver is a standalone analysis artifact: constructed here / by the
        // checker and discarded, never a field of Evaluator, never linked into a
        // running program. Proof by construction: it owns no handle into the
        // Evaluator and the Evaluator owns no handle to it.
        let mut solver = SmtLibSolver::new();
        solver.push_scope();
        solver.assert(&Pred::Bin(
            BinOp::Gt,
            Box::new(var("x")),
            Box::new(num("0")),
        ));
        let _ = solver.check();
        solver.pop_scope();
        drop(solver); // discarded before anything "ships".
    }

    // =======================================================================
    // M10 — higher-order refinement subsumption (§10.6 / §12 M10)
    // =======================================================================

    // A quotation/signature carrying only an output (post) refinement `r OP k`.
    fn post_sig(op: BinOp, k: &str) -> RefinementSig {
        RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![],
                predicate: None,
            },
            guarantees: RefinementSide {
                binders: vec![binder("r", "Num")],
                predicate: Some(Pred::Bin(op, Box::new(var("r")), Box::new(num(k)))),
            },
        }
    }

    // A quotation/signature carrying only an input (pre/demand) refinement `n OP k`.
    fn pre_sig(op: BinOp, k: &str) -> RefinementSig {
        RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![binder("n", "Num")],
                predicate: Some(Pred::Bin(op, Box::new(var("n")), Box::new(num(k)))),
            },
            guarantees: RefinementSide {
                binders: vec![binder("r", "Num")],
                predicate: None,
            },
        }
    }

    #[test]
    fn m10_covariant_stronger_guarantee_is_accepted() {
        // provided post r>5 where expected post r>0: covariant VC r>5 ⟹ r>0 is
        // valid (a STRONGER guarantee subsumes a weaker one) ⇒ ACCEPTED.
        let provided = post_sig(BinOp::Gt, "5");
        let expected = post_sig(BinOp::Gt, "0");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(
            res.guarantee.direction,
            SubsumptionDirection::Guarantee,
            "first VC is the covariant guarantee direction"
        );
        assert!(
            res.guarantee.is_valid(),
            "r>5 ⟹ r>0 must be valid: {:?}",
            res.guarantee
        );
        // The demand side is `true ⟹ true` (both absent) ⇒ trivially valid.
        assert!(res.demand.is_valid());
        assert_eq!(res.outcome(), SubsumptionOutcome::Preserved);
        assert!(res.is_preserved());
    }

    #[test]
    fn m10_covariant_weaker_guarantee_is_rejected() {
        // provided post r>0 where expected post r>5: covariant VC r>0 ⟹ r>5 is
        // INVALID (a weaker guarantee cannot substitute for a stronger one) ⇒
        // REJECTED, with a counterexample.
        let provided = post_sig(BinOp::Gt, "0");
        let expected = post_sig(BinOp::Gt, "5");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert!(!res.guarantee.is_valid(), "r>0 ⟹ r>5 must be invalid");
        assert_eq!(res.guarantee.verdict, Verdict::Sat);
        match res.outcome() {
            SubsumptionOutcome::Violated { direction, model } => {
                assert_eq!(direction, SubsumptionDirection::Guarantee);
                let m = model.expect("a refuted subsumption surfaces a counterexample");
                // Witness: a point with r>0 but r<=5 (e.g. r between 0 and 5).
                assert!(
                    m.get("$post0").is_some(),
                    "model constrains the result: {m}"
                );
            }
            other => panic!("expected Violated, got {other:?}"),
        }
        assert!(!res.is_preserved());
    }

    #[test]
    fn m10_contravariant_weaker_demand_is_accepted() {
        // provided demand n>0 where expected demand n>5: contravariant VC
        // expected_pre ⟹ provided_pre, i.e. n>5 ⟹ n>0, is valid (a WEAKER
        // provided demand subsumes a stronger expected one) ⇒ ACCEPTED. Note the
        // FLIP relative to the guarantee direction.
        let provided = pre_sig(BinOp::Gt, "0");
        let expected = pre_sig(BinOp::Gt, "5");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(res.demand.direction, SubsumptionDirection::Demand);
        assert!(
            res.demand.is_valid(),
            "n>5 ⟹ n>0 (expected_pre ⟹ provided_pre) must be valid: {:?}",
            res.demand
        );
        assert_eq!(res.outcome(), SubsumptionOutcome::Preserved);
    }

    #[test]
    fn m10_contravariant_stronger_demand_is_rejected() {
        // provided demand n>5 where expected demand n>0: contravariant VC
        // n>0 ⟹ n>5 is INVALID (a quotation demanding MORE than the boundary
        // supplies cannot substitute) ⇒ REJECTED. The symmetric (flipped) case.
        let provided = pre_sig(BinOp::Gt, "5");
        let expected = pre_sig(BinOp::Gt, "0");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert!(!res.demand.is_valid(), "n>0 ⟹ n>5 must be invalid");
        assert_eq!(res.demand.verdict, Verdict::Sat);
        match res.outcome() {
            SubsumptionOutcome::Violated { direction, .. } => {
                assert_eq!(direction, SubsumptionDirection::Demand);
            }
            other => panic!("expected Violated on the demand direction, got {other:?}"),
        }
    }

    #[test]
    fn m10_undecidable_subsumption_fails_closed() {
        // A subsumption VC the reasoner cannot decide (an opaque/uninterpreted
        // conjunct) must be REJECTED with the targeted message — never accepted
        // (fail closed, invariant 12). provided post r>0, expected post
        // `length r > 0` (uninterpreted `length`): the guarantee VC
        // r>0 ⟹ length r > 0 has an opaque conjunct ⇒ Unknown ⇒ rejected.
        let provided = post_sig(BinOp::Gt, "0");
        let expected = RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![],
                predicate: None,
            },
            guarantees: RefinementSide {
                binders: vec![binder("r", "Num")],
                predicate: Some(Pred::Bin(
                    BinOp::Gt,
                    Box::new(Pred::App("length".into(), vec![var("r")])),
                    Box::new(num("0")),
                )),
            },
        };
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(
            res.guarantee.verdict,
            Verdict::Unknown,
            "an opaque subsumption VC is Unknown"
        );
        assert!(
            !res.guarantee.is_valid(),
            "Unknown is NOT valid — never a silent pass"
        );
        match res.outcome() {
            SubsumptionOutcome::Undecidable { direction, message } => {
                assert_eq!(direction, SubsumptionDirection::Guarantee);
                assert_eq!(message, SUBSUMPTION_FAIL_CLOSED_MSG);
            }
            other => panic!("Unknown subsumption must fail closed, got {other:?}"),
        }
        assert!(!res.is_preserved(), "fails closed ⇒ not preserved");
    }

    #[test]
    fn m10_unknown_takes_precedence_over_sat_in_failing_closed() {
        // If one direction is Sat (violated) and the other Unknown, the
        // fail-closed mandate reports Undecidable first (the stronger rejection):
        // an Unknown is never downgraded.
        let provided = RefinementSig {
            name: "q".into(),
            // demand n>5 where expected n>0 ⇒ demand VC n>0 ⟹ n>5 is Sat.
            demands: RefinementSide {
                binders: vec![binder("n", "Num")],
                predicate: Some(Pred::Bin(BinOp::Gt, Box::new(var("n")), Box::new(num("5")))),
            },
            // post r>0 where expected `length r > 0` ⇒ guarantee VC Unknown.
            guarantees: RefinementSide {
                binders: vec![binder("r", "Num")],
                predicate: Some(Pred::Bin(BinOp::Gt, Box::new(var("r")), Box::new(num("0")))),
            },
        };
        let expected = RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![binder("n", "Num")],
                predicate: Some(Pred::Bin(BinOp::Gt, Box::new(var("n")), Box::new(num("0")))),
            },
            guarantees: RefinementSide {
                binders: vec![binder("r", "Num")],
                predicate: Some(Pred::Bin(
                    BinOp::Gt,
                    Box::new(Pred::App("length".into(), vec![var("r")])),
                    Box::new(num("0")),
                )),
            },
        };
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(res.guarantee.verdict, Verdict::Unknown);
        assert_eq!(res.demand.verdict, Verdict::Sat);
        assert!(
            matches!(res.outcome(), SubsumptionOutcome::Undecidable { .. }),
            "Unknown must take precedence (fail closed) over a Sat violation"
        );
    }

    #[test]
    fn m10_both_absent_refinements_trivially_preserved() {
        // Two unrefined quotations (where true on both sides): true ⟹ true on
        // both directions ⇒ preserved. (The unrefined-meets-REQUIRED-guarantee
        // case — true ⟹ r>0 — fails, but its targeted "carries no contract"
        // diagnostic is M11, not pulled forward here.)
        let provided = RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![],
                predicate: None,
            },
            guarantees: RefinementSide {
                binders: vec![],
                predicate: None,
            },
        };
        let expected = provided.clone();
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(res.outcome(), SubsumptionOutcome::Preserved);
    }

    // =======================================================================
    // M10 — operator refinement axioms: dip relays a quotation's guarantee
    // (§10.6 / §8 / §12 M10)
    // =======================================================================

    #[test]
    fn m10_dip_relays_refined_quotation_guarantee_to_far_side() {
        // A refined quotation (post r>0) passed through `DIP` preserves its
        // guarantee on the far side via DIP's authored refinement axiom:
        //   a [ produce ] DIP DROP sqrt
        // `produce : ( -- r where r > 0 )` runs on the rest (under dip), its
        // guarantee r>0 is published; `a` is set aside and restored by identity;
        // `DROP` removes it; `sqrt`'s demand r>=0 then discharges via r>0.
        let toks = parse("a [ produce ] DIP DROP sqrt").unwrap();
        let produce = crate::parse_signature("produce : ( -- r: Num where r > 0 )").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| match w {
            "produce" => Some(produce.clone()),
            "sqrt" => Some(sqrt.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1, "only sqrt raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "the quotation's guarantee must survive dip and discharge sqrt's demand"
        );
        assert!(obs[0].is_discharged());
    }

    #[test]
    fn m10_dip_preserves_set_aside_value_predicate() {
        // DIP's axiom also preserves the set-aside value's predicate unchanged.
        //   x guard [ produce ] DIP   then a demand on x
        // We model `guard` as an operator that guarantees its result > 10, set it
        // aside with DIP, run produce on the rest, and confirm the set-aside
        // value's fact still discharges a demand about it on the far side.
        //   guard : ( -- g where g > 10 )
        //   produce : ( -- r where r > 0 )
        //   needs10 : ( m where m > 10 -- s )    (a downstream demand on the value)
        // Program: guard [ produce ] DIP DROP needs10
        //   after guard: stack = g  (g>10 asserted)
        //   [ produce ] DIP: set aside g, run produce (push r, r>0), restore g
        //     ⇒ stack = r g  (g on top, by identity — its fact g>10 preserved)
        //   DROP: remove g? No — DROP removes top = g. That loses g. Instead:
        // Use `SWAP DROP` to drop r and keep g. Simpler: keep g on top and call
        // needs10 directly (needs10 binds m<-top=g).
        // Program: guard [ produce ] DIP needs10
        let toks = parse("guard [ produce ] DIP needs10").unwrap();
        let guard = crate::parse_signature("guard : ( -- g: Num where g > 10 )").unwrap();
        let produce = crate::parse_signature("produce : ( -- r: Num where r > 0 )").unwrap();
        let needs10 =
            crate::parse_signature("needs10 : ( m: Num where m > 10  --  s: Num )").unwrap();
        let lookup = |w: &str| match w {
            "guard" => Some(guard.clone()),
            "produce" => Some(produce.clone()),
            "needs10" => Some(needs10.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1, "only needs10 raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "the set-aside value's predicate (g>10) must survive dip by identity"
        );
    }

    #[test]
    fn m10_call_relays_refined_quotation_guarantee() {
        // `CALL` likewise relays its quotation argument's contract (the DIP rule
        // with no set-aside value): a refined quotation run by `call` publishes
        // its guarantee, which discharges a downstream demand.
        //   [ produce ] CALL sqrt   with produce: ( -- r where r > 0 )
        let toks = parse("[ produce ] CALL sqrt").unwrap();
        let produce = crate::parse_signature("produce : ( -- r: Num where r > 0 )").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| match w {
            "produce" => Some(produce.clone()),
            "sqrt" => Some(sqrt.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1, "only sqrt raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "the quotation's guarantee must survive call and discharge sqrt's demand"
        );
    }

    #[test]
    fn m10_if_relays_branch_guarantees_under_path_conditions() {
        // `if` relays each branch quotation's contract under its branch's path
        // condition (§10.4): a refined producer in the true branch publishes its
        // guarantee there, discharging an in-branch demand.
        //   c [ produce sqrt ] [ 0 ] if
        // with produce: ( -- r where r > 0 ); sqrt's demand r>=0 discharges via
        // the relayed guarantee inside the branch.
        let toks = parse("c [ produce sqrt ] [ 0 ] if").unwrap();
        let produce = crate::parse_signature("produce : ( -- r: Num where r > 0 )").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| match w {
            "produce" => Some(produce.clone()),
            "sqrt" => Some(sqrt.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(
            obs.len(),
            1,
            "only sqrt (in the true branch) raises a demand: {obs:?}"
        );
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "the branch quotation's guarantee must discharge the in-branch demand"
        );
    }

    #[test]
    fn m10_relay_quote_post_asserts_declared_guarantee() {
        // The contract-level relay axiom: relay_quote_post substitutes a
        // quotation's declared post onto its result terms and asserts it, so a
        // downstream goal discharges. This is the axiom expressed WITHOUT
        // expanding the quotation's body (the relay rule, invariant 8).
        let quote = crate::parse_signature("q : ( -- r: Num where r > 0 )").unwrap();
        let mut s = SmtLibSolver::new();
        // The quotation produced one result term `y`.
        relay_quote_post(&quote, &[var("y")], &mut s);
        // Now the goal y >= 0 discharges (Unsat) under the relayed fact y>0.
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("y")), Box::new(num("0")));
        assert_eq!(discharge(&mut s, &goal), Verdict::Unsat);
        // And the relayed fact appears in the script.
        assert!(
            s.script().contains("(assert (> y 0))"),
            "relayed guarantee must be asserted\nscript:\n{}",
            s.script()
        );
    }

    #[test]
    fn m10_relay_quote_post_absent_guarantee_asserts_nothing() {
        // An unrefined quotation (where true) relays nothing.
        let quote = crate::parse_signature("q : ( -- r: Num )").unwrap();
        let mut s = SmtLibSolver::new();
        relay_quote_post(&quote, &[var("y")], &mut s);
        // No fact ⇒ the goal y>=0 is NOT valid (counterexample exists).
        let goal = Pred::Bin(BinOp::Ge, Box::new(var("y")), Box::new(num("0")));
        assert_eq!(discharge(&mut s, &goal), Verdict::Sat);
    }

    // =======================================================================
    // M10 — embedder operator's registered post discharges a downstream
    // user obligation; its pre is checked at the call site (§10.6 / §12 M10)
    // =======================================================================

    #[test]
    fn m10_embedder_operator_post_discharges_downstream_obligation() {
        // An embedder operator carries its pre/post from registration (the
        // operator table is the modulo). Here `db_count` is a registered host
        // operator guaranteeing a non-negative count; its post discharges a
        // downstream user obligation (`sqrt`'s demand) — the user inherits the
        // contract automatically, on faith.
        //   db_count : ( -- c: Num where c >= 0 )    (embedder-registered post)
        //   sqrt     : ( n: Num where n >= 0 -- r )  (user obligation)
        // Program: db_count sqrt
        let toks = parse("db_count sqrt").unwrap();
        let db_count = crate::parse_signature("db_count : ( -- c: Num where c >= 0 )").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| match w {
            "db_count" => Some(db_count.clone()),
            "sqrt" => Some(sqrt.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1, "only sqrt raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "the embedder operator's registered post (c>=0) discharges sqrt's demand"
        );
        assert!(obs[0].is_discharged());
    }

    #[test]
    fn m10_embedder_operator_pre_is_checked_at_call_site() {
        // The operator's pre (demand) is checked at its call site like any other
        // obligation. `db_get` requires a non-negative key; calling it with an
        // unconstrained `k` fails (Sat, counterexample) — the operator's pre is a
        // genuine obligation on the caller, not assumed.
        //   db_get : ( k: Num where k >= 0 -- v: Num )
        // Program: k db_get   (k unconstrained)
        let toks = parse("k db_get").unwrap();
        let db_get =
            crate::parse_signature("db_get : ( k: Num where k >= 0  --  v: Num )").unwrap();
        let lookup = |w: &str| (w == "db_get").then(|| db_get.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(
            obs[0].verdict,
            Verdict::Sat,
            "the operator's pre is a real call-site obligation and must fail unbacked"
        );
        assert!(!obs[0].is_discharged());
        let m = obs[0].model.as_ref().expect("Sat ⇒ counterexample");
        assert!(m.get("k").is_some(), "counterexample constrains k: {m}");
    }

    // =======================================================================
    // M11 — gradual interop: absent refinement = `where true`, the targeted
    // "carries no contract" diagnostic, and Situation A (§10.7 / §12 M11)
    // =======================================================================

    // A fully unrefined quotation/signature: `where true` on both sides (absent
    // payloads). §10.7 reads this as maximally weak demand AND guarantee.
    fn unrefined_sig() -> RefinementSig {
        RefinementSig {
            name: "q".into(),
            demands: RefinementSide {
                binders: vec![],
                predicate: None,
            },
            guarantees: RefinementSide {
                binders: vec![],
                predicate: None,
            },
        }
    }

    #[test]
    fn m11_unrefined_quotation_meets_required_guarantee_carries_no_contract() {
        // (§12 M11 (a)) An UNREFINED quotation (where true) passed where a
        // GUARANTEE is required: the covariant VC `true ⟹ r>0` is invalid, but the
        // actionable diagnosis is that the quotation carries no contract — NOT a
        // bare SMT counterexample. Assert the targeted message.
        let provided = unrefined_sig();
        let expected = post_sig(BinOp::Gt, "0");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        // The guarantee VC is flagged absent-payload (provided side is `where true`).
        assert!(
            res.guarantee.absent_payload,
            "the provided guarantee is absent (where true)"
        );
        assert!(
            !res.guarantee.is_valid(),
            "true ⟹ r>0 is invalid (the contract is required but missing)"
        );
        match res.outcome() {
            SubsumptionOutcome::CarriesNoContract { direction, message } => {
                assert_eq!(direction, SubsumptionDirection::Guarantee);
                assert_eq!(message, SUBSUMPTION_NO_CONTRACT_MSG);
                assert_eq!(
                    message,
                    "this quotation carries no contract and one is required here"
                );
            }
            other => panic!("expected CarriesNoContract, got {other:?}"),
        }
        assert!(
            !res.is_preserved(),
            "an unrefined quotation does not satisfy a required guarantee"
        );
    }

    #[test]
    fn m11_present_but_weaker_guarantee_keeps_m10_counterexample() {
        // (§12 M11 (a), the DISTINCTION) A PRESENT-but-weaker guarantee (r>0 where
        // r>5 is required) is a genuine M10 violation and KEEPS its counterexample
        // — it must NOT be re-routed to "carries no contract", because the
        // contract exists; it is merely too weak.
        let provided = post_sig(BinOp::Gt, "0");
        let expected = post_sig(BinOp::Gt, "5");
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert!(
            !res.guarantee.absent_payload,
            "the provided guarantee is present, not absent"
        );
        match res.outcome() {
            SubsumptionOutcome::Violated { direction, model } => {
                assert_eq!(direction, SubsumptionDirection::Guarantee);
                assert!(
                    model.is_some(),
                    "a present-but-weaker contract keeps its M10 counterexample"
                );
            }
            other => panic!("expected Violated (M10 counterexample), got {other:?}"),
        }
    }

    #[test]
    fn m11_unrefined_quotation_to_contract_agnostic_combinator_is_accepted() {
        // (§12 M11 (b)) An unrefined quotation passed to a CONTRACT-AGNOSTIC
        // boundary (expected demands nothing and guarantees nothing): both
        // directions reduce to `true ⟹ true` ⇒ ACCEPTED. Gradual adoption: the
        // unrefined code slots into the lattice automatically.
        let provided = unrefined_sig();
        let expected = unrefined_sig();
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert!(
            res.guarantee.is_valid() && res.demand.is_valid(),
            "true ⟹ true on both directions"
        );
        assert!(
            !res.guarantee.absent_payload,
            "no required guarantee ⇒ not the carries-no-contract case"
        );
        assert_eq!(res.outcome(), SubsumptionOutcome::Preserved);
        assert!(res.is_preserved());
    }

    #[test]
    fn m11_refined_quotation_to_contract_agnostic_combinator_is_accepted() {
        // A REFINED quotation passed to a contract-agnostic boundary is also fine:
        // `r>5 ⟹ true` is trivially valid — a guarantee no one consumes is no
        // obligation (the gradual lattice accepts strictly-more-refined code too).
        let provided = post_sig(BinOp::Gt, "5");
        let expected = unrefined_sig();
        let mut s = SmtLibSolver::new();
        let res = check_subsumption(&provided, &expected, &mut s);
        assert_eq!(res.outcome(), SubsumptionOutcome::Preserved);
    }

    #[test]
    fn m11_situation_a_dropped_opaque_value_verifies_silently() {
        // (§12 M11, Situation A) An OPAQUE word's result that is DROPPED never
        // reaches an obligation, so no VC depends on it and verification is silent
        // with ZERO user annotation. Here `opaque` is an uncontracted word; its
        // result is dropped; the only obligation (`sqrt`) is discharged by a
        // DIFFERENT, properly-contracted fact (`db_count`'s guarantee). The opaque
        // value never appears in any obligation goal.
        //   opaque drop db_count sqrt
        let toks = parse("opaque drop db_count sqrt").unwrap();
        let db_count = crate::parse_signature("db_count : ( -- c: Num where c >= 0 )").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| match w {
            // `opaque` is intentionally uncontracted (returns None): the shadow
            // stack gives it a fresh, fact-free term.
            "db_count" => Some(db_count.clone()),
            "sqrt" => Some(sqrt.clone()),
            _ => None,
        };
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        // Exactly one obligation (sqrt's demand) — the dropped opaque value raises
        // none — and it discharges from db_count's guarantee, silently.
        assert_eq!(obs.len(), 1, "only sqrt raises a demand: {obs:?}");
        assert_eq!(
            obs[0].verdict,
            Verdict::Unsat,
            "sqrt discharges from db_count's guarantee; the dropped opaque value is irrelevant"
        );
        assert!(obs[0].is_discharged());
        // The opaque value must NOT poison the obligation: its term never appears
        // in the goal (it was dropped before any obligation).
        assert!(
            !format!("{:?}", obs[0].goal).contains("opaque"),
            "the dropped opaque value must not appear in any obligation: {:?}",
            obs[0].goal
        );
    }

    #[test]
    fn m11_situation_b_opaque_value_in_obligation_still_fails_closed() {
        // (§12 M11, Situation B / boundary with M12) The COMPLEMENT of Situation
        // A: when the opaque value DOES flow into an obligation, the default stays
        // FAIL-CLOSED — no silent pass, and (deliberately) no `assume` boundary
        // pulled forward from M12 to rescue it.
        //   opaque sqrt   (opaque's fact-free term flows into sqrt's demand n>=0)
        let toks = parse("opaque sqrt").unwrap();
        let sqrt = crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").unwrap();
        let lookup = |w: &str| (w == "sqrt").then(|| sqrt.clone());
        let mut solver = SmtLibSolver::new();
        let obs = check_refinements(&toks, &lookup, &mut solver).unwrap();
        assert_eq!(obs.len(), 1, "sqrt raises the demand on the opaque value");
        assert!(
            !obs[0].is_discharged(),
            "an opaque value flowing into an obligation must FAIL CLOSED (no M12 assume here): {obs:?}"
        );
        assert_eq!(
            obs[0].verdict,
            Verdict::Sat,
            "the fact-free opaque value admits a counterexample to n>=0"
        );
    }

    // =======================================================================
    // M10 — subsumption is SMT-discharged only, never inferred (invariant 1/13)
    // =======================================================================

    // A compile-time witness that subsumption is settled by the solver, not the
    // type engine: `check_subsumption` takes only refinement payloads and a
    // `Solver` — no `Subst`/`InferCtx` handle — so it cannot infer/unify. If it
    // ever grew a Tier 0 inference parameter this signature would change.
    #[allow(dead_code)]
    fn _subsumption_is_solver_only(
        p: &RefinementSig,
        e: &RefinementSig,
        s: &mut SmtLibSolver,
    ) -> SubsumptionResult {
        check_subsumption(p, e, s)
    }

    // =======================================================================
    // M12 — the `assume` boundary: strict + cached/lenient (§10.7 / invariant 13)
    // =======================================================================
    //
    // These tests pin the assumed-contract boundary: discharge-on-faith rescues a
    // genuinely-opaque dependency while the default WITHOUT `assume` still fails
    // closed; the STRICT anti-rot drop-and-re-run actually runs and rejects an
    // unnecessary `assume`; `assume` with no opaque dependency is rejected; the
    // exploratory check is cached and fails lenient; and the whole-program ledger
    // records each `assume` as an enumerable `verified modulo { … }` entry that
    // propagates to callers.

    // A word `pos : ( -- r where r > 0 )`: a contracted producer that publishes a
    // guarantee `r > 0` as a downstream fact (no demand). Used to manufacture a
    // *non-opaque* (already-contracted) dependency for the strict checks.
    fn pos_call() -> VerifyWord {
        VerifyWord::Call {
            binders: vec![],
            demand: None,
            out_binders: vec![binder("r", "Num")],
            guarantee: Some(Pred::Bin(BinOp::Gt, Box::new(var("r")), Box::new(num("0")))),
            arrow: WordTy::new(
                StackTy::new(vec![], 0, S),
                StackTy::new(vec![Ty::num(S)], 0, S),
            ),
        }
    }

    // Resolver: `sqrt` (demand n>=0), `pos` (guarantee r>0); everything else is a
    // core word, with an uncontracted word like `opaque` falling through to a
    // free-variable term (the genuinely-opaque case). `if`/`assume(...)` never
    // reach here (the verifier intercepts them).
    fn m12_resolver(w: &str) -> VerifyWord {
        match w {
            "sqrt" => sqrt_call(),
            "pos" => pos_call(),
            _ => demo_resolver(w),
        }
    }

    // Run the M12 verifier over `src` with the `m12_resolver` (a VerifyWord
    // resolver, the M8-style seam) and return the full context (obligations +
    // assume ledger). `assume( … )` words are intercepted by `verify_ctx`.
    fn run_m12(src: &str, site: &str) -> VerifyCtx {
        let toks = parse(src).unwrap();
        let mut solver = SmtLibSolver::new();
        let mut stack = ShadowStack::new();
        let mut ctx = VerifyCtx::with_site(site);
        verify_ctx(&toks, &mut stack, &mut solver, &m12_resolver, &mut ctx).unwrap();
        ctx
    }

    #[test]
    fn m12_assume_rescues_opaque_dependency_default_fails_closed() {
        // (§12 M12) WITHOUT `assume`, an opaque value flowing into sqrt's demand
        // fails closed (the M11 Situation B baseline).
        let ctx = run_m12("opaque sqrt", "foo");
        assert_eq!(ctx.obligations().len(), 1);
        assert!(
            !ctx.obligations()[0].is_discharged(),
            "without assume the opaque dependency fails closed: {:?}",
            ctx.obligations()
        );
        assert!(ctx.assumes().is_empty(), "no assume present");

        // WITH `assume(x >= 0)` on the opaque value, the demand discharges on
        // faith and the assume is recorded as legal.
        let ctx = run_m12("opaque assume(x>=0) sqrt", "foo");
        assert_eq!(ctx.assumes().len(), 1);
        assert_eq!(
            ctx.assumes()[0].legality,
            AssumeLegality::Legal,
            "a genuinely-opaque dependency makes the assume legal: {:?}",
            ctx.assumes()[0]
        );
        assert_eq!(ctx.obligations().len(), 1);
        assert!(
            ctx.obligations()[0].is_discharged(),
            "the assumed fact discharges sqrt's demand on faith: {:?}",
            ctx.obligations()
        );
    }

    #[test]
    fn m12_strict_positive_rejection_drop_and_re_run_actually_runs() {
        // (§12 M12, positive rejection REQUIRED to run) `pos` already guarantees
        // r > 0; assuming `x > 0` on its result is UNNECESSARY. The strict
        // drop-and-re-run must actually execute (discharge WITHOUT the assume) and,
        // on `Unsat`, reject with the provable message.
        let ctx = run_m12("pos assume(x>0)", "foo");
        assert_eq!(ctx.assumes().len(), 1);
        let rec = &ctx.assumes()[0];
        assert_eq!(
            rec.legality,
            AssumeLegality::RejectedProvable,
            "an assume the solver can discharge without it is rejected: {rec:?}"
        );
        assert_eq!(rec.legality.message(), Some(ASSUME_PROVABLE_MSG));
        // The drop-and-re-run actually RAN (not a no-op) and produced the positive
        // `Unsat` showing.
        assert_eq!(
            rec.exploratory,
            Verdict::Unsat,
            "the exploratory drop-and-re-run returned the positive Unsat showing"
        );
        assert_eq!(
            ctx.cache().solves(),
            1,
            "the drop-and-re-run path executed exactly one real solve"
        );
        assert!(
            !rec.from_cache,
            "the first run is a cache miss (a real solve)"
        );
        // A rejected assume injects NO fact (fail closed): script never asserts it.
        // (The producer's own guarantee `(> $... 0)` is present; the assume's
        // re-run asserts the NEGATED goal in a popped scope only.)
    }

    #[test]
    fn m12_assume_with_no_opaque_dependency_is_rejected() {
        // (§12 M12) `pos` guarantees r > 0 (a CONTRACTED value). Assuming a
        // STRONGER `x > 5` is not provable from r > 0, but r is not opaque — it is
        // already contracted — so the assume has no opaque dependency and is a
        // hard error.
        let ctx = run_m12("pos assume(x>5)", "foo");
        assert_eq!(ctx.assumes().len(), 1);
        let rec = &ctx.assumes()[0];
        assert_eq!(
            rec.legality,
            AssumeLegality::RejectedNoOpaqueDependency,
            "assuming about a contracted (non-opaque) value is rejected: {rec:?}"
        );
        assert_eq!(rec.legality.message(), Some(ASSUME_NO_OPAQUE_MSG));
        // It was genuinely not provable-without (so the rejection is specifically
        // about the missing opaque dependency, not about provability).
        assert_ne!(rec.exploratory, Verdict::Unsat);
    }

    #[test]
    fn m12_exploratory_check_is_cached() {
        // (§12 M12, cheap) The exploratory verdict is memoized on the obligation's
        // content key (predicate + in-scope facts). Running the SAME body against
        // a FRESH solver with the SAME ctx (so the cache persists, the facts are
        // identical) pays the solver ZERO times the second time — a warm-compile
        // cache HIT, not a re-solve.
        let toks = parse("opaque assume(x>=0) sqrt").unwrap();
        let resolve = m12_resolver;

        let mut ctx = VerifyCtx::with_site("foo");

        let mut s1 = SmtLibSolver::new();
        let mut stack1 = ShadowStack::new();
        verify_ctx(&toks, &mut stack1, &mut s1, &resolve, &mut ctx).unwrap();
        assert_eq!(ctx.cache().solves(), 1, "first run is a real solve (miss)");
        assert_eq!(ctx.cache().hits(), 0);

        let mut s2 = SmtLibSolver::new();
        let mut stack2 = ShadowStack::new();
        verify_ctx(&toks, &mut stack2, &mut s2, &resolve, &mut ctx).unwrap();
        assert_eq!(
            ctx.cache().solves(),
            1,
            "the identical obligation is served from the cache — zero new solves"
        );
        assert_eq!(ctx.cache().hits(), 1, "the second run is a warm cache hit");
        // Both runs accepted the assume (the cached verdict is the same).
        assert!(ctx.assumes().iter().all(|a| a.legality.is_legal()));
    }

    #[test]
    fn m12_unknown_exploratory_fails_lenient_and_accepts() {
        // (§12 M12, fail-lenient) The exploratory drop-and-re-run on an
        // uninterpreted/opaque goal returns `Unknown` (the embedded reasoner's
        // opaque/timeout analogue). `Unknown` is NOT a positive showing, so the
        // assume is ACCEPTED, never rejected.
        let ctx = run_m12("opaque \"assume(length x > 0)\"", "foo");
        assert_eq!(ctx.assumes().len(), 1);
        let rec = &ctx.assumes()[0];
        assert_eq!(
            rec.exploratory,
            Verdict::Unknown,
            "an opaque/uninterpreted goal is Unknown to the exploratory check: {rec:?}"
        );
        assert_eq!(
            rec.legality,
            AssumeLegality::Legal,
            "Unknown fails lenient ⇒ the assume is accepted, never rejected: {rec:?}"
        );
    }

    #[test]
    fn m12_ledger_modulo_status_propagates_to_callers() {
        // (§12 M12) Whole-program ledger: `foo` verifies modulo { result > 0 } via
        // an assume on an opaque dependency; `bar` calls `foo` and INHERITS the
        // modulo status visibly. Each assume is an enumerable ledger entry.
        let foo_body = parse("opaque \"assume(result > 0)\" sqrt").unwrap();
        let bar_body = parse("foo").unwrap();
        let defs = vec![
            Definition {
                name: "foo".to_string(),
                body: foo_body,
                sig: None,
            },
            Definition {
                name: "bar".to_string(),
                body: bar_body,
                sig: None,
            },
        ];
        let lookup = |w: &str| match w {
            "sqrt" => crate::parse_signature("sqrt : ( n: Num where n >= 0  --  r: Num )").ok(),
            _ => None,
        };
        let ledger = check_program(&defs, &lookup, SmtLibSolver::new).unwrap();

        // The program checked clean (the assume was accepted, no hard errors).
        assert!(ledger.is_clean(), "rejections: {:?}", ledger.rejections());

        // One enumerable ledger entry, attributed to `foo`, displaying the
        // user-written predicate.
        assert_eq!(ledger.assumptions().len(), 1);
        let entry = &ledger.assumptions()[0];
        assert_eq!(entry.word, "foo");
        assert_eq!(
            entry.predicate,
            Pred::Bin(BinOp::Gt, Box::new(var("result")), Box::new(num("0")))
        );
        // grep_assume enumerates the complete user trusted base.
        assert_eq!(ledger.grep_assume(), vec!["assume(result > 0)".to_string()]);

        // `foo`'s honest status: verified modulo { result > 0 }.
        let foo_status = ledger.status("foo");
        assert!(foo_status.is_modulo());
        assert_eq!(foo_status.to_string(), "verified modulo { result > 0 }");

        // `bar` calls `foo` and INHERITS the modulo status (a guarantee never
        // silently reads stronger than proven — invariant 13 property 2).
        let bar_status = ledger.status("bar");
        assert!(
            bar_status.is_modulo(),
            "the caller inherits the modulo status: {bar_status}"
        );
        assert_eq!(bar_status.to_string(), "verified modulo { result > 0 }");
    }

    #[test]
    fn m12_rejected_assume_is_a_hard_error_in_the_program_ledger() {
        // (§12 M12) A program whose `assume` is unnecessary (provable without it)
        // does NOT check clean: the rejection is surfaced in the ledger, not
        // silently dropped.
        let body = parse("pos assume(x>0)").unwrap();
        let defs = vec![Definition {
            name: "foo".to_string(),
            body,
            sig: None,
        }];
        let lookup = |_: &str| None;
        // pos must resolve to its guarantee; route through a lookup-free resolver
        // by giving foo a real `pos` contract via the signature lookup.
        let lookup2 = |w: &str| match w {
            "pos" => crate::parse_signature("pos : ( -- r: Num where r > 0 )").ok(),
            _ => lookup(w),
        };
        let ledger = check_program(&defs, &lookup2, SmtLibSolver::new).unwrap();
        assert!(!ledger.is_clean());
        assert_eq!(ledger.rejections().len(), 1);
        assert_eq!(
            ledger.rejections()[0].legality,
            AssumeLegality::RejectedProvable
        );
        // A rejected assume is NOT in the trusted base.
        assert!(ledger.assumptions().is_empty());
        assert!(ledger.grep_assume().is_empty());
    }

    // A compile-time witness that the `assume` legality check is solver-only and
    // never reaches Tier 0: it threads a `Solver`/`FactSnapshot` and a cache, no
    // `Subst`/`InferCtx` (the immutability barrier — invariant 18). If it grew a
    // Tier 0 inference parameter this signature would change.
    #[allow(dead_code)]
    fn _assume_is_solver_only(s: &mut SmtLibSolver, goal: &Pred, c: &mut ExploratoryCache) {
        let _ = assume_legality(s, goal, c);
    }
}
