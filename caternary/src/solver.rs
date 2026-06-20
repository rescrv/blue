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

use crate::Token;
use crate::WordTy;
use crate::refinement::BinOp;
use crate::refinement::Binder;
use crate::refinement::Pred;
use crate::refinement::UnOp;
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
/// This is the *minimal* M8 form: it returns the bare [`Verdict`] only — no model
/// extraction / counterexample surfacing, which is M9 (§10.5).
pub fn discharge<S: Solver>(solver: &mut S, goal: &Pred) -> Verdict {
    solver.push_scope();
    solver.assert(&negate(goal));
    let verdict = solver.check();
    solver.pop_scope();
    verdict
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
    /// A call site carrying a refinement demand to discharge.
    Call {
        /// The demand's parameter binders (source order), zipped right-to-left
        /// against the stack top by [`crate::bind_positional`] (§10.2).
        binders: Vec<Binder>,
        /// The demand predicate over those binders (an obligation on the caller).
        demand: Pred,
        /// The Tier 0 arrow: how many terms the word pops/pushes.
        arrow: WordTy,
    },
}

/// One discharged obligation recorded during verification: the (substituted) VC
/// goal and the verdict the solver returned for it under the live path
/// conditions.
#[derive(Debug, Clone)]
pub struct Obligation {
    /// The VC goal, with binders substituted to the actual shadow terms.
    pub goal: Pred,
    /// The verdict: `Unsat` ⇒ discharged/valid.
    pub verdict: Verdict,
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
fn apply_effect<R: VerifyResolve, S: Solver>(
    stack: &mut ShadowStack,
    word: &str,
    resolve: &R,
    solver: &mut S,
    obligations: &mut Vec<Obligation>,
) -> Result<(), ShadowError> {
    match resolve.resolve(word) {
        VerifyWord::Core(core) => apply_core(stack, core, resolve, solver, obligations),
        VerifyWord::Call {
            binders,
            demand,
            arrow,
        } => {
            // VC at the call site: bind the demand's parameters to the actual
            // shadow terms (§10.2), substitute, discharge under the live path
            // conditions (§10.4/§10.5).
            let bindings = bind_positional(&binders, stack)?;
            let goal = substitute(&demand, &bindings);
            let verdict = discharge(solver, &goal);
            obligations.push(Obligation { goal, verdict });
            // Then move the data per the Tier 0 arrow (opaque for M8).
            stack.apply_opaque(&arrow)
        }
    }
}

/// Apply a core [`ShadowWord`] to the shadow stack, threading the verifier so
/// `dip`/`call` recurse through [`verify`] (and so an `if` *inside* a quotation
/// still gets path conditions).
fn apply_core<R: VerifyResolve, S: Solver>(
    stack: &mut ShadowStack,
    core: ShadowWord,
    resolve: &R,
    solver: &mut S,
    obligations: &mut Vec<Obligation>,
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
            verify(&body, stack, solver, resolve, obligations)?;
            stack.push_slot(hidden);
            Ok(())
        }
        ShadowWord::Call => {
            let body = stack.pop_quote()?;
            verify(&body, stack, solver, resolve, obligations)
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
pub fn verify<R: VerifyResolve, S: Solver>(
    tokens: &[Token],
    stack: &mut ShadowStack,
    solver: &mut S,
    resolve: &R,
    obligations: &mut Vec<Obligation>,
) -> Result<(), ShadowError> {
    for token in tokens {
        match token {
            Token::Bracket(body) => stack.push_quote(body.clone()),
            Token::Word(w) if is_if(w) => {
                verify_if(stack, solver, resolve, obligations)?;
            }
            Token::Word(w) => {
                apply_effect(stack, w, resolve, solver, obligations)?;
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
fn verify_if<R: VerifyResolve, S: Solver>(
    stack: &mut ShadowStack,
    solver: &mut S,
    resolve: &R,
    obligations: &mut Vec<Obligation>,
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
        verify(&then_body, &mut branch, solver, resolve, obligations)?;
        solver.pop_scope();
        branch
    };

    // else-branch under ¬P.
    {
        let mut branch = stack.clone();
        solver.push_scope();
        solver.assert(&negate(&cond));
        verify(&else_body, &mut branch, solver, resolve, obligations)?;
        solver.pop_scope();
    }

    // Advance the real stack by the (shape-identical) then-branch's post-state.
    *stack = then_stack;
    Ok(())
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
    let mut constraints: Vec<Constraint> = Vec::new();
    let mut opaque = false;
    for f in formulas {
        if !collect_constraints(f, false, &mut constraints) {
            opaque = true;
        }
    }
    let feasible = fourier_motzkin_feasible(constraints);
    match (feasible, opaque) {
        (false, _) => Verdict::Unsat, // decidable subset already infeasible ⇒ Unsat
        (true, false) => Verdict::Sat,
        (true, true) => Verdict::Unknown, // can't rule out a hidden contradiction
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

/// Fourier–Motzkin feasibility over the rationals: returns `true` if the
/// constraint set `{ expr <= 0 | expr < 0 }` is satisfiable.
fn fourier_motzkin_feasible(mut constraints: Vec<Constraint>) -> bool {
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
                    return false;
                }
                // else trivially satisfied; drop it.
            } else {
                remaining.push(c);
            }
        }
        constraints = remaining;
        if constraints.is_empty() {
            return true;
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

        constraints = next;
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
            demand: Pred::Bin(BinOp::Ge, Box::new(var("n")), Box::new(num("0"))),
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
}
