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
//! # Scope: M0–M3
//!
//! The driver shape (M0): definitions load into one namespace; the entry is
//! located; quotation descent pushes/pops typing frames (the durable provenance
//! spine). Sequence inference (M2) composes each word's stack arrow through the
//! M1 unifier, with §5 rules for literals, quotation literals, named locals, and
//! the language-core combinators (`DUP`/`DROP`/`SWAP`/`OVER`/`CALL`/`IF`).
//! Definitions are generalized by the M3 SCC pass ([`infer_definition_schemes`],
//! §6): the call graph is condensed by Tarjan, components are processed
//! dependencies-first, each component is inferred under monomorphic assumptions
//! (so legitimate self- and mutual recursion type-check as one component, and
//! inference-defeating polymorphic recursion is rejected with the §6 message),
//! then generalized into a [`Scheme`] a call site resolves by instantiation.
//! A quotation sees its **enclosing** locals (captured by value, mirroring the
//! runtime), so a reference to an outer local resolves to that binding's
//! monomorphic type.

use std::collections::HashMap;

use crate::Evaluator;
use crate::Quotable;
use crate::Span;
use crate::SpannedToken;
use crate::SpannedTokenKind;
use crate::Token;
use crate::evaluator::bind_target;
use crate::types::InferCtx;
use crate::types::MAIN;
use crate::types::RowVar;
use crate::types::Scheme;
use crate::types::StackTy;
use crate::types::Ty;
use crate::types::TyKind;
use crate::types::TyVar;
use crate::types::TypingFrame;
use crate::types::UnifyError;
use crate::types::WordTy;
use crate::types::core_scheme;
use crate::types::is_bool_literal;
use crate::types::is_numeric_literal;
use crate::types::respan_word;

/// A Tier-0 type-check error.
#[derive(Debug, Clone, PartialEq)]
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
    ///
    /// `error` is the unifier's [`UnifyError`], which already carries the
    /// **provenance pair** (both origin spans, §7.1) and never leaks internal
    /// variable names (§7). `frames` is the typing-frame **breadcrumb** (§7.2)
    /// captured at the checker's failing unify call site: the live
    /// [`crate::types::FrameStack`] snapshot, outermost first. It is empty for a
    /// failure that occurs outside any quotation (e.g. directly in the top-level
    /// program body).
    ///
    /// Rendering composes the three §7 requirements: the provenance pair, the
    /// nested-quotation backtrace, and **localize-inward** anchoring — the
    /// diagnostic points at the *innermost* frame whose span still encloses both
    /// conflicting birth spans (the deepest frame carrying the contradiction,
    /// §7.3), which is almost always the user's actual mistake.
    Mismatch {
        /// The underlying unification failure (provenance pair, §7.1).
        error: UnifyError,
        /// The typing-frame breadcrumb captured at the failing unify call site
        /// (§7.2), outermost first; empty outside any quotation.
        frames: Vec<TypingFrame>,
    },
    /// Reserved for a recursion shape Tier-0 genuinely cannot represent and that
    /// is not already covered by [`TypeError::PolymorphicRecursion`].
    ///
    /// With the M3 SCC pass (§6) legitimate self- and mutual recursion is
    /// accepted (one monomorphic strongly-connected component), and the
    /// inference-defeating case — a recursive call at a non-unifying type — is
    /// reported as [`TypeError::PolymorphicRecursion`] with the §6 annotation
    /// message. This variant is therefore **no longer produced for ordinary
    /// recursion**; it is retained only as a stable home for any future
    /// genuinely-unrepresentable case, and **no valid mutually-recursive program
    /// trips it** (the SCC pass admits them all).
    RecursiveDefinition {
        /// The definition caught referencing itself.
        name: String,
        /// The span of the offending reference.
        span: Span,
    },
    /// A definition uses **polymorphic recursion** (§6): a recursive call at a
    /// type that does not unify with the definition's monomorphic in-SCC
    /// assumption. Tier-0 inference cannot solve this (Henglein); the spec
    /// requires a clean rejection asking for a type annotation rather than a raw
    /// unification error or silent widening (§13 invariant 9).
    PolymorphicRecursion {
        /// The definition that recurses polymorphically.
        name: String,
        /// The span of the definition body where the contradiction surfaced.
        span: Span,
    },
    /// A definition applies its **quotation argument at more than one type**
    /// within a single body — the rank-2 case (§8). Tier-0 inference is rank-1:
    /// a quotation parameter has a single monomorphic arrow, so applying it at
    /// two genuinely distinct types cannot be inferred. The spec requires the
    /// targeted §8 message asking for a type annotation, **not** a raw
    /// unification mismatch (§13 invariant 8 territory; see §10.11(b)). With a
    /// correct annotation the body checks instead (the annotation surface).
    Rank2 {
        /// The definition that applies its quotation at more than one type.
        name: String,
        /// The span of the definition body where the second application clashed.
        span: Span,
    },
    /// A Tier-0 stack-effect annotation (the `[ effect ] @name` surface) is
    /// malformed: e.g. no `--` separator, a stray separator, or an unparsable
    /// element. Located at the annotation's span.
    BadAnnotation {
        /// The definition the annotation was attached to.
        name: String,
        /// What is wrong with the annotation.
        detail: String,
        /// The span of the annotation.
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
            TypeError::Mismatch { error, frames } => render_mismatch(f, error, frames),
            TypeError::RecursiveDefinition { name, span } => {
                write!(
                    f,
                    "recursive definition `{name}` at byte {}: mutual/self recursion is resolved by the SCC pass (not yet available)",
                    span.start
                )
            }
            TypeError::PolymorphicRecursion { name, span } => {
                write!(
                    f,
                    "`{name}` appears to use polymorphic recursion, which cannot be inferred; add a type annotation (near byte {})",
                    span.start
                )
            }
            TypeError::Rank2 { name, span } => {
                write!(
                    f,
                    "`{name}` applies its quotation at more than one type; add a type annotation (near byte {})",
                    span.start
                )
            }
            TypeError::BadAnnotation { name, detail, span } => {
                write!(
                    f,
                    "malformed stack-effect annotation for `{name}` (byte {}): {detail}",
                    span.start
                )
            }
        }
    }
}

impl std::error::Error for TypeError {}

/// Build a [`TypeError::Mismatch`] enriched with the live typing-frame
/// breadcrumb (§7.2). Called at every checker unify call site so a unification
/// failure raised deep inside nested quotations carries the durable provenance
/// spine ([`crate::types::FrameStack`]) it needs to render a backtrace and to
/// localize inward (§7.3). The unifier (`types.rs`) raises a span-only
/// [`UnifyError`]; this is where the checker, which holds the live frame stack,
/// attaches the breadcrumb the unifier cannot see.
fn mismatch(ctx: &InferCtx, error: UnifyError) -> TypeError {
    TypeError::Mismatch {
        error,
        frames: ctx.frames.breadcrumb(),
    }
}

/// Does `outer` enclose `inner` (a frame span containing a birth span)?
fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// **Localize inward** (§7.3): from a breadcrumb (outermost first) and the two
/// conflicting birth spans, choose the *innermost* frame whose span still
/// encloses **both** spans — the deepest frame carrying the contradiction, which
/// is almost always the user's real mistake. Returns the index into `frames`, or
/// `None` if no single frame encloses both (the contradiction straddles frames,
/// so the outer site is the honest anchor) or there are no frames.
fn localize_inward(frames: &[TypingFrame], left: Span, right: Span) -> Option<usize> {
    // Walk inward (deepest last) and take the deepest enclosing frame.
    frames
        .iter()
        .enumerate()
        .rev()
        .find(|(_, fr)| span_contains(fr.span, left) && span_contains(fr.span, right))
        .map(|(i, _)| i)
}

/// Render a [`TypeError::Mismatch`]: the provenance pair (§7.1), then the
/// nested-quotation frame breadcrumb (§7.2), anchored at the innermost frame
/// still carrying the contradiction (§7.3). Internal variable names never appear
/// — the underlying [`UnifyError`] renders shapes, and frames render only source
/// byte offsets (§7 / §13 invariant 6).
fn render_mismatch(
    f: &mut std::fmt::Formatter<'_>,
    error: &UnifyError,
    frames: &[TypingFrame],
) -> std::fmt::Result {
    // §7.1: the provenance pair, exactly as the unifier described it.
    write!(f, "{error}")?;

    if frames.is_empty() {
        return Ok(());
    }

    // §7.3: pick the anchor frame from the conflicting birth spans, if we have a
    // provenance pair. A cyclic error carries one span; anchor on it alone.
    let anchor = match error {
        UnifyError::Mismatch {
            left_span,
            right_span,
            ..
        } => localize_inward(frames, *left_span, *right_span),
        UnifyError::Cyclic { span, .. } => frames
            .iter()
            .enumerate()
            .rev()
            .find(|(_, fr)| span_contains(fr.span, *span))
            .map(|(i, _)| i),
    };

    // §7.2: render the breadcrumb, outermost first, as a backtrace. Mark the
    // anchor frame (the innermost concrete contradiction) so the reader's eye
    // lands on the likely mistake rather than the outer consequence.
    for (i, fr) in frames.iter().enumerate() {
        if Some(i) == anchor {
            write!(
                f,
                "\n  in the quotation at byte {} (innermost frame carrying the contradiction)",
                fr.span.start
            )?;
        } else {
            write!(f, "\n  in the quotation at byte {}", fr.span.start)?;
        }
    }
    Ok(())
}

/// The Tier-0 **CI-gate entry point** — the public seam the eventual
/// `caternary check` command drives (§12 *"Tier 0 done … public entry point
/// wired into the existing pipeline"*; §10.10 / invariant 20 *"CI gate; free
/// runtime"*).
///
/// This is the pass/fail face of [`type_check`]: it runs full Tier-0
/// shape-safety over the whole program loaded into `evaluator` (the flat global
/// definition namespace plus the attested operator contracts) and reports `Ok(())`
/// when the program is *checked* or a [`TypeError`] — provenance pair, frame
/// breadcrumb, innermost-frame anchor (§7) — when it is not. The inferred effect
/// itself is an implementation detail of the gate; callers that want it use
/// [`type_check`]. A checked program carries **no** verification residue into the
/// runtime (annotation directives are inert, §10.10): `check` is the build-time
/// gate, `Evaluator::eval` is the lean runtime.
///
/// Wiring: this is the single function a host's `caternary check` subcommand or a
/// CI step calls after [`Evaluator::load_with_spans`]; everything else in the
/// Tier-0 checker hangs off it.
pub fn check<T>(evaluator: &Evaluator<T>) -> Result<(), TypeError>
where
    T: Quotable,
{
    type_check(evaluator).map(|_effect| ())
}

/// The failure mode of the **combined whole-program gate** ([`check_whole_program`]):
/// either Tier 0 (shape safety) rejected the program, or — only reached when Tier 0
/// is green — Tier 1 (refinement / shadow-stack) rejected it, either structurally
/// ([`GateError::Tier1`]), by leaving one or more refinement **obligations
/// undischarged** ([`GateError::Tier1Violated`] — §10.7 Situation B, a demand
/// actually violated with a counterexample), or by recording one or more **illegal
/// `assume`s** in an otherwise-completed ledger ([`GateError::Tier1Rejected`]).
///
/// The variants encode the gate's **ordering invariant** structurally (§10.10,
/// invariant 19): a [`GateError::Tier1`] / [`GateError::Tier1Rejected`] can only
/// ever be observed for a program whose arities Tier 0 has *already* balanced,
/// because the gate never reaches the Tier 1 call when Tier 0 returns
/// [`GateError::Tier0`].
///
/// [`GateError::Tier1Rejected`] is how the gate **fails closed** on a §10.7 hard
/// error: `check_program` records an illegal `assume` (provable goal, or no opaque
/// dependency in its chain) into the ledger's rejections and still returns `Ok`, so
/// the gate inspects the ledger and turns a non-clean one into this error. This
/// honors invariant 13 ("the ledger means *cannot honestly prove*, never *didn't
/// bother*") and invariant 20 (the CI gate fails closed): `Ok(ledger)` from the
/// gate means a *clean* ledger, and a caller never has to consult
/// [`is_clean`](crate::Ledger::is_clean) to learn pass/fail.
#[derive(Debug)]
pub enum GateError {
    /// Tier 0 (shape safety) rejected the program. Tier 1 was **not** run.
    Tier0(crate::TypeError),
    /// Tier 1 (refinement verification / operator-axiom discharge) rejected the
    /// program **structurally** (e.g. a shadow-stack failure). Only reachable
    /// after Tier 0 passed.
    Tier1(crate::ShadowError),
    /// Tier 1 completed but the resulting ledger is **not clean**: it carries one
    /// or more illegal `assume`s (§10.7 hard error — provable goal, or no opaque
    /// dependency). The rejected [`AssumeRecord`](crate::AssumeRecord)s travel
    /// with the error so their structured reasons
    /// ([`ASSUME_PROVABLE_MSG`](crate::ASSUME_PROVABLE_MSG) /
    /// [`ASSUME_NO_OPAQUE_MSG`](crate::ASSUME_NO_OPAQUE_MSG)) survive. Only
    /// reachable after Tier 0 passed.
    Tier1Rejected(Vec<crate::AssumeRecord>),
    /// Tier 1 completed but one or more refinement **obligations were not
    /// discharged**: a demand/guarantee whose VC came back `Sat` (refuted, with a
    /// counterexample [`Model`](crate::Model)) or `Unknown` (undecided — fails
    /// closed for higher-order subsumption per §10.6). This is §10.7 *Situation B*
    /// — a refinement demand actually violated, *not* assumed away — and the gate
    /// must fail closed (§10.5 M9 / invariant 20). The undischarged
    /// [`Obligation`](crate::Obligation)s travel with the error so the
    /// counterexample model survives for diagnostics. Only reachable after Tier 0
    /// passed.
    Tier1Violated(Vec<crate::Obligation>),
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateError::Tier0(e) => write!(f, "{e}"),
            GateError::Tier1(e) => write!(f, "{e}"),
            GateError::Tier1Rejected(rejections) => {
                write!(
                    f,
                    "Tier 1 rejected {} illegal `assume`(s):",
                    rejections.len()
                )?;
                for rec in rejections {
                    let reason = rec
                        .legality
                        .message()
                        .unwrap_or("illegal `assume` (hard error)");
                    write!(f, "\n  - {} at `{}`: {reason}", rec.surface, rec.site)?;
                }
                Ok(())
            }
            GateError::Tier1Violated(violations) => {
                write!(
                    f,
                    "Tier 1 left {} refinement obligation(s) undischarged:",
                    violations.len()
                )?;
                for ob in violations {
                    write!(
                        f,
                        "\n  - `{}` demands `{}` ({:?})",
                        ob.word,
                        crate::render_smtlib(&ob.goal),
                        ob.verdict
                    )?;
                    // A targeted diagnostic (carries-no-contract / fail-closed,
                    // §10.7/§10.6 invariant 12) supersedes the bare SMT witness:
                    // the actionable cause, not the incidental counterexample.
                    if let Some(message) = &ob.message {
                        write!(f, " — {message}")?;
                    } else if let Some(model) = &ob.model {
                        write!(f, " — counterexample {model}")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for GateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GateError::Tier0(e) => Some(e),
            GateError::Tier1(e) => Some(e),
            // The rejected assumes / undischarged obligations are carried as data,
            // not as a single source error, so there is no single underlying cause
            // to surface here.
            GateError::Tier1Rejected(_) => None,
            GateError::Tier1Violated(_) => None,
        }
    }
}

/// The **single whole-program CI gate** — the one build-time act the spec
/// personifies as `caternary check` (§10.10, invariant 20): it pays the *entire*
/// verification cost in one call — Tier 0, then Tier 1, then operator-axiom
/// discharge — and returns the unified outcome (the [`Ledger`](crate::Ledger) of
/// the Tier 1 pass) or the first tier that rejected.
///
/// This composes the two halves that were previously separately-shaped and wired
/// nowhere but tests:
///
/// 1. **Tier 0 first.** Runs [`check`] over the whole program loaded into
///    `evaluator`. On rejection it returns [`GateError::Tier0`] **immediately and
///    runs no Tier 1** — the shadow stack and the solver are never touched.
/// 2. **Tier 1 only on green.** With Tier 0 satisfied, derives the
///    [`Definition`](crate::Definition) list (each definition's name, its body, and
///    its attached [`RefinementSig`](crate::RefinementSig)) and a refinement-sig
///    `lookup` from the evaluator, then runs
///    [`check_program`](crate::check_program) (Tier 1 + operator-axiom discharge).
///    `Ok(ledger)` means a **clean** ledger *and* every refinement obligation
///    **discharged**. The gate **fails closed** on two distinct Tier-1 hard errors:
///    a refinement obligation left undischarged (a demand/guarantee refuted with a
///    counterexample, or undecided — §10.5/§10.7 Situation B) **fails closed** as
///    [`GateError::Tier1Violated`] carrying the offending obligations and their
///    counterexample models; an illegal `assume` (§10.7 hard error) **fails closed**
///    as [`GateError::Tier1Rejected`] carrying the rejected records. The gate never
///    returns `Ok` on a non-clean ledger, so callers read pass/fail straight off
///    the `Result` and need not consult [`is_clean`](crate::Ledger::is_clean).
///
/// This realizes invariant 19 **structurally rather than by caller convention**:
/// Tier 1's shadow-stack soundness is parasitic on Tier 0 having already balanced
/// every arity, and the gate guarantees Tier 1 never runs on a program Tier 0 has
/// not first accepted.
///
/// `mk_solver` builds a fresh solver per definition (the seam is
/// per-checking-session); pass [`SmtLibSolver::new`](crate::SmtLibSolver::new) for
/// the default in-tree backend (a checked program links no solver — invariants
/// 14/20 — so the solver is a build-time-only dependency).
///
/// Definitions are gathered in **name order** so the resulting ledger is
/// deterministic regardless of the (unspecified) iteration order of the evaluator's
/// definition table.
pub fn check_whole_program<T, S, MkSolver>(
    evaluator: &Evaluator<T>,
    mk_solver: MkSolver,
) -> Result<crate::Ledger, GateError>
where
    T: Quotable,
    S: crate::Solver + crate::CounterModel + crate::FactSnapshot,
    MkSolver: FnMut() -> S,
{
    // Tier 0 first — the immutability barrier and the arity floor Tier 1 rides on
    // (invariant 19). On rejection we return *without* constructing a shadow stack
    // or a solver: Tier 1 must never run on a program Tier 0 has not accepted.
    check(evaluator).map_err(GateError::Tier0)?;

    // Tier 0 is green: bridge the evaluator into the Tier 1 shape. Each loaded
    // definition contributes its name, its (spanless) body, and its attached
    // refinement signature; the lookup resolves any referenced word's signature
    // (callees, operators) the same way the standalone Tier 1 entry does.
    let mut names: Vec<&str> = evaluator.definition_names().collect();
    names.sort_unstable();
    let defs: Vec<crate::Definition> = names
        .iter()
        .map(|&name| crate::Definition {
            name: name.to_string(),
            body: evaluator
                .definition_body(name)
                .map(<[crate::Token]>::to_vec)
                .unwrap_or_default(),
            sig: evaluator.refinement(name).cloned(),
        })
        .collect();
    let lookup = |w: &str| evaluator.refinement(w).cloned();

    let ledger = crate::check_program(&defs, &lookup, mk_solver).map_err(GateError::Tier1)?;

    // Fail closed on an UNDISCHARGED obligation (§10.5/§10.7 Situation B):
    // `check_program` records every refuted (`Sat`, with a counterexample) or
    // undecided (`Unknown`) VC into the ledger's violations and *still* returns
    // `Ok`. The CI gate must reject a program whose refinement demand is actually
    // violated (invariant 20 / M9), so any violation becomes a gate error carrying
    // the offending obligations and their counterexample models.
    if !ledger.violations().is_empty() {
        return Err(GateError::Tier1Violated(ledger.violations().to_vec()));
    }

    // Fail closed on a §10.7 hard error: `check_program` records an illegal
    // `assume` (provable goal, or no opaque dependency in its chain) into the
    // ledger's rejections and *still* returns `Ok`. The CI gate must reject such a
    // program (invariants 13/20), so a non-clean ledger becomes a gate error
    // carrying the rejected records (and their ASSUME_PROVABLE_MSG /
    // ASSUME_NO_OPAQUE_MSG reasons). Only a clean ledger is a pass.
    if !ledger.is_clean() {
        return Err(GateError::Tier1Rejected(ledger.rejections().to_vec()));
    }

    Ok(ledger)
}

/// Type-checks the whole program: locate the distinguished entry [`MAIN`] and
/// check it against the empty initial stack.
///
/// Returns the **inferred whole-program effect** (§5): the composition of every
/// word's stack arrow in `main`'s body, unified left-to-right, then required to
/// close against the empty initial stack. A program that demands inputs from the
/// empty stack underflows and is rejected (§12 M2). For the plain pass/fail
/// CI-gate face, use [`check`].
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

