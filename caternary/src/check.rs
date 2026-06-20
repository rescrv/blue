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
                let body_arrow = result?;
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

/// The neutral span carried by synthesized annotation/probe nodes that have no
/// single source byte of their own; real diagnostics re-anchor at the offending
/// call site, exactly as the language-core schemes do.
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
        // `[ [ 1 true + ] call ]`: inside the inner quotation `+` demands two
        // `Num`s but `true` produced a `Bool`. The contradiction is born inside
        // the inner frame; both frames enclose it, so localize-inward must anchor
        // at the INNER one (the user's real mistake), not the outer consequence.
        let err = infer_snippet("[ [ 1 true + ] call ]").unwrap_err();
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
        // breadcrumb — a backtrace, not one opaque site. `[ [ 1 true + ] call ]`
        // is two frames deep, so the diagnostic must list BOTH "in the quotation
        // at byte N" lines, outermost first.
        let err = infer_snippet("[ [ 1 true + ] call ]").unwrap_err();
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
}