    // M3: generalize every loaded definition via the SCC pass first (§6). This
    // both validates all definition bodies (mutual recursion, polymorphic
    // recursion, mismatches surface here) and yields a `Scheme` per definition
    // that a call site resolves by *instantiation* — no more inlining bodies.
    let schemes = infer_definition_schemes(evaluator, &mut ctx)?;
    let def_env = DefEnv {
        schemes,
        mono: HashMap::new(),
    };

    let mut locals: Vec<Local> = Vec::new();
    let no_poly: HashMap<String, Scheme> = HashMap::new();

    // The program is the **top-level** sequence: the initial stack is empty
    // (§12), so any word demanding values from below the empty floor is an
    // underflow, attributed to that word (§7). Definition references inside
    // `main` resolve through their generalized schemes. The program body has no
    // polymorphic (rank-2) locals — those exist only inside an annotated word.
    let effect = infer_seq(
        evaluator,
        body,
        &mut ctx,
        &mut locals,
        &def_env,
        &no_poly,
        true,
    )?;

    Ok(ctx.resolve_word_deep(&effect))
}

/// Infer the stack-effect type of a quotation body against an evaluator's
/// current definition and operator environment.
///
/// This is the shell/introspection counterpart to [`type_check_entry`]: it does
/// not require a `main` definition and it does not impose the empty top-level
/// stack floor. The supplied tokens are treated as the body of a quotation, so
/// the returned [`WordTy`] is the quotation's own arrow `( before -- after )`.
///
/// Definitions referenced by the quotation are resolved through the same SCC
/// generalization pass used by whole-program checking. If the evaluator contains
/// definitions, they should have been loaded with
/// [`Evaluator::load_with_spans`] so diagnostics can retain source locations.
pub fn infer_quote_type<T>(evaluator: &Evaluator<T>, tokens: &[Token]) -> Result<WordTy, TypeError>
where
    T: Quotable,
{
    let mut ctx = InferCtx::new();
    let schemes = infer_definition_schemes(evaluator, &mut ctx)?;
    let def_env = DefEnv {
        schemes,
        mono: HashMap::new(),
    };
    let spanned = synthetic_spanned_tokens(tokens, SYNTH_SPAN);
    let mut locals: Vec<Local> = Vec::new();
    let no_poly: HashMap<String, Scheme> = HashMap::new();
    let arrow = infer_seq(
        evaluator,
        &spanned,
        &mut ctx,
        &mut locals,
        &def_env,
        &no_poly,
        false,
    )?;
    Ok(ctx.resolve_word_deep(&arrow))
}

fn synthetic_spanned_tokens(tokens: &[Token], span: Span) -> Vec<SpannedToken> {
    tokens
        .iter()
        .map(|token| match token {
            Token::Word(word) => SpannedToken {
                span,
                kind: SpannedTokenKind::Word(word.clone()),
            },
            Token::Bracket(inner) => SpannedToken {
                span,
                kind: SpannedTokenKind::Bracket(synthetic_spanned_tokens(inner, span)),
            },
        })
        .collect()
}

/// The definition environment threaded through inference (§6). A definition word
/// resolves either to a **monomorphic in-SCC assumption** (`mono`) — used for
/// recursive calls within the strongly-connected component currently being
/// inferred, which must stay un-generalized (the no-polymorphic-recursion rule)
/// — or to an **already-generalized `Scheme`** (`schemes`) for definitions in
/// previously-processed SCCs, instantiated fresh at each call site.
struct DefEnv {
    /// Generalized schemes for definitions whose SCC has been fully processed.
    schemes: HashMap<String, Scheme>,
    /// Monomorphic arrow assumptions for members of the SCC under inference.
    mono: HashMap<String, WordTy>,
}

impl DefEnv {
    /// An empty definition environment (no definitions in scope) — used by the
    /// bare-snippet inference path in tests.
    #[cfg(test)]
    fn empty() -> Self {
        DefEnv {
            schemes: HashMap::new(),
            mono: HashMap::new(),
        }
    }
}

/// A named local (`>name`) (§5). Ordinarily a local is **monomorphic**: every
/// use yields the *same* `Ty` so all occurrences unify together. The one
/// exception is the rank-2 case (§8): a quotation parameter that must be applied
/// at more than one type. Such a local is **polymorphic** — each use instantiates
/// a fresh copy of its scheme — which is only ever created when checking a body
/// against a rank-2 annotation (the declared quote scheme) or by the rank-2
/// *probe* that detects an un-annotated rank-2 word (a fully generic quote).
#[derive(Clone)]
enum Local {
    /// A monomorphic local: one fixed element type shared by every use.
    Mono {
        /// The local's name.
        name: String,
        /// The single element type every use of the local pushes.
        ty: Ty,
    },
    /// A polymorphic (rank-2) local: a quotation parameter whose *use effect*
    /// (`( 'a -- 'a Quote(…) )`) is a [`Scheme`] instantiated fresh at every
    /// use, so the parameter may be applied at more than one type within one body
    /// (§8). Created only on the annotation-checking path and the rank-2 probe.
    Poly {
        /// The local's name.
        name: String,
        /// The generalized use effect; each use instantiates it with fresh vars.
        scheme: Scheme,
    },
}

impl Local {
    /// The local's name (shared by both variants).
    fn name(&self) -> &str {
        match self {
            Local::Mono { name, .. } | Local::Poly { name, .. } => name,
        }
    }
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
/// The dual-purpose element type of a bracket body, if the body can stand in
/// for a `List` (§8).
///
/// Returns `Some(Num)` when every token is a numeric literal, `Some(Bool)` when
/// every token is a boolean literal, and recursively returns `Some(List elem)`
/// for nested bracket literals whose own bodies are literal lists. The *empty*
/// bracket returns `Some(a)` for a fresh type variable, since an empty list is
/// mappable at every element type (`∀a. List a`, the polymorphic empty list;
/// sound because there are zero elements to iterate). Returns `None` for a body
/// that does not monomorphize (`[ 1 true ]`, `[ [ 1 ] [ true ] ]`) or that
/// contains a word which computes (`[ 1 2 + ]`). Operating on the *syntax* —
/// rather than the inferred arrow, which cannot distinguish `[ 1 ]` from
/// `[ 1 1 + ]` — is what keeps the resulting `List elem` coercion sound against
/// the runtime's element-wise `as_sequence`.
fn literal_list_elem(body: &[SpannedToken], span: Span, ctx: &mut InferCtx) -> Option<Box<Ty>> {
    if body.is_empty() {
        // The polymorphic empty list: no element fixes the type, so mint a
        // fresh variable that unifies with whatever the consumer demands.
        return Some(Box::new(Ty::var(ctx.fresh_ty(), span)));
    }
    let mut elem: Option<Ty> = None;
    for token in body {
        let next = literal_token_elem(token, ctx)?;
        match &elem {
            Some(prev) => {
                if ctx.unify_ty(prev, &next).is_err() {
                    return None;
                }
            }
            None => elem = Some(next),
        }
    }
    elem.map(|elem| Box::new(ctx.resolve_ty_deep(&elem)))
}

fn literal_token_elem(token: &SpannedToken, ctx: &mut InferCtx) -> Option<Ty> {
    match &token.kind {
        SpannedTokenKind::Word(w) => {
            if is_numeric_literal(w) {
                Some(Ty::num(token.span))
            } else if is_bool_literal(w) {
                Some(Ty::bool(token.span))
            } else {
                None
            }
        }
        SpannedTokenKind::Bracket(inner) => {
            let elem = literal_list_elem(inner, token.span, ctx)?;
            Some(Ty::app("List", vec![*elem], token.span))
        }
    }
}

fn infer_seq<T>(
    evaluator: &Evaluator<T>,
    tokens: &[SpannedToken],
    ctx: &mut InferCtx,
    locals: &mut Vec<Local>,
    def_env: &DefEnv,
    poly: &HashMap<String, Scheme>,
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
                word_effect(evaluator, w, token.span, ctx, locals, def_env, poly)?,
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
                // A quotation opens a lexical scope for locals, but it must see
                // the **enclosing** locals: the runtime captures them into the
                // quotation by value (evaluator.rs::capture_body), so a reference
                // to an outer local resolves to the SAME monomorphic `Ty` here
                // (cloning preserves the type variable). An inner `>name` rebind
                // shadows the captured one because `word_effect` resolves locals
                // innermost-first (`rev().find`) and binders push onto the end.
                let mut inner_locals: Vec<Local> = locals.clone();
                let result = infer_seq(
                    evaluator,
                    inner,
                    ctx,
                    &mut inner_locals,
                    def_env,
                    poly,
                    false,
                );
                ctx.frames.pop();
                let mut body_arrow = result?;
                // Dual-purpose tag (§8): if the body is pure monomorphic literal
                // data, record the element type so the unifier may coerce this
                // quote to `List elem`. Computed from the *syntax* (not the
                // arrow), which is what makes it sound — a body that computes,
                // like `[ 1 2 + ]`, is not all-literals and stays untagged.
                body_arrow.list_elem = literal_list_elem(inner, frame_span, ctx);
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
        // advance the output to the word's output (§5). A failure is enriched
        // with the live typing-frame breadcrumb (§7.2) before it escapes.
        if let Err(e) = ctx.unify_stack(&acc.output, &word_arrow.input) {
            return Err(mismatch(ctx, e));
        }
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
/// language-core primitive, or a definition (resolved through its generalized
/// `Scheme`, or its monomorphic in-SCC assumption for a recursive call, §6).
fn word_effect<T>(
    evaluator: &Evaluator<T>,
    w: &str,
    span: Span,
    ctx: &mut InferCtx,
    locals: &mut Vec<Local>,
    def_env: &DefEnv,
    poly: &HashMap<String, Scheme>,
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
        // A binder names the popped value. Normally the local is monomorphic
        // (§5). The exception: if `name` is flagged polymorphic for this body
        // (the rank-2 case, §8) — either by a rank-2 annotation or the rank-2
        // probe — bind it as a `Poly` local so each use instantiates fresh,
        // letting the quotation parameter be applied at more than one type.
        if let Some(scheme) = poly.get(name) {
            locals.push(Local::Poly {
                name: name.to_string(),
                scheme: scheme.clone(),
            });
        } else {
            locals.push(Local::Mono {
                name: name.to_string(),
                ty: t,
            });
        }
        return Ok(WordTy::new(input, output));
    }

    // A use of a bound local (innermost binding wins on shadowing) (§5). A
    // monomorphic local yields the *same* `t` every time; a polymorphic (rank-2)
    // local instantiates its use-effect scheme fresh, so distinct uses may be
    // applied at distinct types (§8).
    if let Some(local) = locals.iter().rev().find(|l| l.name() == w) {
        match local {
            Local::Mono { ty, .. } => {
                let t = ty.clone();
                let r = ctx.fresh_row();
                return Ok(WordTy::new(
                    StackTy::empty(r, span),
                    StackTy::new(vec![Ty { kind: t.kind, span }], r, span),
                ));
            }
            Local::Poly { scheme, .. } => {
                let scheme = scheme.clone();
                let inst = ctx.instantiate(&scheme);
                return Ok(respan_word(&inst, span));
            }
        }
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

    // `assume( P )` (§10.7) is a Tier-1 path-condition marker, **not** a value
    // operator: it asserts a predicate about the current top of stack and moves
    // no data, so its Tier-0 shape effect is the identity ( 'r -- 'r ). Tier 1
    // (`verify_ctx`) intercepts the same word before word resolution; Tier 0 must
    // likewise accept it natively — it is a language construct (like `IF`), never a
    // registered operator and never in the operator table — so an assume-bearing
    // program passes the whole-program gate's Tier-0 half (invariant 19/20). The
    // predicate text is opaque to Tier 0; a malformed clause surfaces at Tier 1.
    if crate::parse_assume(w).is_some() {
        let r = ctx.fresh_row();
        return Ok(WordTy::new(
            StackTy::empty(r, span),
            StackTy::empty(r, span),
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

    // A definition under inference in the *current* SCC: use its monomorphic
    // arrow assumption directly (recursive calls share the un-generalized type,
    // the no-polymorphic-recursion rule, §6). Re-anchor the spans at this call
    // site; the var ids are preserved so every recursive use unifies together.
    if let Some(arrow) = def_env.mono.get(w) {
        return Ok(respan_word(arrow, span));
    }

    // A definition in a previously-processed SCC: instantiate its generalized
    // `Scheme` with fresh vars (so distinct call sites do not alias) and
    // re-anchor at this call site (§6, §5 `lookup`).
    if let Some(scheme) = def_env.schemes.get(w) {
        let inst = ctx.instantiate(scheme);
        return Ok(respan_word(&inst, span));
    }

    Err(TypeError::UnresolvedWord {
        word: w.to_string(),
        span,
    })
}

// ===========================================================================
// M4 — combinator schemes are in `types::core_scheme` (DIP); this section is
// rank-2 detection (§8) and the Tier-0 stack-effect annotation surface.
// ===========================================================================
//
// # Recorded design: the Tier-0 stack-effect annotation surface (§8 / §10.11(b))
//
// `docs/typing.md` is read-only, so the *concrete syntax* of the one admitted
// Tier-0 annotation (the rank-2 case) is defined and recorded HERE, reconciled
// with the existing parser (which only ever produces `Word`/`Bracket` tokens via
// shell tokenization — `parser.rs`).
//
// A stack-effect annotation is a top-level construct that **parallels** the
// `[ body ] :name` definition: a **bracket** holding the effect tokens, followed
// by an **annotation binder word** `@name` naming the definition it annotates:
//
// ```text
// [ [ a -- ] -- ] @rank2     [ >f  1 f CALL  true f CALL ] :rank2
// ```
//
// Every token there (`[`, `]`, `@rank2`, `--`, `a`) is an ordinary parser token,
// so no parser change is needed; `Evaluator::load_with_spans` records the effect
// bracket against the named definition (the runtime `load` ignores it).
//
// Inside the effect bracket the postfix mini-language is:
//   * the bare word `--` separates the input element list (left) from the output
//     element list (right); exactly one `--` at the bracket's top level.
//   * `Num` / `Bool` are base types.
//   * any other identifier word is a **type variable**, interned by name so
//     repeated occurrences in one annotation are the same variable.
//   * a nested bracket `[ … -- … ]` is a **quotation arrow element**. A quotation
//     element in the **input** list is the rank-2 marker: the parameter it types
//     is applied **polymorphically** (instantiated fresh at each use) — exactly
//     the one admitted Tier-0 shape annotation (§8 / §10.11(b)).
//   * row variables are anonymous: each stack (outer input, outer output, and
//     each quotation arrow's two ends) carries a fresh row standing for its tail.
//
// # How an annotated body is checked
//
// The body's leading `>name` binders name the annotated input positions
// top-of-stack first (binder 0 ↔ top input). A binder whose input position is a
// rank-2 quotation becomes a **polymorphic local** bound to that quotation's
// generalized *use effect*, so each application instantiates fresh and the
// quotation may be used at more than one type. The body is then inferred with
// those polymorphic bindings (`infer_seq`'s `poly` map) and its arrow is unified
// against the declared annotation arrow. The declared signature — never the
// quotation body — is what `DIP`/combinator-style relay typing is checked
// against (§13 invariant 8).
//
// # How an un-annotated rank-2 word is detected
//
// Tier-0 inference is rank-1: a quotation parameter bound by `>name` is one
// monomorphic arrow. If a body applies such a parameter at two genuinely
// distinct types, monomorphic inference fails with a `Mismatch`. To tell that
// apart from an ordinary type error, the checker runs a **rank-2 probe**: it
// re-infers the body with every quotation parameter that is applied two or more
// times bound to a *fully generic* quotation (a fresh instance per use). If the
// probe succeeds where monomorphic inference failed, the failure was precisely
// the rank-2 pattern — reported with the targeted §8 message asking for an
// annotation (`TypeError::Rank2`), not a raw mismatch. If the probe still fails,
// the original `Mismatch` is the honest diagnostic and is returned unchanged.

/// The neutral span carried by synthesized annotation/probe nodes or spanless
/// shell quotations that have no single source byte of their own; real
/// diagnostics re-anchor at the offending call site, exactly as the
/// language-core schemes do.
const SYNTH_SPAN: Span = Span { start: 0, end: 0 };

/// True if `w` is an application combinator — `CALL` or `DIP` — i.e. a word
/// that *runs* a quotation. Used to recognize the rank-2 "applies its quotation"
/// pattern (§8).
fn is_apply_word(w: &str) -> bool {
    w == "CALL" || w == "DIP"
}

/// The maximal prefix of `tokens` that are `>name` binder words, in body order
/// (so the first entry names the **top** of the input stack). Used to map an
/// annotation's input positions onto the body's named parameters.
fn leading_binders(tokens: &[SpannedToken]) -> Vec<String> {
    let mut out = Vec::new();
    for token in tokens {
        match &token.kind {
            SpannedTokenKind::Word(w) => match bind_target(w) {
                Some(name) => out.push(name.to_string()),
                None => break,
            },
            SpannedTokenKind::Bracket(_) => break,
        }
    }
    out
}

/// The names of quotation parameters that are **applied at two or more sites**
/// in `tokens` — the rank-2 candidates (§8). A parameter is any `>name` binder;
/// an application is the parameter word immediately followed by `CALL`/`DIP`.
/// Scans nested quotations too, counting applications per name across the body.
fn rank2_candidates(tokens: &[SpannedToken]) -> Vec<String> {
    use std::collections::HashSet;
    let mut binders: HashSet<String> = HashSet::new();
    let mut applies: HashMap<String, usize> = HashMap::new();
    fn walk(
        tokens: &[SpannedToken],
        binders: &mut HashSet<String>,
        applies: &mut HashMap<String, usize>,
    ) {
        for (i, token) in tokens.iter().enumerate() {
            match &token.kind {
                SpannedTokenKind::Word(w) => {
                    if let Some(name) = bind_target(w) {
                        binders.insert(name.to_string());
                    } else if let Some(next) = tokens.get(i + 1)
                        && let SpannedTokenKind::Word(nw) = &next.kind
                        && is_apply_word(nw)
                    {
                        *applies.entry(w.clone()).or_insert(0) += 1;
                    }
                }
                SpannedTokenKind::Bracket(inner) => walk(inner, binders, applies),
            }
        }
    }
    walk(tokens, &mut binders, &mut applies);
    let mut out: Vec<String> = binders
        .into_iter()
        .filter(|n| applies.get(n).copied().unwrap_or(0) >= 2)
        .collect();
    out.sort();
    out
}

/// The *use effect* of a **fully generic** quotation parameter:
/// `( 'r -- 'r ( 's -- 't ) )` generalized over `'r 's 't`. Instantiating it
/// (one per use) pushes a fresh quotation whose arrow constrains nothing — the
/// rank-2 probe binding (§8). If the body type-checks with every twice-applied
/// parameter bound this way, the only thing that was wrong was rank-1
/// monomorphism, i.e. the word is genuinely rank-2.
fn generic_quote_use_scheme() -> Scheme {
    let s = SYNTH_SPAN;
    // Quotation arrow ( 's -- 't ): rows 1 and 2.
    let quote = Ty::quote(WordTy::new(StackTy::empty(1, s), StackTy::empty(2, s)), s);
    // Use effect ( 'r -- 'r quote ): row 0.
    let word = WordTy::new(StackTy::empty(0, s), StackTy::new(vec![quote], 0, s));
    Scheme::new(vec![], vec![0, 1, 2], word)
}

/// The rank-2 probe (§8): re-infer `body` with every quotation parameter that is
/// applied two or more times bound to a *fully generic* quotation, each use a
/// fresh instance. Runs on a CLONE of `ctx` so it never pollutes real inference
/// state. Returns `true` iff this poly-aware inference succeeds — i.e. the body
/// is well-typed once the quotation parameter is allowed to be applied at more
/// than one type, which is exactly the rank-2 pattern. Returns `false` when there
/// are no such candidates, or when the body fails for a different reason.
fn rank2_probe<T>(
    evaluator: &Evaluator<T>,
    body: &[SpannedToken],
    ctx: &InferCtx,
    def_env: &DefEnv,
) -> bool
where
    T: Quotable,
{
    let candidates = rank2_candidates(body);
    if candidates.is_empty() {
        return false;
    }
    let mut poly: HashMap<String, Scheme> = HashMap::new();
    for name in candidates {
        poly.insert(name, generic_quote_use_scheme());
    }
    let mut probe_ctx = ctx.clone();
    let mut locals: Vec<Local> = Vec::new();
    infer_seq(
        evaluator,
        body,
        &mut probe_ctx,
        &mut locals,
        def_env,
        &poly,
        false,
    )
    .is_ok()
}

/// A small interner for an annotation's variables: type variables by name,
/// row variables freshly numbered. Ids are *local* to one annotation; the parsed
/// arrows are generalized and then instantiated with real `InferCtx` vars, so
/// these small ids never collide with inference state.
struct AnnVars {
    ty: HashMap<String, TyVar>,
    next_ty: TyVar,
    next_row: RowVar,
}

impl AnnVars {
    fn new() -> Self {
        AnnVars {
            ty: HashMap::new(),
            next_ty: 0,
            next_row: 0,
        }
    }

    fn tyvar(&mut self, name: &str) -> TyVar {
        if let Some(&v) = self.ty.get(name) {
            return v;
        }
        let v = self.next_ty;
        self.next_ty += 1;
        self.ty.insert(name.to_string(), v);
        v
    }

    fn row(&mut self) -> RowVar {
        let v = self.next_row;
        self.next_row += 1;
        v
    }
}

/// A parsed Tier-0 stack-effect annotation (§8 / §10.11(b)). `word` is the
/// declared arrow (with small interned var ids; generalize+instantiate before
/// use). `poly_input_use[k]` is `Some(scheme)` when the input element `k` *from
/// the top of stack* is a rank-2 quotation, carrying that parameter's
/// generalized use-effect scheme.
struct Annotation {
    word: WordTy,
    poly_input_use: Vec<Option<Scheme>>,
}

/// Parse the inner tokens of an annotation's effect bracket into an
/// [`Annotation`] (the surface recorded above). `name`/`span` are used only for
/// diagnostics.
fn parse_annotation(
    name: &str,
    tokens: &[SpannedToken],
    span: Span,
) -> Result<Annotation, TypeError> {
    let mut vars = AnnVars::new();
    let bad = |detail: &str| TypeError::BadAnnotation {
        name: name.to_string(),
        detail: detail.to_string(),
        span,
    };

    let (input_toks, output_toks) = split_on_arrow(tokens).ok_or_else(|| {
        bad("a stack effect needs exactly one `--` separating inputs from outputs")
    })?;

    let in_row = vars.row();
    let out_row = vars.row();
    let input = parse_elems(input_toks, &mut vars, &bad)?;
    let output = parse_elems(output_toks, &mut vars, &bad)?;

    // Rank-2 markers: a quotation in an INPUT position. Build the parameter's
    // generalized use-effect scheme, indexed from the TOP of the input stack
    // (input is stored bottom-first, so reverse).
    let mut poly_input_use: Vec<Option<Scheme>> = Vec::with_capacity(input.len());
    for elem in input.iter().rev() {
        if matches!(elem.kind, TyKind::Quote(_)) {
            let r = vars.row();
            let use_word = WordTy::new(
                StackTy::empty(r, SYNTH_SPAN),
                StackTy::new(vec![elem.clone()], r, SYNTH_SPAN),
            );
            poly_input_use.push(Some(generalize(&use_word)));
        } else {
            poly_input_use.push(None);
        }
    }

    let word = WordTy::new(
        StackTy::new(input, in_row, span),
        StackTy::new(output, out_row, span),
    );
    Ok(Annotation {
        word,
        poly_input_use,
    })
}

/// Split a token list on the single top-level `--` word, returning
/// `(before, after)`, or `None` if there is not exactly one.
fn split_on_arrow(tokens: &[SpannedToken]) -> Option<(&[SpannedToken], &[SpannedToken])> {
    let mut at = None;
    for (i, token) in tokens.iter().enumerate() {
        if let SpannedTokenKind::Word(w) = &token.kind
            && w == "--"
        {
            if at.is_some() {
                return None;
            }
            at = Some(i);
        }
    }
    let i = at?;
    Some((&tokens[..i], &tokens[i + 1..]))
}

/// Parse a stack element list (left-to-right = bottom-to-top, top last).
fn parse_elems(
    tokens: &[SpannedToken],
    vars: &mut AnnVars,
    bad: &dyn Fn(&str) -> TypeError,
) -> Result<Vec<Ty>, TypeError> {
    let mut out = Vec::new();
    for token in tokens {
        out.push(parse_elem(token, vars, bad)?);
    }
    Ok(out)
}

/// Parse a single annotation element: a base type, a type variable, or a nested
/// quotation arrow.
fn parse_elem(
    token: &SpannedToken,
    vars: &mut AnnVars,
    bad: &dyn Fn(&str) -> TypeError,
) -> Result<Ty, TypeError> {
    match &token.kind {
        SpannedTokenKind::Word(w) => {
            if w == "--" {
                return Err(bad("unexpected `--` inside a stack element list"));
            }
            if w == crate::types::NUM {
                Ok(Ty::num(token.span))
            } else if w == crate::types::BOOL {
                Ok(Ty::bool(token.span))
            } else if w
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            {
                Ok(Ty::var(vars.tyvar(w), token.span))
            } else {
                Err(bad(
                    "a stack element must be `Num`, `Bool`, a type variable, or a `[ .. -- .. ]` quotation",
                ))
            }
        }
        SpannedTokenKind::Bracket(inner) => {
            let (qin, qout) = split_on_arrow(inner)
                .ok_or_else(|| bad("a quotation element needs one `--` inside its brackets"))?;
            let qin_row = vars.row();
            let qout_row = vars.row();
            let qin_elems = parse_elems(qin, vars, bad)?;
            let qout_elems = parse_elems(qout, vars, bad)?;
            let arrow = WordTy::new(
                StackTy::new(qin_elems, qin_row, token.span),
                StackTy::new(qout_elems, qout_row, token.span),
            );
            Ok(Ty::quote(arrow, token.span))
        }
    }
}

// ===========================================================================
// M3 — SCC generalization of top-level definitions (§6)
// ===========================================================================

/// Generalize every loaded definition into a `Scheme` (§6). The pipeline:
///
/// 1. Build the **call graph** (edge `A → B` when `A`'s body references the
///    definition `B`).
/// 2. **Tarjan SCC** to group mutually-recursive definitions; Tarjan emits each
///    component only after its dependencies, i.e. in **reverse-topological
///    order**, which is exactly the order §6 wants (dependencies first).
/// 3. For each SCC: assign each member a fresh **monomorphic** arrow assumption,
///    infer every body under those assumptions (recursive calls inside the SCC
///    use the un-generalized assumption — the no-polymorphic-recursion rule),
///    unify each inferred body against its assumption, then **generalize** each
///    member over the variables free in its arrow into a `Scheme`.
///
/// Returns the map `name → Scheme`. A definition word later resolves by
/// instantiating its scheme; recursion no longer inlines bodies.
/// The **generalized signature** ([`Scheme`]) of every loaded definition (M3 /
/// §6), keyed by name — the definition half of the whole-program contract set
/// the M14 attestation hash content-addresses (architecture section / §12 M14).
///
/// This runs the same SCC generalization pass [`type_check`] runs, so a program
/// that does not type-check returns the same [`TypeError`] here; a checked
/// program returns one `Scheme` per definition. It reads the evaluator
/// read-only (the §3 immutability barrier). Use [`crate::ContractSet`] to fold
/// these together with the operator table into the artifact attestation hash.
pub fn definition_schemes<T>(evaluator: &Evaluator<T>) -> Result<HashMap<String, Scheme>, TypeError>
where
    T: Quotable,
{
    let mut ctx = InferCtx::new();
    infer_definition_schemes(evaluator, &mut ctx)
}

fn infer_definition_schemes<T>(
    evaluator: &Evaluator<T>,
    ctx: &mut InferCtx,
) -> Result<HashMap<String, Scheme>, TypeError>
where
    T: Quotable,
{
    // A stable list of the definition names (Tarjan needs deterministic indices;
    // the *result* is order-independent, but a fixed iteration order keeps the
    // diagnostics and tests reproducible).
    let mut names: Vec<String> = evaluator.definition_names().map(str::to_string).collect();
    names.sort();
    let index: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    // Call graph: adjacency over definition indices. An edge A → B exists when
    // A's body references the definition B (B is a loaded definition name). This
    // over-approximates harmlessly (a local shadowing a definition name only
    // merges SCCs, which stays sound; the merged assumption is simply unused).
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
    for (i, name) in names.iter().enumerate() {
        let body = evaluator
            .definition_body_spanned(name)
            .expect("a loaded definition has a spanned body when loaded with spans");
        let mut refs: Vec<usize> = Vec::new();
        collect_def_refs(body, &index, &mut refs);
        refs.sort_unstable();
        refs.dedup();
        adj[i] = refs;
    }

    let sccs = tarjan_sccs(&adj);

    // Parse every definition's Tier-0 stack-effect annotation up front (the
    // rank-2 surface, §8). A definition with no `[ effect ] @name` simply has no
    // entry; a malformed annotation is rejected here.
    let mut anns: HashMap<String, Annotation> = HashMap::new();
    for name in &names {
        if let Some(toks) = evaluator.annotation_tokens(name) {
            let span = evaluator
                .annotation_span(name)
                .unwrap_or(Span { start: 0, end: 0 });
            anns.insert(name.clone(), parse_annotation(name, toks, span)?);
        }
    }

    let no_poly: HashMap<String, Scheme> = HashMap::new();

    let mut def_env = DefEnv {
        schemes: HashMap::new(),
        mono: HashMap::new(),
    };

    for scc in &sccs {
        // 1. Assign each member its in-SCC assumption. An **annotated** member's
        //    assumption is its declared arrow (instantiated fresh), so recursive
        //    references see the declared type. An **un-annotated** member gets a
        //    fresh monomorphic arrow ( 'r1 -- 'r2 ) shared by every in-SCC
        //    reference, forcing recursive uses to one monomorphic type.
        for &i in scc {
            let name = &names[i];
            let span = evaluator
                .definition_span(name)
                .unwrap_or(Span { start: 0, end: 0 });
            let assumption = if let Some(ann) = anns.get(name) {
                ctx.instantiate(&generalize(&ann.word))
            } else {
                let r_in = ctx.fresh_row();
                let r_out = ctx.fresh_row();
                WordTy::new(StackTy::empty(r_in, span), StackTy::empty(r_out, span))
            };
            def_env.mono.insert(name.clone(), assumption);
        }

        // 2. Infer each member's body under the assumptions and unify the body's
        //    arrow against the member's assumption.
        for &i in scc {
            let name = &names[i];
            let span = evaluator
                .definition_span(name)
                .unwrap_or(Span { start: 0, end: 0 });
            let body = evaluator
                .definition_body_spanned(name)
                .expect("a loaded definition has a spanned body when loaded with spans");
            let assumption = def_env
                .mono
                .get(name)
                .expect("the assumption was inserted above")
                .clone();

            if let Some(ann) = anns.get(name) {
                // Annotated: check the body against the declared signature. Map
                // the leading `>name` binders onto the annotation's input
                // positions (top-of-stack first); a rank-2 quotation input binds
                // a polymorphic local so it may be applied at more than one type
                // (§8). The body is then checked from the DECLARED signature, not
                // by expanding any quotation body (§13 invariant 8).
                let binders = leading_binders(body);
                let mut poly: HashMap<String, Scheme> = HashMap::new();
                for (k, binder) in binders.iter().enumerate() {
                    if let Some(Some(scheme)) = ann.poly_input_use.get(k) {
                        poly.insert(binder.clone(), scheme.clone());
                    }
                }
                let mut locals: Vec<Local> = Vec::new();
                let body_arrow =
                    infer_seq(evaluator, body, ctx, &mut locals, &def_env, &poly, false)?;
                // The body must match its declared arrow (which is `assumption`).
                if let Err(e) = ctx
                    .unify_stack(&body_arrow.input, &assumption.input)
                    .and_then(|()| ctx.unify_stack(&body_arrow.output, &assumption.output))
                {
                    return Err(mismatch(ctx, e));
                }
                continue;
            }

            // Un-annotated: ordinary monomorphic inference (rank-1). Checkpoint
            // first so a failed attempt can be rewound before the rank-2 probe.
            let cp = ctx.subst.checkpoint();
            let mut locals: Vec<Local> = Vec::new();
            let body_arrow =
                match infer_seq(evaluator, body, ctx, &mut locals, &def_env, &no_poly, false) {
                    Ok(arrow) => arrow,
                    Err(TypeError::Mismatch { error, frames }) => {
                        // A `Mismatch` might be the rank-2 pattern: a quotation
                        // parameter applied at two distinct types (§8). Rewind to
                        // a clean state and re-infer with every twice-applied
                        // parameter bound to a fully generic quotation. If THAT
                        // succeeds, the only obstacle was rank-1 monomorphism, so
                        // emit the targeted §8 diagnostic; otherwise the original
                        // mismatch (breadcrumb intact) is the honest error.
                        ctx.subst.rewind(cp);
                        if rank2_probe(evaluator, body, ctx, &def_env) {
                            return Err(TypeError::Rank2 {
                                name: name.clone(),
                                span,
                            });
                        }
                        return Err(TypeError::Mismatch { error, frames });
                    }
                    Err(other) => return Err(other),
                };
            // A failure of THIS unify means a recursive use forced the assumption
            // to a shape the body cannot satisfy — polymorphic recursion (§6).
            ctx.unify_stack(&body_arrow.input, &assumption.input)
                .and_then(|()| ctx.unify_stack(&body_arrow.output, &assumption.output))
                .map_err(|_| TypeError::PolymorphicRecursion {
                    name: name.clone(),
                    span,
                })?;
        }

        // 3. Generalize each member over the variables free in its (now fully
        //    constrained) arrow and publish the scheme; drop the monomorphic
        //    assumption so later SCCs resolve this definition by instantiation.
        for &i in scc {
            let name = &names[i];
            let assumption = def_env
                .mono
                .remove(name)
                .expect("the assumption was inserted above");
            let resolved = ctx.resolve_word_deep(&assumption);
            let scheme = generalize(&resolved);
            def_env.schemes.insert(name.clone(), scheme);
        }
    }

    Ok(def_env.schemes)
}

/// Collect the indices of every definition referenced (transitively through
/// nested quotations) by a definition body. A word token is a reference iff it
/// names a loaded definition.
fn collect_def_refs(tokens: &[SpannedToken], index: &HashMap<&str, usize>, out: &mut Vec<usize>) {
    for token in tokens {
        match &token.kind {
            SpannedTokenKind::Word(w) => {
                if let Some(&i) = index.get(w.as_str()) {
                    out.push(i);
                }
            }
            SpannedTokenKind::Bracket(inner) => collect_def_refs(inner, index, out),
        }
    }
}

/// Iterative Tarjan strongly-connected-components over the adjacency list. Returns
/// the SCCs in the order Tarjan finalizes them, which is **reverse-topological**
/// over the condensation (every component is emitted after all components it
/// depends on) — exactly the dependencies-first order §6 requires. The recursion
/// is made explicit on a worklist so a deep call chain cannot overflow the stack
/// (the same robustness mandate as the occurs check, §13 invariant 4).
///
/// The translation from the textbook recursive form preserves its two lowlink
/// rules exactly: a **tree edge** folds the child's *lowlink* into the parent's
/// when the child frame finishes (`fold_into` records the parent to update); a
/// **back edge** to a node still on the stack folds that node's *index*.
fn tarjan_sccs(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut lowlink: Vec<usize> = vec![0; n];
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    let mut next_index = 0usize;

    // A frame on the explicit call stack: the node `v`, the next adjacency edge
    // to examine, and the parent to fold `v`'s lowlink into when `v` finishes
    // (the recursive form's `min(parent.lowlink, v.lowlink)` after the call).
    struct Frame {
        v: usize,
        edge: usize,
        fold_into: Option<usize>,
    }

    for start in 0..n {
        if indices[start].is_some() {
            continue;
        }
        let mut work: Vec<Frame> = vec![Frame {
            v: start,
            edge: 0,
            fold_into: None,
        }];

        // Initialize the start frame's node (the `Enter` step) before looping.
        indices[start] = Some(next_index);
        lowlink[start] = next_index;
        next_index += 1;
        stack.push(start);
        on_stack[start] = true;

        while let Some(frame) = work.last_mut() {
            let v = frame.v;
            if frame.edge < adj[v].len() {
                let w = adj[v][frame.edge];
                frame.edge += 1;
                match indices[w] {
                    None => {
                        // Tree edge: descend into w, folding its lowlink back
                        // into v when w's frame finishes.
                        indices[w] = Some(next_index);
                        lowlink[w] = next_index;
                        next_index += 1;
                        stack.push(w);
                        on_stack[w] = true;
                        work.push(Frame {
                            v: w,
                            edge: 0,
                            fold_into: Some(v),
                        });
                    }
                    Some(w_index) => {
                        // Back/forward/cross edge: fold w's index iff w is still
                        // on the stack (i.e. in the current SCC search tree).
                        if on_stack[w] {
                            lowlink[v] = lowlink[v].min(w_index);
                        }
                    }
                }
            } else {
                // v is finished: if it is a root, pop its SCC, then fold v's
                // lowlink into its parent.
                if lowlink[v] == indices[v].expect("v has an index") {
                    let mut component = Vec::new();
                    loop {
                        let w = stack.pop().expect("non-empty while popping an SCC");
                        on_stack[w] = false;
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(component);
                }
                let fold_into = frame.fold_into;
                work.pop();
                if let Some(parent) = fold_into {
                    lowlink[parent] = lowlink[parent].min(lowlink[v]);
                }
            }
        }
    }

    sccs
}

/// Generalize a fully-resolved arrow into a `Scheme` by quantifying over every
/// type and row variable that appears in it (§6). At top level there is no
/// enclosing monomorphic environment — previously-processed definitions are
/// closed schemes contributing no free variables — so every free variable in the
/// arrow is generalizable.
fn generalize(arrow: &WordTy) -> Scheme {
    let mut tyvars: Vec<TyVar> = Vec::new();
    let mut rowvars: Vec<RowVar> = Vec::new();
    free_vars_stack(&arrow.input, &mut tyvars, &mut rowvars);
    free_vars_stack(&arrow.output, &mut tyvars, &mut rowvars);
    tyvars.sort_unstable();
    tyvars.dedup();
    rowvars.sort_unstable();
    rowvars.dedup();
    Scheme::new(tyvars, rowvars, arrow.clone())
}

/// Accumulate the free type and row variables of a (resolved) stack type.
fn free_vars_stack(s: &StackTy, tyvars: &mut Vec<TyVar>, rowvars: &mut Vec<RowVar>) {
    rowvars.push(s.row);
    for e in &s.elems {
        free_vars_ty(e, tyvars, rowvars);
    }
}

/// Accumulate the free type and row variables of a (resolved) element type,
/// recursing uniformly through `Quote` arrow interiors.
fn free_vars_ty(ty: &Ty, tyvars: &mut Vec<TyVar>, rowvars: &mut Vec<RowVar>) {
    match &ty.kind {
        TyKind::Var(v) => tyvars.push(*v),
        TyKind::Con(_) => {}
        TyKind::App(_, args) => {
            for a in args {
                free_vars_ty(a, tyvars, rowvars);
            }
        }
        TyKind::Quote(w) => {
            free_vars_stack(&w.input, tyvars, rowvars);
            free_vars_stack(&w.output, tyvars, rowvars);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QuoteItem;
    use crate::Token;
    use crate::parse;
    use crate::parse_with_spans;
    use crate::types::BOOL;
    use crate::types::NUM;
    use crate::types::Scheme;
    use crate::types::StackTy;
    use crate::types::Ty;
    use crate::types::TyKind;

    /// A minimal stack value type for driver tests.
    #[derive(Debug, Clone, PartialEq)]
    enum Value {
        Word(String),
        Bracket(Vec<QuoteItem<Value>>),
    }

    impl From<Token> for Value {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => Value::Word(w),
                Token::Bracket(b) => Value::Bracket(crate::quote_items_from_tokens(&b)),
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
                Value::Bracket(b) => vec![Token::Bracket(crate::quote_items_to_tokens(b))],
            }
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            match self {
                Value::Bracket(b) => Some(crate::quote_items_to_values(b)),
                Value::Word(_) => None,
            }
        }

        fn from_sequence(elements: Vec<Self>) -> Self {
            Value::Bracket(elements.into_iter().map(QuoteItem::Push).collect())
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
        let def_env = DefEnv::empty();
        let no_poly: HashMap<String, Scheme> = HashMap::new();
        let arrow = infer_seq(
            &eval,
            &tokens,
            &mut ctx,
            &mut locals,
            &def_env,
            &no_poly,
            false,
        )?;
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
            matches!(err, TypeError::Mismatch { .. }),
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

    // ----- Locals captured into quotations (regression vs M0; finding 1) -----

    #[test]
    fn quotation_references_an_enclosing_local() {
        // `[ 5 >x [ x ] CALL ] :main` must type-check: the runtime captures `x`
        // into the inner quotation by value (evaluator.rs::capture_body), so the
        // checker must make the enclosing local visible inside the quotation.
        // Before this fix the inner `x` was rejected as UnresolvedWord.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ 5 >x [ x ] CALL ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        type_check(&eval).expect("a quotation referencing an enclosing local type-checks");
    }

    #[test]
    fn captured_local_resolves_to_the_bindings_monomorphic_type() {
        // `>x [ x ] CALL`: the popped value is a fresh monomorphic `t`; the
        // captured `x` inside the quotation must resolve to the SAME `t`, so the
        // snippet threads one variable through: ( 'a t -- 'a t ).
        let arrow = infer_snippet(">x [ x ] CALL").unwrap();
        assert_eq!(arrow.input.elems.len(), 1, "consumes the bound value");
        assert_eq!(arrow.output.elems.len(), 1, "re-produces it via the quote");
        let inv = &arrow.input.elems[0].kind;
        assert!(
            matches!(inv, TyKind::Var(_)),
            "the binding stays a monomorphic var"
        );
        assert_eq!(
            arrow.output.elems[0].kind, *inv,
            "the captured use resolves to the SAME variable as the binding"
        );
    }

    #[test]
    fn inner_binder_shadows_a_captured_local() {
        // `true >x [ 5 >x x ] CALL`: the outer `x` is a Bool, but the inner
        // `>x` rebinds `x` to a Num and the inner `x` must resolve to that Num
        // (innermost binding wins). The whole snippet therefore produces a Num;
        // were shadowing broken (outer wins) it would produce a Bool.
        let arrow = infer_snippet("true >x [ 5 >x x ] CALL").unwrap();
        assert!(arrow.input.elems.is_empty(), "consumes nothing net");
        assert_eq!(arrow.output.elems.len(), 1);
        assert_eq!(
            arrow.output.elems[0].kind,
            TyKind::Con(NUM.into()),
            "the inner >x shadows the captured Bool with a Num"
        );
    }

    // ----- §12 M3 acceptance: definitions / SCC generalization -----

    /// Run the SCC generalization pass over a program's definitions (with `+`
    /// registered) and return the per-definition schemes. This is the M3
    /// build-order unit: it validates every definition body and generalizes it,
    /// independent of any `main` entry.
    fn check_defs(src: &str) -> Result<HashMap<String, Scheme>, TypeError> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let mut ctx = InferCtx::new();
        infer_definition_schemes(&eval, &mut ctx)
    }

    #[test]
    fn m3_order_independence_use_before_define() {
        // `:foo` references `bar` and is defined BEFORE `bar`. The flat global
        // pre-pass + SCC processing in reverse-topological order makes this
        // type-check regardless of textual order.
        let schemes = check_defs("[ bar 1 + ] :foo [ 2 ] :bar").unwrap();
        assert!(schemes.contains_key("foo"));
        assert!(schemes.contains_key("bar"));
        // `foo` produces a Num ( 'a -- 'a Num ).
        let foo = &schemes["foo"];
        assert_eq!(foo.ty.output.elems.len(), 1);
        assert_eq!(foo.ty.output.elems[0].kind, TyKind::Con(NUM.into()));
    }

    #[test]
    fn m3_mutual_recursion_type_checks_as_one_scc() {
        // `even`/`odd` reference each other: a single strongly-connected
        // component, inferred monomorphically. The M2 stopgap rejected ALL such
        // references as RecursiveDefinition; the SCC pass accepts them.
        let schemes = check_defs("[ odd ] :even [ even ] :odd").unwrap();
        assert!(schemes.contains_key("even"));
        assert!(schemes.contains_key("odd"));
    }

    #[test]
    fn m3_nonrecursive_helper_used_at_two_distinct_types() {
        // A generalized helper `id : ( 'a -- 'a )` used by one caller at Num and
        // another at Bool. Generalization (instantiate-per-call-site) lets both
        // type-check; a single shared monomorphic type could not satisfy both.
        let schemes = check_defs("[ ] :id [ 1 id ] :usenum [ true id ] :usebool").unwrap();
        // `id` is generalized: it quantifies over its row variable(s).
        let id = &schemes["id"];
        assert!(
            !id.rowvars.is_empty(),
            "the helper is generalized over its row tail, not monomorphic"
        );
        // Each caller instantiated `id` at its own concrete type.
        let usenum = &schemes["usenum"];
        assert_eq!(usenum.ty.output.elems.len(), 1);
        assert_eq!(usenum.ty.output.elems[0].kind, TyKind::Con(NUM.into()));
        let usebool = &schemes["usebool"];
        assert_eq!(usebool.ty.output.elems.len(), 1);
        assert_eq!(usebool.ty.output.elems[0].kind, TyKind::Con(BOOL.into()));
    }

    #[test]
    fn m3_polymorphic_recursion_yields_the_section6_diagnostic() {
        // `[ DUP foo ] :foo` calls itself at a stack one element larger than its
        // own — a self-call at a non-unifying type. Tier-0 cannot infer this; it
        // must be rejected with the §6 polymorphic-recursion message, NOT a raw
        // unification or cyclic-type error.
        let err = check_defs("[ DUP foo ] :foo").unwrap_err();
        match &err {
            TypeError::PolymorphicRecursion { name, .. } => assert_eq!(name, "foo"),
            other => panic!("expected PolymorphicRecursion for foo, got {other:?}"),
        }
        let text = err.to_string();
        assert!(
            text.contains("polymorphic recursion") && text.contains("type annotation"),
            "the §6 diagnostic must name the failure and ask for an annotation: {text}"
        );
        assert!(!text.contains('\''), "no internal variable names: {text}");
    }

    // ----- §12 M4 acceptance: combinators + rank-2 -----

    /// A `List` element type (the App constructor exercised by the §12 M4 `push`
    /// snippet).
    fn list_ty(s: Span) -> Ty {
        Ty {
            kind: TyKind::App("List".to_string(), Vec::new()),
            span: s,
        }
    }

    #[test]
    fn m4_core_scheme_returns_the_dip_scheme() {
        // DIP : ( 'S a ( 'S -- 'T ) -- 'T a ). The set-aside `a` sits below the
        // quotation on input and is carried through unchanged on output; the
        // output row 'T (the quotation's result) differs from the input row 'S.
        let scheme = core_scheme("DIP").expect("DIP has a core scheme at M4");
        assert_eq!(
            scheme.ty.input.elems.len(),
            2,
            "input is `a` then the quote"
        );
        assert!(matches!(scheme.ty.input.elems[0].kind, TyKind::Var(_)));
        assert!(matches!(scheme.ty.input.elems[1].kind, TyKind::Quote(_)));
        assert_eq!(scheme.ty.output.elems.len(), 1, "output carries `a`");
        // Same variable in and out: the set-aside value returns unchanged.
        assert_eq!(
            scheme.ty.input.elems[0].kind,
            scheme.ty.output.elems[0].kind
        );
        assert_ne!(
            scheme.ty.input.row, scheme.ty.output.row,
            "output row is 'T, input row is 'S"
        );
        assert!(core_scheme("DIP").is_some());
    }

    #[test]
    fn m4_core_schemes_cover_fixed_stack_combinators() {
        for word in [
            "ROT", "-ROT", "NIP", "TUCK", "2DUP", "2DROP", "2SWAP", "2OVER", "2ROT",
        ] {
            assert!(
                core_scheme(word).is_some(),
                "missing core scheme for {word}"
            );
        }

        let arrow = infer_snippet(
            "1 2 3 -ROT DROP DROP DROP \
             1 2 3 4 5 6 2ROT 2DROP 2DROP 2DROP",
        )
        .unwrap();
        assert!(arrow.input.elems.is_empty());
        assert!(arrow.output.elems.is_empty());
    }

    #[test]
    fn m4_core_schemes_cover_fixed_quotation_combinators() {
        for word in [
            "2DIP", "3DIP", "KEEP", "2KEEP", "3KEEP", "BI", "BI*", "BI@", "TRI", "TRI*", "TRI@",
            "COMPOSE", "CURRY", "2CURRY", "3CURRY",
        ] {
            assert!(
                core_scheme(word).is_some(),
                "missing core scheme for {word}"
            );
        }

        let arrow = infer_snippet(
            "1 2 3 4 [ + ] 2DIP DROP DROP DROP \
             1 2 3 4 5 [ + ] 3DIP DROP DROP DROP DROP \
             5 [ 1 + ] KEEP DROP DROP \
             1 2 [ + ] 2KEEP DROP DROP DROP \
             1 2 3 [ + + ] 3KEEP DROP DROP DROP DROP \
             5 [ 1 + ] [ 2 + ] BI DROP DROP \
             5 6 [ 1 + ] [ 2 + ] BI* DROP DROP \
             5 6 [ 1 + ] BI@ DROP DROP \
             5 [ 1 + ] [ 2 + ] [ 3 + ] TRI DROP DROP DROP \
             5 6 7 [ 1 + ] [ 2 + ] [ 3 + ] TRI* DROP DROP DROP \
             5 6 7 [ 1 + ] TRI@ DROP DROP DROP \
             [ 1 + ] [ 2 + ] COMPOSE 5 SWAP CALL DROP \
             10 [ + ] CURRY 5 SWAP CALL DROP \
             10 20 [ + + ] 2CURRY 5 SWAP CALL DROP \
             1 2 3 [ + + + ] 3CURRY 4 SWAP CALL DROP",
        )
        .unwrap();
        assert!(arrow.input.elems.is_empty());
        assert!(arrow.output.elems.is_empty());
    }

    #[test]
    fn m4_dip_relays_quotation_and_carries_the_set_aside_value() {
        // §12 M4: `xs total [ 99 push ] DIP` type-checks; result shape `… List
        // Num`; DIP contributes ONLY that `total` is carried through. `DIP` is
        // typed from its declared scheme — the quotation body is not expanded.
        let s = sp();
        let mut eval: Evaluator<Value> = Evaluator::new();
        // push : ( 'S List Num -- 'S List )
        eval.register_operator_with_contract(
            "push",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(
                    StackTy::new(vec![list_ty(s), Ty::num(s)], 0, s),
                    StackTy::new(vec![list_ty(s)], 0, s),
                ),
            ),
        );
        // xs : ( 'S -- 'S List ) and total : ( 'S -- 'S Num ) seed the stack.
        eval.register_operator_with_contract(
            "xs",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(StackTy::empty(0, s), StackTy::new(vec![list_ty(s)], 0, s)),
            ),
        );
        eval.register_operator_with_contract(
            "total",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(StackTy::empty(0, s), StackTy::new(vec![Ty::num(s)], 0, s)),
            ),
        );

        let tokens = parse_with_spans("xs total [ 99 push ] DIP").unwrap();
        let mut ctx = InferCtx::new();
        let mut locals: Vec<Local> = Vec::new();
        let def_env = DefEnv::empty();
        let no_poly: HashMap<String, Scheme> = HashMap::new();
        let arrow = infer_seq(
            &eval,
            &tokens,
            &mut ctx,
            &mut locals,
            &def_env,
            &no_poly,
            false,
        )
        .unwrap();
        let arrow = ctx.resolve_word_deep(&arrow);

        assert!(arrow.input.elems.is_empty(), "consumes nothing from below");
        assert_eq!(arrow.output.elems.len(), 2, "result shape is `List Num`");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert!(args.is_empty());
            }
            other => panic!("expected a List below, got {other:?}"),
        }
        // The set-aside `total` (a Num) is carried through unchanged on top.
        assert_eq!(arrow.output.elems[1].kind, TyKind::Con(NUM.into()));
    }

    /// `List a` element type with element `elem` (the higher-order MAP/FILTER
    /// signatures track the element type, unlike the nullary `list_ty` above).
    fn list_of(elem: Ty, s: Span) -> Ty {
        Ty::app("List", vec![elem], s)
    }

    fn assert_list_num_depth(ty: &Ty, depth: usize) {
        if depth == 0 {
            assert_eq!(ty.kind, TyKind::Con(NUM.into()), "leaf element is Num");
            return;
        }
        match &ty.kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(args.len(), 1, "List carries exactly one element type");
                assert_list_num_depth(&args[0], depth - 1);
            }
            other => panic!("expected List nesting depth {depth}, got {other:?}"),
        }
    }

    /// Seed an evaluator with a `List Num` source and the element words the
    /// MAP/FILTER acceptance tests apply through a quotation.
    fn eval_with_list_words(s: Span) -> Evaluator<Value> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        // nums : ( 'S -- 'S (List Num) )
        eval.register_operator_with_contract(
            "nums",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(
                    StackTy::empty(0, s),
                    StackTy::new(vec![list_of(Ty::num(s), s)], 0, s),
                ),
            ),
        );
        // inc : ( 'S Num -- 'S Num ) — an element transform Num -> Num.
        eval.register_operator_with_contract(
            "inc",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(
                    StackTy::new(vec![Ty::num(s)], 0, s),
                    StackTy::new(vec![Ty::num(s)], 0, s),
                ),
            ),
        );
        // is_pos : ( 'S Num -- 'S Bool ) — a predicate Num -> Bool.
        eval.register_operator_with_contract(
            "is_pos",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(
                    StackTy::new(vec![Ty::num(s)], 0, s),
                    StackTy::new(vec![Ty::bool(s)], 0, s),
                ),
            ),
        );
        eval
    }

    /// Infer a token sequence's whole effect under `eval`, fully resolved.
    fn infer_effect(eval: &Evaluator<Value>, src: &str) -> Result<WordTy, TypeError> {
        let tokens = parse_with_spans(src).unwrap();
        let mut ctx = InferCtx::new();
        let mut locals: Vec<Local> = Vec::new();
        let def_env = DefEnv::empty();
        let no_poly: HashMap<String, Scheme> = HashMap::new();
        infer_seq(
            eval,
            &tokens,
            &mut ctx,
            &mut locals,
            &def_env,
            &no_poly,
            false,
        )
        .map(|arrow| ctx.resolve_word_deep(&arrow))
    }

    #[test]
    fn map_is_a_core_scheme_not_an_unresolved_word() {
        // Before the fix `MAP` had no checker scheme and failed as UNDEFINED
        // (effect lookup absent). It must now resolve as a language-core
        // primitive.
        assert!(core_scheme("MAP").is_some(), "MAP must have a core scheme");
        assert!(
            core_scheme("FILTER").is_some(),
            "FILTER must have a core scheme"
        );
    }

    #[test]
    fn map_relays_the_quotation_and_preserves_the_list() {
        // `nums [ inc ] MAP` : ( 'S -- 'S (List Num) ). The Num->Num element
        // transform relays through; the result is a `List Num`.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "nums [ inc ] MAP").expect("MAP must type-check");
        assert!(arrow.input.elems.is_empty(), "consumes nothing from below");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(args.len(), 1, "List carries its element type");
                assert_eq!(args[0].kind, TyKind::Con(NUM.into()), "element is Num");
            }
            other => panic!("expected `List Num`, got {other:?}"),
        }
    }

    #[test]
    fn literal_bracket_coerces_to_list_for_map() {
        // The dual-purpose tag (§8): a quotation literal of pure monomorphic
        // numeric data stands in for `List Num`, so `[ 1 2 3 ] [ inc ] MAP`
        // type-checks even though brackets are represented as quotations.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "[ 1 2 3 ] [ inc ] MAP").expect("literal list must MAP");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(args[0].kind, TyKind::Con(NUM.into()), "element is Num");
            }
            other => panic!("expected `List Num`, got {other:?}"),
        }
    }

    #[test]
    fn nested_literal_bracket_coerces_to_list_of_lists_for_map() {
        // Nested literal data is tagged recursively: `[ [ 1 2 ] [ 3 4 ] ]`
        // stands in for `List (List Num)`. The outer MAP consumes each inner
        // `List Num`, and its quotation maps `inc` across that inner list.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "[ [ 1 2 ] [ 3 4 ] ] [ [ inc ] MAP ] MAP")
            .expect("literal list of lists must MAP");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        assert_list_num_depth(&arrow.output.elems[0], 2);
    }

    #[test]
    fn triple_nested_literal_bracket_maps_recursively() {
        // The same recursive literal tag supports `List (List (List Num))`;
        // this is not a special `List2` case.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(
            &eval,
            "[ [ [ 1 ] [ 2 ] ] [ [ 3 ] [ 4 ] ] ] [ [ [ inc ] MAP ] MAP ] MAP",
        )
        .expect("literal list of lists of lists must MAP");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        assert_list_num_depth(&arrow.output.elems[0], 3);
    }

    #[test]
    fn empty_nested_literal_list_unifies_with_non_empty_sibling() {
        // Empty lists remain polymorphic at any nesting level: the first inner
        // `[]` is fixed to `List Num` by its non-empty sibling and the mapping
        // quotation.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "[ [ ] [ 1 2 ] ] [ [ inc ] MAP ] MAP")
            .expect("empty nested list should be fixed by sibling element type");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        assert_list_num_depth(&arrow.output.elems[0], 2);
    }

    #[test]
    fn empty_bracket_is_a_polymorphic_list() {
        // The empty list is mappable at every element type (`∀a. List a`): the
        // fresh tag unifies with the transform's input, so `[ ] [ inc ] MAP`
        // yields `List Num`. This matches the runtime, which maps zero times and
        // returns the empty list.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "[ ] [ inc ] MAP").expect("empty list must MAP");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(
                    args[0].kind,
                    TyKind::Con(NUM.into()),
                    "element fixed to Num by the transform"
                );
            }
            other => panic!("expected `List Num`, got {other:?}"),
        }
    }

    #[test]
    fn heterogenous_nested_literal_bracket_does_not_coerce() {
        // The recursive tag still requires one monomorphic element type. A list
        // containing both `List Num` and `List Bool` does not stand in for a
        // `List` demanded by MAP.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "[ [ 1 ] [ true ] ] [ [ inc ] MAP ] MAP")
            .expect_err("heterogeneous list-of-lists must not pass as a List");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "expected a type mismatch, got {err:?}"
        );
    }

    #[test]
    fn nested_computing_bracket_does_not_coerce_to_a_list() {
        // Soundness guard at depth > 1: an inner bracket that computes is not
        // literal data, so the outer bracket cannot become `List (List _)`.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "[ [ 1 inc ] ] [ [ inc ] MAP ] MAP")
            .expect_err("a nested computing bracket must not pass as a List");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "expected a type mismatch, got {err:?}"
        );
    }

    #[test]
    fn boolean_literal_bracket_coerces_to_list_bool() {
        // The tag monomorphizes on the boolean base type too: `[ true false ]`
        // coerces to `List Bool`, which `[ is_pos ]` (Num -> Bool) then rejects
        // because the element is Bool, not Num — a *correct* rejection, proving
        // the coercion carries the real element type rather than waving it past.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "[ true false ] [ is_pos ] MAP")
            .expect_err("Bool element cannot feed a Num -> Bool transform");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "expected a type mismatch, got {err:?}"
        );
    }

    #[test]
    fn computing_bracket_does_not_coerce_to_a_list() {
        // Soundness guard: `[ 1 inc ]` *computes* (it is not all-literals), so it
        // carries no tag and stays a quotation. Feeding it where a `List` is
        // demanded must fail — otherwise the checker would accept a program the
        // runtime's element-wise `as_sequence` would mis-iterate.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "[ 1 inc ] [ inc ] MAP")
            .expect_err("a computing bracket must not pass as a List");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "expected a type mismatch, got {err:?}"
        );
    }

    #[test]
    fn non_monomorphic_bracket_does_not_coerce() {
        // `[ 1 true ]` mixes a numeric and a boolean literal, so it does not
        // monomorphize to a single element type and earns no tag. It cannot
        // stand in for a `List`.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "[ 1 true ] [ inc ] MAP")
            .expect_err("a heterogeneous bracket must not pass as a List");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "expected a type mismatch, got {err:?}"
        );
    }

    #[test]
    fn tagged_bracket_still_serves_as_a_code_quotation() {
        // Dual purpose is preserved: a tagged literal bracket is still a
        // quotation, so `IF`'s branch slots (which demand `( 'S -- 'T )` arrows,
        // not lists) accept it exactly as before. `true [ 1 ] [ 2 ] IF : Num`.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow =
            infer_effect(&eval, "true [ 1 ] [ 2 ] IF").expect("IF branches stay quotations");
        assert_eq!(arrow.output.elems.len(), 1);
        assert_eq!(
            arrow.output.elems[0].kind,
            TyKind::Con(NUM.into()),
            "IF yields the branch's Num"
        );
    }

    #[test]
    fn map_changes_the_element_type_through_the_quotation() {
        // `nums [ is_pos ] MAP` : the Num->Bool quotation makes the output a
        // `List Bool` — the quotation's output element `b` becomes the result
        // list's element (the §8 higher-order relay).
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "nums [ is_pos ] MAP").expect("MAP must type-check");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(
                    args[0].kind,
                    TyKind::Con(BOOL.into()),
                    "element became Bool"
                );
            }
            other => panic!("expected `List Bool`, got {other:?}"),
        }
    }

    #[test]
    fn filter_keeps_the_element_type_and_takes_a_predicate() {
        // `nums [ is_pos ] FILTER` : ( 'S -- 'S (List Num) ). The Num->Bool
        // predicate relays; the element type is unchanged (List Num in, List Num
        // out) — no post-filter length claim.
        let s = sp();
        let eval = eval_with_list_words(s);
        let arrow = infer_effect(&eval, "nums [ is_pos ] FILTER").expect("FILTER must type-check");
        assert_eq!(arrow.output.elems.len(), 1, "result is a single List");
        match &arrow.output.elems[0].kind {
            TyKind::App(n, args) => {
                assert_eq!(n, "List");
                assert_eq!(args[0].kind, TyKind::Con(NUM.into()), "element stays Num");
            }
            other => panic!("expected `List Num`, got {other:?}"),
        }
    }

    #[test]
    fn sequence_relays_accept_nested_list_elements() {
        // FILTER/FOLD/EACH are generic over `List a`, so once recursive literal
        // tagging gives us `List (List Num)`, the existing schemes should relay
        // that nested element type without extra `List2` variants.
        let s = sp();
        let eval = eval_with_list_words(s);

        let filtered = infer_effect(&eval, "[ [ 1 ] [ 2 ] ] [ DROP true ] FILTER")
            .expect("FILTER accepts a predicate over inner lists");
        assert_eq!(filtered.output.elems.len(), 1);
        assert_list_num_depth(&filtered.output.elems[0], 2);

        let folded = infer_effect(&eval, "[ [ 1 ] [ 2 ] ] 0 [ DROP inc ] FOLD")
            .expect("FOLD accepts a step over inner lists");
        assert_eq!(folded.output.elems.len(), 1);
        assert_eq!(
            folded.output.elems[0].kind,
            TyKind::Con(NUM.into()),
            "FOLD returns the numeric accumulator"
        );

        let each = infer_effect(&eval, "[ [ 1 ] [ 2 ] ] [ DROP ] EACH")
            .expect("EACH accepts a consumer over inner lists");
        assert!(
            each.output.elems.is_empty(),
            "EACH consumes the nested list"
        );
    }

    #[test]
    fn map_rejects_an_element_transform_of_the_wrong_input_type() {
        // The input list's element relays into the quotation's input: a Num->Bool
        // quotation cannot map a `List Bool` source whose element a = Bool against
        // `inc : Num -> Num`. Build a `List Bool` source and apply `inc`.
        let s = sp();
        let mut eval = eval_with_list_words(s);
        // bools : ( 'S -- 'S (List Bool) )
        eval.register_operator_with_contract(
            "bools",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(
                    StackTy::empty(0, s),
                    StackTy::new(vec![list_of(Ty::bool(s), s)], 0, s),
                ),
            ),
        );
        let err = infer_effect(&eval, "bools [ inc ] MAP")
            .expect_err("element type must relay: Bool list vs Num transform is a mismatch");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "wrong element type is a typed mismatch, got {err:?}"
        );
    }

    #[test]
    fn filter_rejects_a_non_boolean_predicate() {
        // FILTER's quotation must produce Bool. An `inc : Num -> Num` quotation is
        // not a predicate and must be rejected.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "nums [ inc ] FILTER")
            .expect_err("a non-Bool-producing quotation is not a predicate");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "non-predicate quotation is a typed mismatch, got {err:?}"
        );
    }

    // ----------------------------------------------------------------------
    // The remaining runtime builtins given Tier-0 contracts (mirrors MAP/FILTER):
    // the applicative relays KEEP/BI/BI*/BI@, the quotation-builders
    // COMPOSE/CURRY, the one-armed conditionals WHEN/UNLESS, and the sequence
    // relays FOLD/EACH.
    // ----------------------------------------------------------------------

    /// Assert the whole effect of `src` lands `outs` `Num`s on top of an
    /// otherwise-untouched base stack ( 'S -- 'S Num^outs ).
    fn assert_pushes_nums(eval: &Evaluator<Value>, src: &str, outs: usize) {
        let arrow = infer_effect(eval, src).unwrap_or_else(|e| panic!("{src:?} must check: {e:?}"));
        assert!(
            arrow.input.elems.is_empty(),
            "{src:?} consumes nothing from below, got {:?}",
            arrow.input.elems
        );
        assert_eq!(
            arrow.output.elems.len(),
            outs,
            "{src:?} should leave {outs} values"
        );
        for (i, e) in arrow.output.elems.iter().enumerate() {
            assert_eq!(
                e.kind,
                TyKind::Con(NUM.into()),
                "{src:?} output {i} should be Num"
            );
        }
    }

    #[test]
    fn newly_typed_builtins_have_a_core_scheme() {
        for name in [
            "KEEP", "BI", "BI*", "BI@", "COMPOSE", "CURRY", "2CURRY", "3CURRY", "WHEN", "UNLESS",
            "FOLD", "EACH",
        ] {
            assert!(
                core_scheme(name).is_some(),
                "{name} must resolve as a language-core primitive"
            );
        }
    }

    #[test]
    fn applicative_relays_thread_their_quotations() {
        let s = sp();
        let eval = eval_with_list_words(s);
        // KEEP runs `inc` on the value and restores a copy: ( Num -- Num Num ).
        assert_pushes_nums(&eval, "5 [ inc ] KEEP", 2);
        // BI applies two quotations to one value; BI* to two values; both relay.
        assert_pushes_nums(&eval, "5 [ inc ] [ inc ] BI", 2);
        assert_pushes_nums(&eval, "5 6 [ inc ] [ inc ] BI*", 2);
        // BI@ applies one element-transform to two same-typed values: ( -- b b ).
        assert_pushes_nums(&eval, "5 6 [ inc ] BI@", 2);
    }

    #[test]
    fn quotation_builders_compose_and_curry() {
        let s = sp();
        let eval = eval_with_list_words(s);
        // COMPOSE chains [inc] then [inc]; CALL runs the composite: ( -- Num ).
        assert_pushes_nums(&eval, "5 [ inc ] [ inc ] COMPOSE CALL", 1);
        // CURRY bakes 5 into [inc]; CALL runs the closed quotation: ( -- Num ).
        assert_pushes_nums(&eval, "5 [ inc ] CURRY CALL", 1);
    }

    #[test]
    fn one_armed_conditionals_require_a_stack_neutral_quotation() {
        let s = sp();
        let eval = eval_with_list_words(s);
        // An empty (identity) quotation is `'S -- 'S`; both paths agree.
        assert_pushes_nums(&eval, "5 true [ ] WHEN", 1);
        assert_pushes_nums(&eval, "5 false [ ] UNLESS", 1);
    }

    #[test]
    fn when_rejects_a_shape_changing_quotation() {
        // The absent branch is the identity, so a quotation that changes the
        // stack shape (here `[ DROP ]` shrinks it) cannot agree with the
        // do-nothing path.
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "5 true [ DROP ] WHEN")
            .expect_err("a stack-shrinking quotation is not `'S -- 'S`");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "non-neutral WHEN quotation is a typed rejection, got {err:?}"
        );
    }

    #[test]
    fn fold_threads_an_accumulator_and_each_consumes_for_effect() {
        let s = sp();
        let eval = eval_with_list_words(s);
        // FOLD: `[ DROP ]` is a valid step ( 'r b a -- 'r b ) (drops the element,
        // keeps the accumulator). Result is the final accumulator: ( -- Num ).
        assert_pushes_nums(&eval, "nums 0 [ DROP ] FOLD", 1);
        // EACH: `[ DROP ]` is a stack-neutral consumer ( 'r a -- 'r ). The list
        // is consumed and the base stack returns unchanged: ( -- ).
        assert_pushes_nums(&eval, "nums [ DROP ] EACH", 0);
    }

    #[test]
    fn fold_rejects_a_step_that_does_not_preserve_the_accumulator() {
        // An identity step `[ ]` ( 'R -- 'R ) cannot be a fold step, which must
        // consume one element while preserving the accumulator ( 'r b a -- 'r b ).
        let s = sp();
        let eval = eval_with_list_words(s);
        let err = infer_effect(&eval, "nums 0 [ ] FOLD")
            .expect_err("an identity step does not consume the element");
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "a non-consuming FOLD step is a typed rejection, got {err:?}"
        );
    }

    #[test]
    fn m4_rank2_without_annotation_yields_the_section8_diagnostic() {
        // `>f` binds a quotation parameter applied at TWO distinct types (to a
        // Num and to a Bool). Rank-1 inference cannot type it; the checker must
        // emit the targeted §8 message, NOT a raw mismatch.
        let err = check_defs("[ >f 1 f CALL true f CALL ] :rank2").unwrap_err();
        match &err {
            TypeError::Rank2 { name, .. } => assert_eq!(name, "rank2"),
            other => panic!("expected Rank2 for rank2, got {other:?}"),
        }
        let text = err.to_string();
        assert!(
            text.contains("applies its quotation at more than one type")
                && text.contains("type annotation"),
            "the §8 diagnostic must name the failure and ask for an annotation: {text}"
        );
        assert!(!text.contains('\''), "no internal variable names: {text}");
    }

    #[test]
    fn m4_rank1_quotation_use_still_infers_without_annotation() {
        // A quotation parameter applied at ONE type is ordinary rank-1 inference:
        // it must type-check with no annotation and no §8 diagnostic.
        let schemes = check_defs("[ >f 1 f CALL ] :rank1").unwrap();
        assert!(schemes.contains_key("rank1"));
    }

    #[test]
    fn m4_rank2_with_a_correct_annotation_type_checks() {
        // The SAME rank-2 word, now annotated `( ( a -- ) -- )`: its quotation
        // parameter is declared polymorphic, so each application instantiates
        // fresh and the body checks against the declared signature (§8).
        let src = "[ [ a -- ] -- ] @rank2 [ >f 1 f CALL true f CALL ] :rank2";
        let schemes = check_defs(src).unwrap();
        assert!(
            schemes.contains_key("rank2"),
            "the annotated rank-2 word must check"
        );
    }

    #[test]
    fn m4_incorrect_annotation_is_a_mismatch_not_silently_accepted() {
        // An annotation that mistypes the parameter as `Num` (not a quotation)
        // must reject the body: applying a Num via `CALL` cannot unify. With an
        // annotation present we report the honest mismatch, not the §8 hint.
        let src = "[ Num -- ] @rank2 [ >f 1 f CALL true f CALL ] :rank2";
        let err = check_defs(src).unwrap_err();
        assert!(
            matches!(err, TypeError::Mismatch { .. }),
            "a body that contradicts its annotation is a mismatch, got {err:?}"
        );
    }

    #[test]
    fn m4_malformed_annotation_is_rejected() {
        // No `--` separator inside the effect bracket is a located annotation
        // error, not a panic.
        let src = "[ a b ] @bad [ >f ] :bad";
        let err = check_defs(src).unwrap_err();
        match &err {
            TypeError::BadAnnotation { name, .. } => assert_eq!(name, "bad"),
            other => panic!("expected BadAnnotation, got {other:?}"),
        }
    }

    #[test]
    fn tarjan_groups_cycles_and_emits_dependencies_first() {
        // Graph: 0→1, 1→2, 2→0 (a 3-cycle), 3→0 (depends on the cycle), 4 alone.
        // Expected SCCs: {0,1,2}, {3}, {4}; the cycle (a dependency of 3) must be
        // emitted BEFORE {3} (reverse-topological), and {4} stands alone.
        let adj = vec![vec![1], vec![2], vec![0], vec![0], vec![]];
        let order: Vec<Vec<usize>> = tarjan_sccs(&adj)
            .into_iter()
            .map(|mut c| {
                c.sort_unstable();
                c
            })
            .collect();
        assert_eq!(order.len(), 3, "exactly three components: {order:?}");
        assert!(
            order.contains(&vec![0, 1, 2]),
            "the cycle is one SCC: {order:?}"
        );
        assert!(order.contains(&vec![3]));
        assert!(order.contains(&vec![4]));

        // Dependencies-first: the cycle {0,1,2} is emitted before {3} (3 → 0).
        let idx_cycle = order.iter().position(|c| c == &vec![0, 1, 2]).unwrap();
        let idx_three = order.iter().position(|c| c == &vec![3]).unwrap();
        assert!(
            idx_cycle < idx_three,
            "the cycle (a dependency of 3) must be emitted first: {order:?}"
        );
    }

    #[test]
    fn m3_program_with_mutual_recursion_type_checks_via_driver() {
        // The whole-program driver accepts mutual recursion reached from `main`.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ even ] :main [ odd ] :even [ even ] :odd").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        type_check(&eval).expect("mutual recursion reachable from main type-checks");
    }

    // ----- §12 M5 acceptance: error quality (§7) -----

    #[test]
    fn m5_mismatch_reports_provenance_pair_and_innermost_frame_no_var_names() {
        // §7.1 + §7.3: a type mismatch must report BOTH origin spans (the
        // provenance pair) and anchor at the innermost frame where both
        // conflicting types are concrete — and must NOT leak internal variable
        // names (`'t7`, `'r3`).
        //
        // `[ [ 1 true + ] CALL ]`: inside the inner quotation `+` demands two
        // `Num`s but `true` produced a `Bool`. The contradiction is born inside
        // the inner frame; both frames enclose it, so localize-inward must anchor
        // at the INNER one (the user's real mistake), not the outer consequence.
        let err = infer_snippet("[ [ 1 true + ] CALL ]").unwrap_err();
        let TypeError::Mismatch { error, frames } = &err else {
            panic!("expected a typed mismatch, got {err:?}");
        };

        // Provenance pair: both birth spans are carried, and they differ (one is
        // the `Bool` from `true`, the other the `Num` demanded by `+`).
        let (ls, rs) = match error {
            UnifyError::Mismatch {
                left_span,
                right_span,
                ..
            } => (*left_span, *right_span),
            other => panic!("expected a Mismatch provenance pair, got {other:?}"),
        };
        assert_ne!(ls.start, rs.start, "the two origin spans must be distinct");

        // Localize inward must pick the innermost (deepest) of the two frames.
        let anchor = localize_inward(frames, ls, rs).expect("a frame encloses both");
        assert_eq!(
            anchor,
            frames.len() - 1,
            "the anchor must be the innermost enclosing frame, not the outer one"
        );

        // The rendered diagnostic shows the provenance pair AND marks the
        // innermost frame, and leaks no internal variable name (no apostrophes —
        // the user-facing text uses backticks, never `'t7`).
        let msg = format!("{err}");
        assert!(
            msg.contains(&format!("byte {}", ls.start))
                && msg.contains(&format!("byte {}", rs.start)),
            "both origin spans must appear: {msg}"
        );
        assert!(
            msg.contains("innermost frame carrying the contradiction"),
            "the innermost frame must be marked: {msg}"
        );
        assert!(
            !msg.contains('\''),
            "no internal variable name (no apostrophe) may leak: {msg}"
        );
    }

    #[test]
    fn m5_nested_quotation_renders_a_multi_level_breadcrumb() {
        // §7.2: a mismatch inside a NESTED quotation renders the frame
        // breadcrumb — a backtrace, not one opaque site. `[ [ 1 true + ] CALL ]`
        // is two frames deep, so the diagnostic must list BOTH "in the quotation
        // at byte N" lines, outermost first.
        let err = infer_snippet("[ [ 1 true + ] CALL ]").unwrap_err();
        let TypeError::Mismatch { frames, .. } = &err else {
            panic!("expected a typed mismatch, got {err:?}");
        };
        assert_eq!(frames.len(), 2, "two nested quotation frames: {frames:?}");

        let msg = format!("{err}");
        let crumb_lines = msg.matches("in the quotation at byte").count();
        assert_eq!(
            crumb_lines, 2,
            "the breadcrumb must render both frames as a backtrace: {msg}"
        );

        // Outermost first: the outer frame (smaller start) renders before the
        // inner one.
        let outer = frames[0].span.start;
        let inner = frames[1].span.start;
        assert!(outer < inner, "frames must be ordered outermost first");
        let outer_pos = msg
            .find(&format!("in the quotation at byte {outer}"))
            .unwrap();
        let inner_pos = msg
            .find(&format!("in the quotation at byte {inner}"))
            .unwrap();
        assert!(
            outer_pos < inner_pos,
            "the breadcrumb must read outermost-first: {msg}"
        );

        // And the inner (deepest) frame is the anchored one (§7.3).
        assert!(
            msg.contains(&format!(
                "in the quotation at byte {inner} (innermost frame carrying the contradiction)"
            )),
            "the inner frame is the innermost contradiction: {msg}"
        );
    }

    #[test]
    fn m5_localize_inward_skips_outer_frame_with_no_contradiction() {
        // §7.3 precision: localize_inward walks INWARD to the deepest frame still
        // enclosing both conflicting spans. Given two nested frames where both
        // enclose the contradiction, the inner one wins; a frame that does not
        // enclose both is never chosen.
        let outer = TypingFrame {
            span: Span { start: 0, end: 100 },
            expected: WordTy::new(StackTy::empty(0, sp()), StackTy::empty(0, sp())),
        };
        let inner = TypingFrame {
            span: Span { start: 40, end: 60 },
            expected: WordTy::new(StackTy::empty(1, sp()), StackTy::empty(1, sp())),
        };
        let frames = vec![outer, inner];
        let left = Span { start: 45, end: 46 };
        let right = Span { start: 50, end: 51 };
        // Both spans sit inside the inner frame: anchor must be index 1.
        assert_eq!(localize_inward(&frames, left, right), Some(1));

        // A contradiction that straddles the inner boundary (one span outside it)
        // can only be enclosed by the outer frame: anchor must be index 0.
        let straddle_left = Span { start: 10, end: 11 };
        let straddle_right = Span { start: 50, end: 51 };
        assert_eq!(
            localize_inward(&frames, straddle_left, straddle_right),
            Some(0)
        );

        // No frame encloses both: no anchor.
        let none_left = Span {
            start: 200,
            end: 201,
        };
        let none_right = Span { start: 50, end: 51 };
        assert_eq!(localize_inward(&frames, none_left, none_right), None);
    }

    #[test]
    fn m5_arity_underflow_names_word_and_gives_counts() {
        // §7 / §12 M5: an arity/underflow message names the offending word and
        // gives counts. Top-level `DROP` against the empty initial stack
        // underflows; the diagnostic blames `DROP` with expected/found counts and
        // leaks no internal variable name.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ DROP ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();
        let err = type_check(&eval).unwrap_err();
        match &err {
            TypeError::Arity {
                word,
                expected,
                found,
                ..
            } => {
                assert_eq!(word, "DROP", "the arity error must name the offending word");
                assert_eq!(*expected, 1, "it consumes one value");
                assert_eq!(*found, 0, "none were available below the empty floor");
            }
            other => panic!("expected an Arity underflow naming DROP, got {other:?}"),
        }
        let msg = format!("{err}");
        assert!(msg.contains("DROP"), "message names the word: {msg}");
        assert!(
            msg.contains('1') && msg.contains('0'),
            "message gives counts: {msg}"
        );
        assert!(
            !msg.contains('\''),
            "no internal variable name may leak: {msg}"
        );
    }

    #[test]
    fn check_is_the_pass_fail_ci_gate() {
        // §12 "Tier 0 done": the public `check` entry point is the CI-gate seam.
        // A well-typed whole program passes with `Ok(())`; an ill-typed one fails
        // with the same rich `TypeError` `type_check` would raise.
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("+", plus_scheme());
        let ok = parse_with_spans("[ 1 2 + ] :main").unwrap();
        eval.load_with_spans(&ok).unwrap();
        assert_eq!(check(&eval), Ok(()), "a checked program passes the gate");

        // An underflowing program is rejected by the gate with an Arity error.
        let mut bad: Evaluator<Value> = Evaluator::new();
        let prog = parse_with_spans("[ DROP ] :main").unwrap();
        bad.load_with_spans(&prog).unwrap();
        match check(&bad) {
            Err(TypeError::Arity { word, .. }) => assert_eq!(word, "DROP"),
            other => panic!("the gate must reject an underflow, got {other:?}"),
        }
    }

    // ----- §3 / invariant 10 (M6): refinement payload is inert at Tier 0 -----

    #[test]
    fn tier0_inference_ignores_an_attached_refinement() {
        // Build the SAME program twice — once with a refinement signature
        // attached to a definition, once without — and assert Tier 0 infers the
        // identical whole-program shape. The refinement is a forwarded payload
        // Tier 0 never reads (§3): attaching it must not perturb inference.
        let prog = "[ 1 2 + ] :sum  [ 3 sum ] :main";

        let mut bare: Evaluator<Value> = Evaluator::new();
        bare.register_operator_with_contract("+", plus_scheme());
        bare.load_with_spans(&parse_with_spans(prog).unwrap())
            .unwrap();
        let bare_effect = type_check(&bare).expect("bare program type-checks");

        let mut refined: Evaluator<Value> = Evaluator::new();
        refined.register_operator_with_contract("+", plus_scheme());
        refined
            .load_with_spans(&parse_with_spans(prog).unwrap())
            .unwrap();
        // Attach a refinement signature to `sum` via the separate infix parser.
        let sig = refined
            .attach_refinement("sum : ( a: Num b: Num -- s: Num where s = a + b )")
            .expect("refinement signature parses and attaches");
        assert_eq!(sig.name, "sum");
        assert!(refined.refinement("sum").is_some());
        let refined_effect = type_check(&refined).expect("refined program type-checks");

        // Identical shape: same inputs, same outputs, same row identity.
        assert_eq!(bare_effect.input.elems, refined_effect.input.elems);
        assert_eq!(bare_effect.output.elems, refined_effect.output.elems);
        assert_eq!(
            bare_effect.input.row == bare_effect.output.row,
            refined_effect.input.row == refined_effect.output.row
        );
        // And the inferred whole-program arrow carries NO refinement payload —
        // Tier 0 produced shape only; the attached signature stayed in its side
        // table, never read.
        assert!(refined_effect.refinement.is_none());
    }

    #[test]
    fn malformed_attached_refinement_is_a_located_error() {
        // The attachment surface surfaces a located parse error (§12 M6) rather
        // than panicking or silently dropping the signature.
        let mut eval: Evaluator<Value> = Evaluator::new();
        let err = eval
            .attach_refinement("sqrt : ( n: Num where n >=  --  r: Num )")
            .expect_err("malformed where must be a located parse error");
        assert!(err.span.start <= "sqrt : ( n: Num where n >=  --  r: Num )".len());
    }

    // -----------------------------------------------------------------------
    // The combined whole-program CI gate (`check_whole_program`): Tier 0 then,
    // only on green, Tier 1 + operator-axiom discharge, in one call (§10.10,
    // invariant 19/20).
    // -----------------------------------------------------------------------

    /// A neutral Tier-0 scheme of the given pop/push arity over `Num` —
    /// ( 'S Num…(pops) -- 'S Num…(pushes) ). Used to register the refined `sqrt`
    /// and the opaque producer the assume guards.
    fn num_scheme(pops: usize, pushes: usize) -> Scheme {
        let s = sp();
        let input = StackTy::new((0..pops).map(|_| Ty::num(s)).collect(), 0, s);
        let output = StackTy::new((0..pushes).map(|_| Ty::num(s)).collect(), 0, s);
        Scheme::new(vec![], vec![0], WordTy::new(input, output))
    }

    /// Build a small **assume-bearing refined whole program** loaded into an
    /// evaluator: `mk` is an opaque `( -- Num )` producer; `sqrt` carries a
    /// Tier-0 arrow ( Num -- Num ) and a refinement demanding `n >= 0`; `foo`
    /// produces an opaque value, asserts `result >= 0` over it, and feeds it to
    /// `sqrt`; `main` calls `foo`. The `assume` is legal (a genuinely opaque
    /// dependency it cannot prove otherwise), so the Tier-1 ledger records `foo`
    /// verified modulo `{ result >= 0 }`.
    fn assume_bearing_program() -> Evaluator<Value> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("mk", num_scheme(0, 1));
        eval.register_operator_with_contract("sqrt", num_scheme(1, 1));
        eval.attach_refinement("sqrt : ( n: Num where n >= 0  --  r: Num )")
            .expect("sqrt refinement attaches");
        let src = "[ mk \"assume(result >= 0)\" sqrt DROP ] :foo [ foo ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        eval
    }

    #[test]
    fn gate_runs_tier0_then_tier1_and_returns_the_ledger() {
        // The single act: Tier 0 (shape) THEN Tier 1 (+ axiom discharge), one
        // call, unified outcome (§10.10 / invariant 20). The returned Ledger is
        // the Tier-1 ledger, enumerating the user trusted base.
        let eval = assume_bearing_program();
        let ledger = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect("Tier 0 green, so the gate runs Tier 1 and returns its ledger");

        // The one accepted assumption is in the ledger, attributed to `foo`.
        assert_eq!(
            ledger.grep_assume(),
            vec!["assume(result >= 0)".to_string()]
        );
        assert!(ledger.is_clean(), "rejections: {:?}", ledger.rejections());
        assert!(
            ledger.status("foo").is_modulo(),
            "foo is verified modulo its assume: {}",
            ledger.status("foo")
        );
    }

    #[test]
    fn gate_fails_closed_on_an_illegal_assume() {
        // §10.7 HARD ERROR / invariants 13+20: `check_program` records an illegal
        // `assume` into the ledger's rejections and still returns Ok, so the gate
        // must inspect the ledger and FAIL CLOSED — never return Ok on a non-clean
        // ledger. Here `foo` asserts `assume(1 >= 0)`: a concrete goal with no
        // opaque dependency in its chain, which strict legality rejects.
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("sqrt", num_scheme(1, 1));
        eval.attach_refinement("sqrt : ( n: Num where n >= 0  --  r: Num )")
            .expect("sqrt refinement attaches");
        // `1` is a concrete Num — no opaque/uncontracted value sits in the
        // obligation's chain, so the assume is illegal (ASSUME_NO_OPAQUE_MSG).
        let src = "[ 1 \"assume(1 >= 0)\" sqrt DROP ] :foo [ foo ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();

        let err = check_whole_program(&eval, crate::SmtLibSolver::new).expect_err(
            "an illegal assume is a §10.7 hard error: the gate must fail closed, not return Ok",
        );

        match err {
            GateError::Tier1Rejected(rejections) => {
                assert_eq!(
                    rejections.len(),
                    1,
                    "exactly one illegal assume was rejected"
                );
                let reason = rejections[0]
                    .legality
                    .message()
                    .expect("a rejected assume carries a hard-error reason");
                assert!(
                    reason == crate::ASSUME_NO_OPAQUE_MSG || reason == crate::ASSUME_PROVABLE_MSG,
                    "the rejection reason is one of the §10.7 hard-error messages, got: {reason}"
                );
            }
            other => panic!(
                "the gate must fail closed with Tier1Rejected carrying the reason, got: {other}"
            ),
        }
    }

    #[test]
    fn gate_fails_closed_on_a_violated_obligation() {
        // §10.7 SITUATION B / §10.5 M9 / invariant 20: an opaque producer feeds an
        // unconstrained value into a refined demand with NO assume to cover it, so
        // the VC `true => n >= 0` is `Sat` (refuted) with a counterexample. `verify`
        // records the violated obligation but does NOT error — discharge-checking is
        // the gate's job — so the gate must inspect the obligation stream and FAIL
        // CLOSED, surfacing the counterexample, never return Ok.
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("mk", num_scheme(0, 1));
        eval.register_operator_with_contract("sqrt", num_scheme(1, 1));
        eval.attach_refinement("sqrt : ( n: Num where n >= 0  --  r: Num )")
            .expect("sqrt refinement attaches");
        // `mk` is opaque ( -- Num ) with no fact bounding its output; feeding it to
        // `sqrt`'s demand `n >= 0` with no assume is the §10.7 Situation B hole.
        let src = "[ mk sqrt DROP ] :foo [ foo ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();

        let err = check_whole_program(&eval, crate::SmtLibSolver::new).expect_err(
            "a violated refinement demand (§10.7 Situation B) must fail closed, not return Ok",
        );

        match err {
            GateError::Tier1Violated(violations) => {
                assert_eq!(
                    violations.len(),
                    1,
                    "exactly one refinement obligation was left undischarged"
                );
                let ob = &violations[0];
                assert_eq!(ob.word, "sqrt", "the demand belongs to `sqrt`");
                assert!(
                    !ob.is_discharged(),
                    "the surfaced obligation is genuinely undischarged"
                );
                assert_eq!(
                    ob.verdict,
                    crate::Verdict::Sat,
                    "the VC `true => n >= 0` is refuted (Sat), not merely undecided"
                );
                let model = ob
                    .model
                    .as_ref()
                    .expect("a refuted, fully-decidable VC carries a counterexample model (§10.5)");
                assert!(
                    !model.is_empty(),
                    "the counterexample constrains the opaque input: {model}"
                );
            }
            other => panic!(
                "the gate must fail closed with Tier1Violated carrying the counterexample, got: {other}"
            ),
        }
    }

    // -----------------------------------------------------------------------
    // M10 end-to-end: higher-order subsumption reached THROUGH the whole-program
    // gate (§10.6 — the centerpiece, no longer test-only). These promote the
    // solver-level m10_* unit tests to whole-program acceptance: a refined
    // quotation crosses a higher-order boundary into a word whose signature
    // declares a refined-quotation parameter, and `check_whole_program` runs the
    // §10.6 subsumption check — accepting a stronger contract, failing closed on a
    // weaker or undecidable one.
    // -----------------------------------------------------------------------

    /// A Tier-0 scheme for a combinator-like operator that pops one element of
    /// **any** type (so a quotation value type-checks into the slot) and pushes a
    /// `Num`: ( 'S a -- 'S Num ). This is the shape of the higher-order `apply`
    /// the subsumption tests pass a refined quotation through.
    fn apply_scheme() -> Scheme {
        let s = sp();
        let input = StackTy::new(vec![Ty::var(1, s)], 0, s);
        let output = StackTy::new(vec![Ty::num(s)], 0, s);
        Scheme::new(vec![1], vec![0], WordTy::new(input, output))
    }

    /// Build a whole program in which `foo` passes a quotation `[ producer ]`
    /// through the higher-order `apply`, whose signature declares its parameter
    /// `q` as a refined quotation guaranteeing `r OP_EXPECTED k_expected`.
    /// `producer` is a refined definition guaranteeing `r OP_PROVIDED k_provided`.
    /// The §10.6 subsumption check runs at the `apply` boundary in `foo`.
    fn higher_order_program(
        apply_sig: &str,
        producer_sig: &str,
        producer_body: &str,
    ) -> Evaluator<Value> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("apply", apply_scheme());
        eval.attach_refinement(apply_sig)
            .expect("apply higher-order refinement attaches");
        eval.attach_refinement(producer_sig)
            .expect("producer refinement attaches");
        let src =
            format!("[ {producer_body} ] :producer [ [ producer ] apply DROP ] :foo [ foo ] :main");
        let tokens = parse_with_spans(&src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        eval
    }

    #[test]
    fn gate_accepts_a_stronger_quotation_guarantee_at_a_higher_order_boundary() {
        // §10.6 covariant guarantee, reached end-to-end: a quotation guaranteeing
        // `r > 5` passed where `r > 0` is expected. The subsumption VC
        // `r>5 ⟹ r>0` is valid (a STRONGER guarantee subsumes a weaker one), so
        // the gate returns Ok(clean) — the subsumption check ran through
        // `check_whole_program`, not a direct `check_subsumption` call.
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where r > 0 ) -- s: Num )",
            "producer : ( -- r: Num where r > 5 )",
            "6",
        );
        let ledger = check_whole_program(&eval, crate::SmtLibSolver::new).expect(
            "a stronger quotation guarantee subsumes the expected one: the gate returns Ok(clean)",
        );
        assert!(
            ledger.is_clean(),
            "covariant subsumption is preserved, so no violations: {:?}",
            ledger.violations()
        );
        assert!(
            ledger.violations().is_empty(),
            "a preserved higher-order contract leaves no undischarged obligation"
        );
    }

    #[test]
    fn gate_fails_closed_on_a_weaker_quotation_guarantee_at_a_higher_order_boundary() {
        // §10.6 covariant guarantee, reached end-to-end and FAILING: a quotation
        // guaranteeing only `r > 0` passed where `r > 5` is expected. The VC
        // `r>0 ⟹ r>5` is INVALID (a weaker guarantee cannot substitute), so the
        // gate must fail closed carrying the subsumption counterexample — never
        // return Ok.
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where r > 5 ) -- s: Num )",
            "producer : ( -- r: Num where r > 0 )",
            "1",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new).expect_err(
            "a weaker quotation guarantee violates §10.6 covariance: the gate must fail closed",
        );
        match err {
            GateError::Tier1Violated(violations) => {
                assert_eq!(
                    violations.len(),
                    1,
                    "exactly one subsumption direction is violated"
                );
                let ob = &violations[0];
                assert!(
                    ob.word.contains("subsumption"),
                    "the violation is a subsumption obligation, got `{}`",
                    ob.word
                );
                assert!(
                    ob.word.contains("guarantee"),
                    "the failing direction is the covariant guarantee, got `{}`",
                    ob.word
                );
                assert_eq!(
                    ob.verdict,
                    crate::Verdict::Sat,
                    "a present-but-weaker contract is refuted (Sat), not merely undecided"
                );
                let model = ob.model.as_ref().expect(
                    "a refuted, decidable subsumption VC carries a counterexample model (§10.6/§10.5)",
                );
                assert!(
                    !model.is_empty(),
                    "the counterexample witnesses the gap (a value > 0 but not > 5): {model}"
                );
            }
            other => panic!(
                "the gate must fail closed with Tier1Violated carrying the subsumption counterexample, got: {other}"
            ),
        }
    }

    #[test]
    fn gate_fails_closed_on_an_undecidable_subsumption_at_a_higher_order_boundary() {
        // §10.6 fail-closed (invariant 12), reached end-to-end: the expected
        // guarantee is `length r > 0` (an uninterpreted `length`), so the
        // covariant VC `r>0 ⟹ length r > 0` is UNKNOWN. An undecidable subsumption
        // VC must REJECT — never a silent pass — so the gate fails closed.
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where length r > 0 ) -- s: Num )",
            "producer : ( -- r: Num where r > 0 )",
            "6",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect_err("an Unknown subsumption VC must fail closed (§10.6), never return Ok");
        match err {
            GateError::Tier1Violated(violations) => {
                assert_eq!(violations.len(), 1, "one undecidable subsumption direction");
                let ob = &violations[0];
                assert!(
                    ob.word.contains("subsumption"),
                    "the violation is a subsumption obligation, got `{}`",
                    ob.word
                );
                assert_eq!(
                    ob.verdict,
                    crate::Verdict::Unknown,
                    "the VC is undecided (Unknown), which fails closed (§10.6)"
                );
                assert!(
                    !ob.is_discharged(),
                    "an Unknown is never accepted as discharged"
                );
                assert!(
                    ob.model.is_none(),
                    "an Unknown never carries a fabricated counterexample (§10.5)"
                );
            }
            other => panic!(
                "an Unknown subsumption VC must fail closed with Tier1Violated, got: {other}"
            ),
        }
    }

    #[test]
    fn gate_fails_closed_on_a_stronger_quotation_demand_at_a_higher_order_boundary() {
        // §10.6 contravariant demand (the FLIP), reached end-to-end and FAILING:
        // the quotation `producer` *demands* `n > 5`, but `apply` only promises to
        // call it where `n > 0` holds. The contravariant VC `expected_pre ⟹
        // provided_pre`, i.e. `n>0 ⟹ n>5`, is INVALID (a quotation demanding MORE
        // than the boundary supplies cannot substitute), so the gate fails closed.
        let eval = higher_order_program(
            "apply : ( q: ( n: Num where n > 0 -- r: Num ) -- s: Num )",
            "producer : ( n: Num where n > 5 -- r: Num )",
            "6",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new).expect_err(
            "a stronger quotation demand violates §10.6 contravariance: the gate must fail closed",
        );
        match err {
            GateError::Tier1Violated(violations) => {
                assert!(
                    violations.iter().any(|o| o.word.contains("subsumption")
                        && o.word.contains("demand")
                        && o.verdict == crate::Verdict::Sat),
                    "the contravariant demand direction is the refuted subsumption VC: {:?}",
                    violations.iter().map(|o| &o.word).collect::<Vec<_>>()
                );
            }
            other => panic!(
                "a stronger quotation demand must fail closed with Tier1Violated, got: {other}"
            ),
        }
    }

    #[test]
    fn gate_accepts_a_weaker_quotation_demand_at_a_higher_order_boundary() {
        // §10.6 contravariant demand, the accepting twin: `producer` demands only
        // `n > 0` where `apply` promises `n > 5`. The VC `n>5 ⟹ n>0` is valid (a
        // WEAKER provided demand subsumes a stronger expected one), so the gate
        // returns Ok(clean).
        let eval = higher_order_program(
            "apply : ( q: ( n: Num where n > 5 -- r: Num ) -- s: Num )",
            "producer : ( n: Num where n > 0 -- r: Num )",
            "6",
        );
        let ledger = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect("a weaker quotation demand subsumes: the gate returns Ok(clean)");
        assert!(
            ledger.is_clean(),
            "contravariant demand is preserved: {:?}",
            ledger.violations()
        );
    }

    #[test]
    fn gate_subsumption_is_reached_from_a_non_test_code_path() {
        // The acceptance bar: the subsumption check is invoked from a NON-TEST
        // code path reachable through `check_whole_program` (not a direct
        // `check_subsumption` call). We assert that by observing a subsumption
        // obligation surfacing through the gate's ledger — only `verify`'s
        // higher-order boundary handling produces one, and only when a refined
        // quotation crosses into a refined-quotation parameter.
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where r > 5 ) -- s: Num )",
            "producer : ( -- r: Num where r > 0 )",
            "1",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect_err("the weaker contract fails closed");
        let GateError::Tier1Violated(violations) = err else {
            panic!("expected a Tier1Violated subsumption failure");
        };
        assert!(
            violations.iter().any(|o| o.word.contains("subsumption")),
            "a subsumption obligation reached the ledger through the gate (not a direct call)"
        );
    }

    /// Build a higher-order program whose `producer` carries **no** refinement
    /// signature — an **unrefined** quotation (§10.7 absent-payload). It is passed
    /// through `apply`, whose signature declares its parameter `q` as a *refined*
    /// quotation. The §10.6 subsumption boundary then sees a `where true` provided
    /// contract meeting a required guarantee: the gradual-interop "carries no
    /// contract" case (M11), as opposed to [`higher_order_program`] which always
    /// attaches a producer contract.
    fn unrefined_producer_program(apply_sig: &str, producer_body: &str) -> Evaluator<Value> {
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("apply", apply_scheme());
        eval.attach_refinement(apply_sig)
            .expect("apply higher-order refinement attaches");
        // NOTE: deliberately NO `attach_refinement` for `producer` — it stays
        // unrefined so its relayed contract is `where true` (absent payload).
        let src =
            format!("[ {producer_body} ] :producer [ [ producer ] apply DROP ] :foo [ foo ] :main");
        let tokens = parse_with_spans(&src).unwrap();
        eval.load_with_spans(&tokens).unwrap();
        eval
    }

    #[test]
    fn gate_surfaces_carries_no_contract_for_an_unrefined_quotation_meeting_a_guarantee() {
        // §10.7 / M11 / invariant 12, reached end-to-end: an UNREFINED quotation
        // (`producer` has no contract) passed where a guarantee `r > 0` is
        // required. The covariant guarantee VC `true ⟹ r>0` fails, but the honest,
        // actionable diagnosis is the ABSENT contract — not a bare SMT witness for
        // some incidental value. The gate must fail closed AND render the targeted
        // SUBSUMPTION_NO_CONTRACT_MSG, never a bare counterexample.
        let eval =
            unrefined_producer_program("apply : ( q: ( -- r: Num where r > 0 ) -- s: Num )", "6");
        let err = check_whole_program(&eval, crate::SmtLibSolver::new).expect_err(
            "an unrefined quotation meeting a required guarantee fails closed (§10.7/M11)",
        );
        let GateError::Tier1Violated(ref violations) = err else {
            panic!("expected a Tier1Violated subsumption failure, got: {err}");
        };
        assert_eq!(
            violations.len(),
            1,
            "one clear cause per boundary: the carries-no-contract guarantee direction"
        );
        let ob = &violations[0];
        assert!(
            ob.word.contains("subsumption") && ob.word.contains("guarantee"),
            "the absent-contract failure is the covariant guarantee direction, got `{}`",
            ob.word
        );
        assert_eq!(
            ob.message.as_deref(),
            Some(crate::SUBSUMPTION_NO_CONTRACT_MSG),
            "the obligation carries the targeted carries-no-contract message"
        );
        assert!(
            ob.model.is_none(),
            "the absent-contract case surfaces no incidental SMT witness"
        );
        // The rendered gate error must say "carries no contract" and NOT show a
        // bare counterexample — the diagnostic must be actionable (§7).
        let rendered = err.to_string();
        assert!(
            rendered.contains(crate::SUBSUMPTION_NO_CONTRACT_MSG),
            "the gate error must contain the carries-no-contract message, got:\n{rendered}"
        );
        assert!(
            rendered.contains("carries no contract"),
            "the gate error text names the missing contract, got:\n{rendered}"
        );
        assert!(
            !rendered.contains("counterexample"),
            "the carries-no-contract diagnostic must NOT degrade to a bare counterexample, got:\n{rendered}"
        );
    }

    #[test]
    fn gate_keeps_the_counterexample_for_a_present_but_weaker_guarantee_no_carries_no_contract() {
        // The present-vs-absent distinction, end-to-end: a PRESENT-but-weaker
        // guarantee (`r > 0` where `r > 5` is required) is a genuine M10 violation
        // — the contract exists and is simply too weak — so the gate keeps the SMT
        // counterexample and must NOT emit the carries-no-contract message
        // (which is reserved for the absent-contract case).
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where r > 5 ) -- s: Num )",
            "producer : ( -- r: Num where r > 0 )",
            "1",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect_err("a present-but-weaker guarantee fails closed with a counterexample");
        let GateError::Tier1Violated(ref violations) = err else {
            panic!("expected a Tier1Violated subsumption failure, got: {err}");
        };
        assert_eq!(violations.len(), 1, "one violated guarantee direction");
        let ob = &violations[0];
        assert_eq!(
            ob.verdict,
            crate::Verdict::Sat,
            "a present-but-weaker contract is refuted (Sat), keeping its counterexample"
        );
        assert!(
            ob.message.is_none(),
            "a present contract is NOT the carries-no-contract case — no targeted message"
        );
        assert!(
            ob.model.is_some(),
            "the present-but-weaker violation keeps its counterexample model (§10.5)"
        );
        let rendered = err.to_string();
        assert!(
            !rendered.contains("carries no contract"),
            "a present-but-weaker guarantee must NOT claim the quotation carries no contract, got:\n{rendered}"
        );
        assert!(
            rendered.contains("counterexample"),
            "the present-but-weaker violation surfaces its counterexample, got:\n{rendered}"
        );
    }

    #[test]
    fn gate_surfaces_fail_closed_message_for_an_undecidable_subsumption() {
        // §10.6 fail-closed (invariant 12), end-to-end with diagnostic fidelity:
        // the expected guarantee `length r > 0` (uninterpreted `length`) makes the
        // covariant VC `r>0 ⟹ length r > 0` UNKNOWN. The boundary fails closed AND
        // the gate must render the targeted SUBSUMPTION_FAIL_CLOSED_MSG, not just a
        // bare verdict.
        let eval = higher_order_program(
            "apply : ( q: ( -- r: Num where length r > 0 ) -- s: Num )",
            "producer : ( -- r: Num where r > 0 )",
            "6",
        );
        let err = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect_err("an Unknown subsumption VC must fail closed (§10.6)");
        let GateError::Tier1Violated(ref violations) = err else {
            panic!("expected a Tier1Violated subsumption failure, got: {err}");
        };
        assert_eq!(violations.len(), 1, "one undecidable subsumption direction");
        let ob = &violations[0];
        assert_eq!(
            ob.verdict,
            crate::Verdict::Unknown,
            "the VC is undecided (Unknown), which fails closed (§10.6)"
        );
        assert_eq!(
            ob.message.as_deref(),
            Some(crate::SUBSUMPTION_FAIL_CLOSED_MSG),
            "the obligation carries the targeted fail-closed message"
        );
        assert!(
            ob.model.is_none(),
            "an Unknown never carries a fabricated counterexample (§10.5)"
        );
        let rendered = err.to_string();
        assert!(
            rendered.contains(crate::SUBSUMPTION_FAIL_CLOSED_MSG),
            "the gate error must contain the fail-closed message, got:\n{rendered}"
        );
        assert!(
            !rendered.contains("counterexample"),
            "the fail-closed diagnostic carries no counterexample, got:\n{rendered}"
        );
    }

    #[test]
    fn gate_returns_ok_when_the_demand_is_satisfied() {
        // The positive twin of the Situation B test: the SAME refined `sqrt` demand
        // `n >= 0`, but fed a value the solver can prove satisfies it (a constant
        // `4`), with no assume. The VC `true => 4 >= 0` is `Unsat` (discharged), so
        // the obligation stream is clean and the gate returns Ok(clean ledger).
        let mut eval: Evaluator<Value> = Evaluator::new();
        eval.register_operator_with_contract("sqrt", num_scheme(1, 1));
        eval.attach_refinement("sqrt : ( n: Num where n >= 0  --  r: Num )")
            .expect("sqrt refinement attaches");
        let src = "[ 4 sqrt DROP ] :foo [ foo ] :main";
        let tokens = parse_with_spans(src).unwrap();
        eval.load_with_spans(&tokens).unwrap();

        let ledger = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect("a satisfied refinement demand discharges: the gate returns Ok(clean)");
        assert!(
            ledger.is_clean(),
            "no violations or rejections: {:?} / {:?}",
            ledger.violations(),
            ledger.rejections()
        );
        assert!(
            ledger.violations().is_empty(),
            "the discharged demand leaves no violated obligation"
        );
    }

    #[test]
    fn gate_does_not_run_tier1_when_tier0_fails() {
        // Invariant 19: Tier 1 is parasitic on Tier 0 having balanced every
        // arity, so the gate must NOT reach the shadow stack / solver when Tier 0
        // rejects. A top-level `DROP` underflows the empty stack (§12 M2).
        let mut eval: Evaluator<Value> = Evaluator::new();
        let tokens = parse_with_spans("[ DROP ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();

        // A solver factory that PANICS if the gate ever builds a solver — i.e. if
        // it ever reached the Tier-1 half. Tier-0 rejection must short-circuit
        // before this fires.
        let solver_built = std::cell::Cell::new(0usize);
        let mk_solver = || {
            solver_built.set(solver_built.get() + 1);
            crate::SmtLibSolver::new()
        };

        let err = check_whole_program(&eval, mk_solver)
            .expect_err("a Tier-0 underflow must reject at the gate");
        assert!(
            matches!(err, GateError::Tier0(_)),
            "the gate must return the Tier-0 error, got: {err}"
        );
        assert_eq!(
            solver_built.get(),
            0,
            "Tier 1 must NOT run when Tier 0 fails — no solver may be built"
        );
    }

    #[test]
    fn gate_ledger_equals_the_separate_check_then_check_program_sequence() {
        // The gate is exactly the composition the M14 example wired by hand:
        // `check` then `check_program`. Driving them separately must yield the
        // same ledger the one-call gate returns (same accepted entries / modulo
        // status).
        let eval = assume_bearing_program();

        // --- the separate two-call sequence (the pre-gate pattern) ---
        check(&eval).expect("Tier 0 passes");
        let mut names: Vec<&str> = eval.definition_names().collect();
        names.sort_unstable();
        let defs: Vec<crate::Definition> = names
            .iter()
            .map(|&name| crate::Definition {
                name: name.to_string(),
                body: eval.definition_body(name).unwrap().to_vec(),
                sig: eval.refinement(name).cloned(),
            })
            .collect();
        let lookup = |w: &str| eval.refinement(w).cloned();
        let separate =
            crate::check_program(&defs, &lookup, crate::SmtLibSolver::new).expect("Tier 1 passes");

        // --- the one-call combined gate ---
        let combined = check_whole_program(&eval, crate::SmtLibSolver::new)
            .expect("the gate composes the same two halves");

        // Equivalent ledgers: same accepted entries, same per-word modulo status.
        assert_eq!(separate.grep_assume(), combined.grep_assume());
        assert_eq!(
            separate.assumptions().len(),
            combined.assumptions().len(),
            "same number of accepted assumptions"
        );
        for name in &names {
            assert_eq!(
                separate.status(name).to_string(),
                combined.status(name).to_string(),
                "modulo status of `{name}` must match between the separate sequence and the gate"
            );
        }
        assert_eq!(separate.is_clean(), combined.is_clean());
    }
}
