//! Tier-0 type-system substrate for Caternary (the §3 representation).
//!
//! This module is the foundation the whole-program type checker rests on. It
//! implements the *type representation* from `docs/typing.md` §3 — `Ty`,
//! `StackTy`, `WordTy`, `Scheme` — together with the two representation
//! invariants that must be present "from the first inference commit": origin
//! spans on every node, and the durable typing-frame stack. It also provides the
//! flat substitution arena + trailed undo log mandated by ratified decision (b).
//!
//! It is deliberately *Tier-0 only*. None of the Tier 1 refinement machinery
//! (predicate payloads, shadow stack, VC generation, Z3) lives here. The
//! substitution trail in this module is the Tier-0 inference backtracking trail
//! and is **not** the Tier 1 logical-scope mechanism (§3 invariant 5 / 18).
//!
//! # Runtime facts recorded by the substrate
//!
//! `docs/typing.md` uses the same **UPPER_SNAKE_CASE** spelling the runtime
//! registers for builtins, so there is no spec-name/runtime-name compatibility
//! table in this layer. The remaining Tier-0 reconciliation facts are:
//!
//! * **Scalar operators are registered builtins, not core schemes.** Arithmetic,
//!   comparison, boolean, and bitwise words such as `+`, `>=`, `&&`, and `|`
//!   enter the type environment through
//!   [`crate::register_scalar_builtins`] /
//!   [`crate::Evaluator::register_operator_with_contract`], never from
//!   [`core_scheme`]. The numeric base type is spelled [`NUM`].
//! * **There is no numeric token.** The parser emits only `Token::Word` and
//!   `Token::Bracket` (see `parser.rs`); there is no `Token::Num`. The Tier-0
//!   decision, recorded as [`is_numeric_literal`], is that a numeric literal is a
//!   `Token::Word` whose text parses as a Rust `f64` (which subsumes integers).
//!   Such a word has Tier-0 type `( 'a -- 'a Num )` per §5.

use crate::Span;

/// A Tier-0 type variable. Identifies a slot in the substitution arena.
pub type TyVar = u32;

/// A Tier-0 row variable. Identifies a stack-tail slot in the substitution arena.
pub type RowVar = u32;

/// The name of the single numeric base type (§1 ratified decision (a): one
/// numeric type `Num`, no Int/Float split, no overloading).
pub const NUM: &str = "Num";

/// The name of the boolean base type.
pub const BOOL: &str = "Bool";

/// The distinguished whole-program entry point: the definition whose effect must
/// close against the empty stack (§12 / architecture section). The runtime has
/// no `main` convention of its own; this constant *is* that reconciliation —
/// the type checker treats the definition named `main` as the program, and every
/// other `[ body ] :name` as a library definition in the flat global namespace.
pub const MAIN: &str = "main";

/// The Tier-0 decision for what counts as a numeric literal at the `Token`
/// level. The parser has no numeric token; a numeric literal is a word whose
/// text parses as an `f64` (integers included). Such a word has Tier-0 type
/// `( 'a -- 'a Num )` (§5).
pub fn is_numeric_literal(word: &str) -> bool {
    !word.is_empty() && word.parse::<f64>().is_ok()
}

/// The Tier-0 decision for what counts as a boolean literal.
///
/// `docs/typing.md` §5 spells out the numeric-literal rule but the `IF`
/// combinator's §2 scheme demands a `Bool` on top, and the M2 acceptance snippet
/// `true [ 1 ] [ 2 ] IF` (§12) feeds a `true`. The runtime already treats the
/// bare words `true`/`false` as boolean values (see `combinators.rs`); this is
/// the typing counterpart, recorded here so the reconciliation cannot drift: a
/// boolean literal is the word `true` or `false`, with Tier-0 type
/// `( 'a -- 'a Bool )`.
pub fn is_bool_literal(word: &str) -> bool {
    word == "true" || word == "false"
}

/// The synthetic span carried by a language-core scheme's nodes. Core schemes
/// (`DUP`/`DROP`/`SWAP`/`OVER`/`CALL`/`IF` and the other fixed builtins here)
/// have no *source* location — they
/// are authored contracts, not parsed text — so their nodes are born at this
/// neutral span and [`respan_word`] re-anchors them at the call-site token
/// whenever a use is instantiated, so diagnostics point at the user's word
/// rather than byte 0.
const CORE_SPAN: Span = Span { start: 0, end: 0 };

/// The Tier-0 scheme for a **language-core primitive**, keyed by its *runtime*
/// (uppercase) name (§2, §8). These are the irreducible words the language core
/// registers — they have no Caternary body — authored once here, never
/// user-registrable (the capability wall is the embedding API; §13 invariant 16).
///
/// This is the language-core counterpart to the embedder's
/// [`crate::Evaluator::register_operator_with_contract`]: both contribute schemes
/// to inference (§5 `lookup`), but core schemes are baked in rather than attested
/// at embed time. The schemes include the §2 primitives and the fixed-arity
/// stack/combinator words that the runtime implements directly:
///
/// ```text
/// DUP    : ( 'S a        -- 'S a a )
/// DROP   : ( 'S a        -- 'S )
/// SWAP   : ( 'S a b      -- 'S b a )
/// OVER   : ( 'S a b      -- 'S a b a )
/// ROT    : ( 'S a b c    -- 'S b c a )
/// -ROT   : ( 'S a b c    -- 'S c a b )
/// NIP    : ( 'S a b      -- 'S b )
/// TUCK   : ( 'S a b      -- 'S b a b )
/// 2DUP   : ( 'S a b      -- 'S a b a b )
/// 2DROP  : ( 'S a b      -- 'S )
/// 2SWAP  : ( 'S a b c d  -- 'S c d a b )
/// 2OVER  : ( 'S a b c d  -- 'S a b c d a b )
/// 2ROT   : ( 'S a b c d e f -- 'S c d e f a b )
/// CALL   : ( 'S ('S -- 'T) -- 'T )
/// IF     : ( 'S Bool ('S -- 'T) ('S -- 'T) -- 'T )
/// CURRY  : ( 'R a ('S a -- 'T)             -- 'R ('S -- 'T) )
/// WHEN   : ( 'S Bool ('S -- 'S)            -- 'S )
/// UNLESS : ( 'S Bool ('S -- 'S)            -- 'S )
/// MAP    : ( 'S (List a) ( 'r a -- 'r b )    -- 'S (List b) )
/// FILTER : ( 'S (List a) ( 'r a -- 'r Bool ) -- 'S (List a) )
/// FOLD   : ( 'S (List a) b ( 'r b a -- 'r b ) -- 'S b )
/// EACH   : ( 'S (List a) ( 'r a -- 'r )    -- 'S )
/// ```
///
/// `DIP` is the M4 §8 **relay** combinator: its scheme alone states the only
/// fact `DIP` contributes — the set-aside value `a` returns unchanged while the
/// quotation runs on the rest. The quotation's **declared** arrow `( 'S -- 'T )`
/// is relayed verbatim; the body is never expanded into the caller's contract
/// (§13 invariant 8).
///
/// `MAP` / `FILTER` / `FOLD` / `EACH` are the **higher-order sequence relays**:
/// their quotation argument's two arrow ends share a row (`'r`), forcing an
/// element-to-element shape, and the element type relays through the `List`
/// constructor. `MAP` turns `List a` into `List b` via the element transform
/// `a -> b`; `FILTER` keeps `List a` via the predicate `a -> Bool` (no
/// post-filter length claim); `FOLD` threads an accumulator `b` through a step
/// `( 'r b a -- 'r b )` and returns the final accumulator; `EACH` runs a
/// stack-neutral consumer `( 'r a -- 'r )` per element for effect. Like `DIP`,
/// only the declared quotation arrow is relayed, never expanded (§13 inv 8).
///
/// `CURRY` is a **quotation-builder**: it bakes a value into a quotation's input,
/// producing a quotation whose arrow drops that value (`a ( 'S a -- 'T ) -- ( 'S
/// -- 'T )`). `WHEN` / `UNLESS` are the **one-armed conditionals**: the absent
/// branch is the identity, so the single quotation must be stack-shape-preserving
/// (`'S -- 'S`) for both control paths to agree.
pub fn core_scheme(runtime_name: &str) -> Option<Scheme> {
    let s = CORE_SPAN;
    let v = |idx| Ty::var(idx, s);
    let stack = |row, elems| StackTy::new(elems, row, s);
    let empty = |row| StackTy::empty(row, s);
    let quote = |input, output| Ty::quote(WordTy::new(input, output), s);
    let list = |elem| Ty::app("List", vec![elem], s);
    // The quotation argument's arrow ( 'S -- 'T ): rowvar 0 = 'S, rowvar 1 = 'T.
    let arrow_st = || WordTy::new(empty(0), empty(1));
    let scheme = match runtime_name {
        "DUP" => Scheme::new(
            vec![0],
            vec![0],
            WordTy::new(stack(0, vec![v(0)]), stack(0, vec![v(0), v(0)])),
        ),
        "DROP" => Scheme::new(
            vec![0],
            vec![0],
            WordTy::new(stack(0, vec![v(0)]), empty(0)),
        ),
        "SWAP" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(stack(0, vec![v(0), v(1)]), stack(0, vec![v(1), v(0)])),
        ),
        "OVER" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(stack(0, vec![v(0), v(1)]), stack(0, vec![v(0), v(1), v(0)])),
        ),
        "ROT" => Scheme::new(
            vec![0, 1, 2],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2)]),
                stack(0, vec![v(1), v(2), v(0)]),
            ),
        ),
        "-ROT" => Scheme::new(
            vec![0, 1, 2],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2)]),
                stack(0, vec![v(2), v(0), v(1)]),
            ),
        ),
        "NIP" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(stack(0, vec![v(0), v(1)]), stack(0, vec![v(1)])),
        ),
        "TUCK" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(stack(0, vec![v(0), v(1)]), stack(0, vec![v(1), v(0), v(1)])),
        ),
        "2DUP" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1)]),
                stack(0, vec![v(0), v(1), v(0), v(1)]),
            ),
        ),
        "2DROP" => Scheme::new(
            vec![0, 1],
            vec![0],
            WordTy::new(stack(0, vec![v(0), v(1)]), empty(0)),
        ),
        "2SWAP" => Scheme::new(
            vec![0, 1, 2, 3],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2), v(3)]),
                stack(0, vec![v(2), v(3), v(0), v(1)]),
            ),
        ),
        "2OVER" => Scheme::new(
            vec![0, 1, 2, 3],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2), v(3)]),
                stack(0, vec![v(0), v(1), v(2), v(3), v(0), v(1)]),
            ),
        ),
        "2ROT" => Scheme::new(
            vec![0, 1, 2, 3, 4, 5],
            vec![0],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2), v(3), v(4), v(5)]),
                stack(0, vec![v(2), v(3), v(4), v(5), v(0), v(1)]),
            ),
        ),
        "CALL" => Scheme::new(
            vec![],
            vec![0, 1],
            WordTy::new(stack(0, vec![Ty::quote(arrow_st(), s)]), empty(1)),
        ),
        // DIP : ( 'S a ( 'S -- 'T ) -- 'T a ) (§2, §8 relay). Input has the
        // set-aside value `a` (tyvar 0) below the quotation ( 'S -- 'T ) on top
        // (rowvar 0 = 'S, rowvar 1 = 'T). The quotation runs on 'S and yields
        // 'T; `a` is carried through unchanged onto the result 'T a. Only the
        // declared quotation arrow is relayed — no body expansion (§13 inv 8).
        "DIP" => Scheme::new(
            vec![0],
            vec![0, 1],
            WordTy::new(
                stack(0, vec![v(0), Ty::quote(arrow_st(), s)]),
                stack(1, vec![v(0)]),
            ),
        ),
        "2DIP" => Scheme::new(
            vec![0, 1],
            vec![0, 1],
            WordTy::new(
                stack(0, vec![v(0), v(1), Ty::quote(arrow_st(), s)]),
                stack(1, vec![v(0), v(1)]),
            ),
        ),
        "3DIP" => Scheme::new(
            vec![0, 1, 2],
            vec![0, 1],
            WordTy::new(
                stack(0, vec![v(0), v(1), v(2), Ty::quote(arrow_st(), s)]),
                stack(1, vec![v(0), v(1), v(2)]),
            ),
        ),
        "IF" => Scheme::new(
            vec![],
            vec![0, 1],
            WordTy::new(
                stack(
                    0,
                    vec![
                        Ty::bool(s),
                        Ty::quote(arrow_st(), s),
                        Ty::quote(arrow_st(), s),
                    ],
                ),
                empty(1),
            ),
        ),
        "KEEP" => {
            let q = quote(stack(0, vec![v(0)]), empty(1));
            Scheme::new(
                vec![0],
                vec![0, 1],
                WordTy::new(stack(0, vec![v(0), q]), stack(1, vec![v(0)])),
            )
        }
        "2KEEP" => {
            let q = quote(stack(0, vec![v(0), v(1)]), empty(1));
            Scheme::new(
                vec![0, 1],
                vec![0, 1],
                WordTy::new(stack(0, vec![v(0), v(1), q]), stack(1, vec![v(0), v(1)])),
            )
        }
        "3KEEP" => {
            let q = quote(stack(0, vec![v(0), v(1), v(2)]), empty(1));
            Scheme::new(
                vec![0, 1, 2],
                vec![0, 1],
                WordTy::new(
                    stack(0, vec![v(0), v(1), v(2), q]),
                    stack(1, vec![v(0), v(1), v(2)]),
                ),
            )
        }
        "BI" => {
            let p = quote(stack(0, vec![v(0)]), empty(1));
            let q = quote(stack(1, vec![v(0)]), empty(2));
            Scheme::new(
                vec![0],
                vec![0, 1, 2],
                WordTy::new(stack(0, vec![v(0), p, q]), empty(2)),
            )
        }
        "BI*" => {
            let p = quote(stack(0, vec![v(0)]), empty(1));
            let q = quote(stack(1, vec![v(1)]), empty(2));
            Scheme::new(
                vec![0, 1],
                vec![0, 1, 2],
                WordTy::new(stack(0, vec![v(0), v(1), p, q]), empty(2)),
            )
        }
        "BI@" => {
            let q = quote(stack(1, vec![v(0)]), stack(1, vec![v(1)]));
            Scheme::new(
                vec![0, 1],
                vec![0, 1],
                WordTy::new(stack(0, vec![v(0), v(0), q]), stack(0, vec![v(1), v(1)])),
            )
        }
        "TRI" => {
            let p = quote(stack(0, vec![v(0)]), empty(1));
            let q = quote(stack(1, vec![v(0)]), empty(2));
            let r = quote(stack(2, vec![v(0)]), empty(3));
            Scheme::new(
                vec![0],
                vec![0, 1, 2, 3],
                WordTy::new(stack(0, vec![v(0), p, q, r]), empty(3)),
            )
        }
        "TRI*" => {
            let p = quote(stack(0, vec![v(0)]), empty(1));
            let q = quote(stack(1, vec![v(1)]), empty(2));
            let r = quote(stack(2, vec![v(2)]), empty(3));
            Scheme::new(
                vec![0, 1, 2],
                vec![0, 1, 2, 3],
                WordTy::new(stack(0, vec![v(0), v(1), v(2), p, q, r]), empty(3)),
            )
        }
        "TRI@" => {
            let q = quote(stack(1, vec![v(0)]), stack(1, vec![v(1)]));
            Scheme::new(
                vec![0, 1],
                vec![0, 1],
                WordTy::new(
                    stack(0, vec![v(0), v(0), v(0), q]),
                    stack(0, vec![v(1), v(1), v(1)]),
                ),
            )
        }
        "COMPOSE" => {
            let p = quote(empty(1), empty(2));
            let q = quote(empty(2), empty(3));
            let composed = quote(empty(1), empty(3));
            Scheme::new(
                vec![],
                vec![0, 1, 2, 3],
                WordTy::new(stack(0, vec![p, q]), stack(0, vec![composed])),
            )
        }
        // CURRY : ( 'R a ( 'S a -- 'T ) -- 'R ( 'S -- 'T ) ). Bake the value `a`
        // (tyvar 0) into the quotation's input: the input quotation expects `a`
        // on top of 'S, the result quotation supplies it itself, so its arrow
        // drops the `a` ( 'S -- 'T ). rowvar 0='R (the outer base), 1='S, 2='T.
        "CURRY" => Scheme::new(
            vec![0],
            vec![0, 1, 2],
            WordTy::new(
                stack(
                    0,
                    vec![v(0), quote(stack(1, vec![v(0)]), empty(2))],
                ),
                stack(0, vec![quote(empty(1), empty(2))]),
            ),
        ),
        // WHEN : ( 'S Bool ( 'S -- 'S ) -- 'S ). The absent (false) branch is the
        // identity, so for both control paths to agree the quotation must be
        // stack-shape-preserving: both ends share row 0 ('S -- 'S). Unlike IF,
        // there is no 'T — the result is always 'S.
        "WHEN" => Scheme::new(
            vec![],
            vec![0],
            WordTy::new(
                stack(0, vec![Ty::bool(s), quote(empty(0), empty(0))]),
                empty(0),
            ),
        ),
        // UNLESS : ( 'S Bool ( 'S -- 'S ) -- 'S ). As WHEN, but the quotation runs
        // on the false branch; the typing constraint (stack-shape-preserving
        // quotation, 'S -- 'S) is identical.
        "UNLESS" => Scheme::new(
            vec![],
            vec![0],
            WordTy::new(
                stack(0, vec![Ty::bool(s), quote(empty(0), empty(0))]),
                empty(0),
            ),
        ),
        // MAP : ( 'S (List a) ( 'r a -- 'r b ) -- 'S (List b) ) (§8 relay,
        // higher-order). The quotation argument is the **element transform**: it
        // pops one `a` and pushes one `b`, leaving the rest of the stack ('r)
        // untouched — so both ends of its arrow share row 1 ('r), forcing an
        // element-to-element shape. The element type relays through the lists:
        // the input list's element (tyvar 0 = a) is the quotation's input, the
        // quotation's output (tyvar 1 = b) is the output list's element. Only the
        // declared quotation arrow is relayed — no body expansion (§13 inv 8).
        "MAP" => Scheme::new(
            vec![0, 1],
            vec![0, 1],
            WordTy::new(
                stack(
                    0,
                    vec![
                        list(v(0)),
                        quote(stack(1, vec![v(0)]), stack(1, vec![v(1)])),
                    ],
                ),
                stack(0, vec![list(v(1))]),
            ),
        ),
        // FILTER : ( 'S (List a) ( 'r a -- 'r Bool ) -- 'S (List a) ) (§8 relay,
        // higher-order). The quotation is the **predicate**: it pops one `a` and
        // pushes a Bool, leaving the rest of the stack ('r) untouched — both ends
        // of its arrow share row 1 ('r). The element type (tyvar 0 = a) is
        // unchanged across the call: the result is `List a`, same element as the
        // input — no length claim is made. Only the declared quotation arrow is
        // relayed — no body expansion (§13 inv 8).
        "FILTER" => Scheme::new(
            vec![0],
            vec![0, 1],
            WordTy::new(
                stack(
                    0,
                    vec![
                        list(v(0)),
                        quote(stack(1, vec![v(0)]), stack(1, vec![Ty::bool(s)])),
                    ],
                ),
                stack(0, vec![list(v(0))]),
            ),
        ),
        // FOLD : ( 'S (List a) b ( 'r b a -- 'r b ) -- 'S b ). Thread an
        // accumulator `b` (tyvar 1) through a step quotation that consumes the
        // accumulator and one element `a` (tyvar 0) and leaves the new
        // accumulator, sharing row 'r (rowvar 1) across both ends. The result is
        // the final accumulator `b`. Relay only — the step arrow is never
        // expanded (§13 inv 8).
        "FOLD" => Scheme::new(
            vec![0, 1],
            vec![0, 1],
            WordTy::new(
                stack(
                    0,
                    vec![
                        list(v(0)),
                        v(1),
                        quote(stack(1, vec![v(1), v(0)]), stack(1, vec![v(1)])),
                    ],
                ),
                stack(0, vec![v(1)]),
            ),
        ),
        // EACH : ( 'S (List a) ( 'r a -- 'r ) -- 'S ). Run a stack-neutral
        // consumer (pops one element `a` (tyvar 0), leaves the rest of the stack
        // 'r untouched — both ends share row 1) once per element, for effect. The
        // list is consumed and the base stack 'S returns unchanged. Relay only.
        "EACH" => Scheme::new(
            vec![0],
            vec![0, 1],
            WordTy::new(
                stack(
                    0,
                    vec![
                        list(v(0)),
                        quote(stack(1, vec![v(0)]), empty(1)),
                    ],
                ),
                empty(0),
            ),
        ),
        _ => return None,
    };
    Some(scheme)
}

/// Re-anchor every origin span inside a [`WordTy`] to `span`. Used after
/// instantiating a registered or language-core scheme so the freshly minted
/// nodes report the **call-site** word's location in a diagnostic instead of the
/// scheme's authoring span (§7: every conflicting type carries a real birth
/// span; a core scheme's `CORE_SPAN` would otherwise surface as byte 0).
pub fn respan_word(word: &WordTy, span: Span) -> WordTy {
    WordTy::new(
        respan_stack(&word.input, span),
        respan_stack(&word.output, span),
    )
}

fn respan_stack(stack: &StackTy, span: Span) -> StackTy {
    StackTy {
        elems: stack.elems.iter().map(|e| respan_ty(e, span)).collect(),
        row: stack.row,
        span,
    }
}

fn respan_ty(ty: &Ty, span: Span) -> Ty {
    let kind = match &ty.kind {
        TyKind::Var(v) => TyKind::Var(*v),
        TyKind::Con(n) => TyKind::Con(n.clone()),
        TyKind::App(n, args) => {
            TyKind::App(n.clone(), args.iter().map(|a| respan_ty(a, span)).collect())
        }
        TyKind::Quote(w) => TyKind::Quote(Box::new(respan_word(w, span))),
    };
    Ty { kind, span }
}

/// The shape of a Tier-0 element type (§3). Variant set is exactly the spec's:
/// `Var`/`Con`/`App`/`Quote`. The origin span lives on the enclosing [`Ty`].
#[derive(Clone, Debug, PartialEq)]
pub enum TyKind {
    /// A type variable.
    Var(TyVar),
    /// A nullary base type, e.g. `Num`, `Bool`.
    Con(String),
    /// A parameterized type, e.g. `List a`. Added to the representation now;
    /// the unifier grows support for it when first needed.
    App(String, Vec<Ty>),
    /// A quotation value, carrying an arrow. Tier-0 stores SHAPE ONLY; any Tier 1
    /// refinement payload is added later and is never read by Tier-0 (§3).
    Quote(Box<WordTy>),
}

/// A Tier-0 element type with its origin span (§3 invariant 2: every `Ty` carries
/// the source span where it was *born*, so the checker can report a provenance
/// pair instead of a bare failure site). The span is propagated through
/// unification; it is **not** an M5 deliverable, it is a representation invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct Ty {
    /// The shape of this type.
    pub kind: TyKind,
    /// The source span where this type was born.
    pub span: Span,
}

impl Ty {
    /// A type variable born at `span`.
    pub fn var(v: TyVar, span: Span) -> Self {
        Ty {
            kind: TyKind::Var(v),
            span,
        }
    }

    /// A base type born at `span`.
    pub fn con(name: impl Into<String>, span: Span) -> Self {
        Ty {
            kind: TyKind::Con(name.into()),
            span,
        }
    }

    /// The `Num` base type born at `span`.
    pub fn num(span: Span) -> Self {
        Ty::con(NUM, span)
    }

    /// The `Bool` base type born at `span`.
    pub fn bool(span: Span) -> Self {
        Ty::con(BOOL, span)
    }

    /// A parameterized type `name args…` born at `span`, e.g. `List a`.
    pub fn app(name: impl Into<String>, args: Vec<Ty>, span: Span) -> Self {
        Ty {
            kind: TyKind::App(name.into(), args),
            span,
        }
    }

    /// A quotation value carrying `arrow`, born at `span`.
    pub fn quote(arrow: WordTy, span: Span) -> Self {
        Ty {
            kind: TyKind::Quote(Box::new(arrow)),
            span,
        }
    }
}

/// A Tier-0 stack type: an ordered list of element types with the **top of stack
/// LAST**, terminated by a [`RowVar`] standing for the untouched tail (§2, §3).
/// Carries an origin span (§3 invariant 2).
#[derive(Clone, Debug, PartialEq)]
pub struct StackTy {
    /// Element types, top of stack last.
    pub elems: Vec<Ty>,
    /// The row variable standing for "whatever was underneath, untouched".
    pub row: RowVar,
    /// The source span where this stack type was born.
    pub span: Span,
}

impl StackTy {
    /// A stack type with no observed elements and tail `row`, born at `span`.
    pub fn empty(row: RowVar, span: Span) -> Self {
        StackTy {
            elems: Vec::new(),
            row,
            span,
        }
    }

    /// A stack type with the given elements (top last) and tail `row`.
    pub fn new(elems: Vec<Ty>, row: RowVar, span: Span) -> Self {
        StackTy { elems, row, span }
    }
}

/// A word's type: an arrow between two stack types (§2, §3). A quotation value
/// carries one of these inside [`TyKind::Quote`].
#[derive(Clone, Debug, PartialEq)]
pub struct WordTy {
    /// The stack before the word runs.
    pub input: StackTy,
    /// The stack after the word runs.
    pub output: StackTy,
    /// **Tier 1 refinement payload (§3, reserved here).** The optional `pre`/`post`
    /// predicate signature attached to a quotation arrow (demands on inputs,
    /// guarantees on outputs). **Tier 0 never reads this** — it is forwarded
    /// untouched and the unifier matches on shape (`input`/`output`) alone
    /// (invariant 10). It wakes up only at the VC boundary (§10), which is a later
    /// milestone. For M6 it is a parse-only payload: the [`crate::RefinementSig`]
    /// the §10.1 parser produces lands here so the shape exists for later tiers.
    pub refinement: Option<Box<crate::refinement::RefinementSig>>,
}

impl WordTy {
    /// Construct a Tier-0 arrow from `input` to `output` with **no** refinement
    /// payload (the common path: Tier 0 builds shape-only arrows).
    pub fn new(input: StackTy, output: StackTy) -> Self {
        WordTy {
            input,
            output,
            refinement: None,
        }
    }

    /// Construct an arrow carrying a Tier 1 refinement payload (§3). The payload
    /// is **forwarded untouched** and never read by Tier 0; this is how a parsed
    /// [`crate::RefinementSig`] is wired into the §3-reserved `Quote` shape.
    pub fn with_refinement(
        input: StackTy,
        output: StackTy,
        refinement: crate::refinement::RefinementSig,
    ) -> Self {
        WordTy {
            input,
            output,
            refinement: Some(Box::new(refinement)),
        }
    }
}

/// Render a word stack-effect type in Caternary surface notation.
///
/// Internal variable ids are renamed to stable, user-facing names in first-use
/// order: row variables render as `'S`, `'T`, ... and element variables as
/// `a`, `b`, ... . The rendered form is intended for diagnostics and shell
/// inspection, for example `( 'S Num -- 'S Num )`.
pub fn format_word_type(word: &WordTy) -> String {
    let mut renderer = TypeRenderer::default();
    renderer.render_word(word)
}

#[derive(Default)]
struct TypeRenderer {
    rows: Vec<RowVar>,
    tys: Vec<TyVar>,
}

impl TypeRenderer {
    fn render_word(&mut self, word: &WordTy) -> String {
        format!(
            "( {} -- {} )",
            self.render_stack(&word.input),
            self.render_stack(&word.output)
        )
    }

    fn render_stack(&mut self, stack: &StackTy) -> String {
        let mut parts = Vec::with_capacity(stack.elems.len() + 1);
        parts.push(self.row_name(stack.row));
        parts.extend(stack.elems.iter().map(|ty| self.render_ty(ty)));
        parts.join(" ")
    }

    fn render_ty(&mut self, ty: &Ty) -> String {
        match &ty.kind {
            TyKind::Var(v) => self.ty_name(*v),
            TyKind::Con(name) => name.clone(),
            TyKind::App(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let args: Vec<String> = args.iter().map(|arg| self.render_ty(arg)).collect();
                    format!("{name} {}", args.join(" "))
                }
            }
            TyKind::Quote(word) => self.render_word(word),
        }
    }

    fn row_name(&mut self, row: RowVar) -> String {
        let idx = match self.rows.iter().position(|&r| r == row) {
            Some(idx) => idx,
            None => {
                self.rows.push(row);
                self.rows.len() - 1
            }
        };
        const ROW_NAMES: &[&str] = &["'S", "'T", "'U", "'V", "'W", "'X", "'Y", "'Z"];
        ROW_NAMES
            .get(idx)
            .map(|name| (*name).to_string())
            .unwrap_or_else(|| format!("'R{}", idx - ROW_NAMES.len() + 1))
    }

    fn ty_name(&mut self, ty: TyVar) -> String {
        let idx = match self.tys.iter().position(|&t| t == ty) {
            Some(idx) => idx,
            None => {
                self.tys.push(ty);
                self.tys.len() - 1
            }
        };
        alpha_name(idx)
    }
}

fn alpha_name(mut idx: usize) -> String {
    let mut chars = Vec::new();
    loop {
        chars.push((b'a' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    chars.iter().rev().collect()
}

/// A polymorphic type scheme: a [`WordTy`] generalized over type and row
/// variables (§3). Operators are registered with a `Scheme`; definitions are
/// generalized into one by the SCC pass (M3, later).
#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    /// The generalized type variables.
    pub tyvars: Vec<TyVar>,
    /// The generalized row variables.
    pub rowvars: Vec<RowVar>,
    /// The body arrow.
    pub ty: WordTy,
}

impl Scheme {
    /// A scheme generalizing `ty` over `tyvars` and `rowvars`.
    pub fn new(tyvars: Vec<TyVar>, rowvars: Vec<RowVar>, ty: WordTy) -> Self {
        Scheme {
            tyvars,
            rowvars,
            ty,
        }
    }

    /// A monomorphic scheme (no generalized variables).
    pub fn monomorphic(ty: WordTy) -> Self {
        Scheme {
            tyvars: Vec::new(),
            rowvars: Vec::new(),
            ty,
        }
    }
}

/// One entry in the substitution undo log: a slot and the value it held before
/// the most recent binding (§3 invariant 1). Recording the *old* value is what
/// lets [`Subst::rewind`] restore the arena to an earlier checkpoint.
#[derive(Clone, Debug)]
enum TrailEntry {
    Ty(TyVar, Option<Ty>),
    Row(RowVar, Option<StackTy>),
}

/// The flat Tier-0 substitution arena with a trailed undo log (§3 invariant 1,
/// ratified decision (b)).
///
/// This is deliberately **not** a persistent HAMT and **not** destructive
/// union-find. Bindings live in flat vectors indexed by variable id, giving
/// O(1)/zero-allocation mutation; speculative inference backtracking is served
/// by the trail: [`Subst::bind_ty`]/[`Subst::bind_row`] push `(slot, old_value)`,
/// a [`Subst::checkpoint`] records the trail length, and [`Subst::rewind`]
/// unwinds back to it.
///
/// A trail gives *undo*, not *queryable history*: after a rewind the bindings are
/// gone. That is acceptable because the durable provenance spine is the
/// [`FrameStack`] (§3 invariant 3), not the substitution.
///
/// **Tier-0 only.** This trail is not the Tier 1 logical-scope mechanism. Tier 1
/// treats this map as read-only (§3 invariant 18); its `push_scope`/`pop_scope`
/// belong to the (future) Z3 trait and shadow evaluator, never to this arena.
#[derive(Clone, Debug, Default)]
pub struct Subst {
    ty_bindings: Vec<Option<Ty>>,
    row_bindings: Vec<Option<StackTy>>,
    trail: Vec<TrailEntry>,
}

impl Subst {
    /// A fresh, empty substitution.
    pub fn new() -> Self {
        Subst::default()
    }

    fn ensure_ty_slot(&mut self, v: TyVar) {
        let idx = v as usize;
        if idx >= self.ty_bindings.len() {
            self.ty_bindings.resize(idx + 1, None);
        }
    }

    fn ensure_row_slot(&mut self, v: RowVar) {
        let idx = v as usize;
        if idx >= self.row_bindings.len() {
            self.row_bindings.resize(idx + 1, None);
        }
    }

    /// The current binding of a type variable, if any (one step, not chased).
    pub fn get_ty(&self, v: TyVar) -> Option<&Ty> {
        self.ty_bindings.get(v as usize).and_then(Option::as_ref)
    }

    /// The current binding of a row variable, if any (one step, not chased).
    pub fn get_row(&self, v: RowVar) -> Option<&StackTy> {
        self.row_bindings.get(v as usize).and_then(Option::as_ref)
    }

    /// Bind a type variable, recording the previous value on the trail so the
    /// binding can be undone by [`Subst::rewind`].
    pub fn bind_ty(&mut self, v: TyVar, ty: Ty) {
        self.ensure_ty_slot(v);
        let old = self.ty_bindings[v as usize].take();
        self.trail.push(TrailEntry::Ty(v, old));
        self.ty_bindings[v as usize] = Some(ty);
    }

    /// Bind a row variable, recording the previous value on the trail so the
    /// binding can be undone by [`Subst::rewind`].
    pub fn bind_row(&mut self, v: RowVar, stack: StackTy) {
        self.ensure_row_slot(v);
        let old = self.row_bindings[v as usize].take();
        self.trail.push(TrailEntry::Row(v, old));
        self.row_bindings[v as usize] = Some(stack);
    }

    /// Record a backtrack point: the current trail length. Pass the returned
    /// value to [`Subst::rewind`] to undo every binding made since.
    pub fn checkpoint(&self) -> usize {
        self.trail.len()
    }

    /// Undo every binding made since `checkpoint`, restoring the arena exactly to
    /// the state it had then. Bindings are restored in reverse order so nested
    /// rebindings of the same slot unwind correctly.
    pub fn rewind(&mut self, checkpoint: usize) {
        while self.trail.len() > checkpoint {
            match self
                .trail
                .pop()
                .expect("trail length checked by loop guard")
            {
                TrailEntry::Ty(v, old) => {
                    self.ty_bindings[v as usize] = old;
                }
                TrailEntry::Row(v, old) => {
                    self.row_bindings[v as usize] = old;
                }
            }
        }
    }
}

/// One typing frame: the span and expected effect recorded on descent into a
/// quotation `[ … ]` (§3 invariant 3).
#[derive(Clone, Debug, PartialEq)]
pub struct TypingFrame {
    /// The span of the quotation whose checking this frame covers.
    pub span: Span,
    /// The effect expected at entry to the quotation.
    pub expected: WordTy,
}

/// The typing-frame stack: the durable provenance spine (§3 invariant 3).
///
/// Descending into a quotation pushes a [`TypingFrame`] recording its span and
/// expected effect; leaving it pops. The frame stack is the compile-time
/// backtrace used for error breadcrumbs (§7) **and** the fallback for any
/// provenance the substitution trail has unwound ([`Subst`] invariant 1: trail =
/// inference speed, frames = error history).
///
/// Frames are a *logical* spine: although `pop` removes a frame from the live
/// stack on scope exit, frame information persists in any diagnostic captured
/// while the frame was live. The substitution may forget; the frame breadcrumb
/// recorded into an error does not. This is why frames "persist across scope
/// pops" — an error built from the frame stack keeps its breadcrumb after the
/// frame leaves the live stack.
#[derive(Clone, Debug, Default)]
pub struct FrameStack {
    frames: Vec<TypingFrame>,
}

impl FrameStack {
    /// A fresh, empty frame stack.
    pub fn new() -> Self {
        FrameStack::default()
    }

    /// Push a frame on descent into a quotation.
    pub fn push(&mut self, frame: TypingFrame) {
        self.frames.push(frame);
    }

    /// Pop the innermost frame on leaving a quotation.
    pub fn pop(&mut self) -> Option<TypingFrame> {
        self.frames.pop()
    }

    /// The current nesting depth.
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// A snapshot of the live frames, innermost last. Used to capture a durable
    /// breadcrumb into a diagnostic — the snapshot outlives later `pop`s.
    pub fn breadcrumb(&self) -> Vec<TypingFrame> {
        self.frames.clone()
    }
}

/// The Tier-0 inference context: a fresh-variable allocator, the substitution
/// arena, and the typing-frame stack. Holds the §3 substrate together so later
/// milestones (M1 unifier, M2 inference) build on one coherent state.
#[derive(Clone, Debug, Default)]
pub struct InferCtx {
    /// The flat substitution arena + trail.
    pub subst: Subst,
    /// The durable typing-frame stack.
    pub frames: FrameStack,
    next_ty: TyVar,
    next_row: RowVar,
}

impl InferCtx {
    /// A fresh inference context.
    pub fn new() -> Self {
        InferCtx::default()
    }

    /// Allocate a fresh type variable.
    pub fn fresh_ty(&mut self) -> TyVar {
        let v = self.next_ty;
        self.next_ty += 1;
        v
    }

    /// Allocate a fresh row variable.
    pub fn fresh_row(&mut self) -> RowVar {
        let v = self.next_row;
        self.next_row += 1;
        v
    }

    /// The Tier-0 initial program stack type: the identity arrow `( 'a -- 'a )`
    /// over one fresh row variable, with empty observed elements. This is the
    /// effect a whole-program `main` entry must close against (§12: "Initial
    /// stack for a top-level program is empty"). Both `input` and `output` share
    /// the same fresh row var, encoding "starts empty, ends empty modulo the
    /// untouched tail".
    pub fn empty_program_effect(&mut self, span: Span) -> WordTy {
        let row = self.fresh_row();
        WordTy::new(StackTy::empty(row, span), StackTy::empty(row, span))
    }

    /// Instantiate a [`Scheme`] with fresh variables, demonstrating that a
    /// registered operator's scheme is *usable*: each generalized variable is
    /// renamed to a fresh one so distinct uses do not alias (§5 `instantiate`).
    /// The returned [`WordTy`] re-spans nothing; nodes keep their birth spans.
    pub fn instantiate(&mut self, scheme: &Scheme) -> WordTy {
        let ty_map: Vec<(TyVar, TyVar)> = scheme
            .tyvars
            .iter()
            .map(|&old| (old, self.fresh_ty()))
            .collect();
        let row_map: Vec<(RowVar, RowVar)> = scheme
            .rowvars
            .iter()
            .map(|&old| (old, self.fresh_row()))
            .collect();
        rename_word(&scheme.ty, &ty_map, &row_map)
    }
}

/// A failure raised by the Tier-0 unifier (§4, §7).
///
/// Both variants carry **origin spans**, not a single failure site: §7's first
/// requirement is a *provenance pair*. A `Mismatch` holds the birth span of each
/// of the two conflicting types; a `Cyclic` holds the span of the offending node
/// that closed the loop. Internal variable names are never surfaced here — the
/// `detail` strings describe shapes (`Num`, `Bool`, an arrow), never `'t7` (§7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifyError {
    /// Two element types could not be made equal. `left`/`right` describe the two
    /// conflicting shapes; `left_span`/`right_span` are where each was born.
    Mismatch {
        /// Human-readable description of the left shape (never a variable name).
        left: String,
        /// Birth span of the left type.
        left_span: Span,
        /// Human-readable description of the right shape (never a variable name).
        right: String,
        /// Birth span of the right type.
        right_span: Span,
    },
    /// A type/row variable was about to be bound to a term that contains it: a
    /// cyclic (infinite) type. Rejected cleanly per §4 / §13 invariant 4.
    Cyclic {
        /// A description of the offending term's shape (never a variable name).
        detail: String,
        /// The span of the node that closed the cycle.
        span: Span,
    },
}

impl std::fmt::Display for UnifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnifyError::Mismatch {
                left,
                left_span,
                right,
                right_span,
            } => write!(
                f,
                "type mismatch: `{left}` (from byte {}) cannot unify with `{right}` (from byte {})",
                left_span.start, right_span.start
            ),
            UnifyError::Cyclic { detail, span } => write!(
                f,
                "cyclic type: a type would contain itself (`{detail}` at byte {})",
                span.start
            ),
        }
    }
}

impl std::error::Error for UnifyError {}

/// The recursion guard for unification and the occurs check (§4 / §13 invariant
/// 4). With occurs-before-every-bind the substitution graph stays acyclic, so
/// honest terms terminate well within this bound; the cap exists so a malicious
/// or accidental cyclic program degrades to a *typed cyclic-type error* instead
/// of a compiler stack overflow.
const UNIFY_MAX_DEPTH: usize = 10_000;

/// Render a type's shape for a diagnostic, without leaking internal variable
/// names (§7: never surface `'t7`). A variable renders as the neutral word
/// `a value`; concrete shapes render structurally.
fn describe_ty(ty: &Ty) -> String {
    match &ty.kind {
        TyKind::Var(_) => "a value".to_string(),
        TyKind::Con(name) => name.clone(),
        TyKind::App(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(describe_ty).collect();
                format!("{name} {}", inner.join(" "))
            }
        }
        TyKind::Quote(_) => "a quotation".to_string(),
    }
}

impl InferCtx {
    /// Chase top-level type-variable bindings to a representative: the result is
    /// either a non-`Var` shape or an unbound `Var`. Interiors are **not**
    /// rewritten — callers that need deep canonicalization recurse themselves
    /// (occurs and `describe`). This is the "canonicalize, then check" step §4
    /// mandates before the occurs check.
    pub fn resolve_ty(&self, ty: &Ty) -> Ty {
        let mut cur = ty.clone();
        let mut guard = 0usize;
        while let TyKind::Var(v) = cur.kind {
            match self.subst.get_ty(v) {
                Some(bound) => {
                    cur = bound.clone();
                    guard += 1;
                    if guard > UNIFY_MAX_DEPTH {
                        // An acyclic substitution can never loop here; bail to a
                        // representative rather than spin.
                        break;
                    }
                }
                None => break,
            }
        }
        cur
    }

    /// Fully resolve a stack type: chase every row binding so all observed
    /// elements are exposed and the resulting `row` is an *unbound* row variable.
    ///
    /// A row stands for "whatever is underneath, untouched"; if `s.row` is bound
    /// to `StackTy { elems2, row2 }`, those `elems2` sit **below** `s.elems`
    /// (top-of-stack is last), so the flattened element list is
    /// `elems2 ++ s.elems` and the tail chases on to `row2`.
    pub fn resolve_stack(&self, s: &StackTy) -> StackTy {
        let mut elems = s.elems.clone();
        let mut row = s.row;
        let span = s.span;
        let mut guard = 0usize;
        while let Some(bound) = self.subst.get_row(row) {
            let mut prefix = bound.elems.clone();
            prefix.extend(elems);
            elems = prefix;
            row = bound.row;
            guard += 1;
            if guard > UNIFY_MAX_DEPTH {
                break;
            }
        }
        StackTy { elems, row, span }
    }

    /// Fully resolve an element type: chase the top-level variable binding *and*
    /// recurse into every interior (`App` arguments, `Quote` arrows) so the
    /// result carries no further-resolvable variables. This is the read path the
    /// inference driver and its tests use to inspect a *final* inferred type;
    /// [`InferCtx::resolve_ty`] is the shallow one-step chase the unifier uses.
    pub fn resolve_ty_deep(&self, ty: &Ty) -> Ty {
        let t = self.resolve_ty(ty);
        match &t.kind {
            TyKind::Var(_) | TyKind::Con(_) => t,
            TyKind::App(name, args) => Ty {
                kind: TyKind::App(
                    name.clone(),
                    args.iter().map(|a| self.resolve_ty_deep(a)).collect(),
                ),
                span: t.span,
            },
            TyKind::Quote(w) => Ty {
                kind: TyKind::Quote(Box::new(self.resolve_word_deep(w))),
                span: t.span,
            },
        }
    }

    /// Fully resolve a stack type: chase the row to an unbound representative and
    /// deep-resolve every element (see [`InferCtx::resolve_ty_deep`]).
    pub fn resolve_stack_deep(&self, s: &StackTy) -> StackTy {
        let r = self.resolve_stack(s);
        StackTy {
            elems: r.elems.iter().map(|e| self.resolve_ty_deep(e)).collect(),
            row: r.row,
            span: r.span,
        }
    }

    /// Fully resolve both stacks of a word arrow (see
    /// [`InferCtx::resolve_stack_deep`]). The canonical form of an inferred
    /// effect, used to report and assert a sequence's principal type.
    pub fn resolve_word_deep(&self, w: &WordTy) -> WordTy {
        WordTy {
            input: self.resolve_stack_deep(&w.input),
            output: self.resolve_stack_deep(&w.output),
            // Resolution chases type/row bindings; the refinement payload (§3) is
            // over named binders, so it is forwarded untouched — never read.
            refinement: w.refinement.clone(),
        }
    }

    /// Occurs check for a **type** variable: does `v` appear anywhere in `term`,
    /// after canonicalization? Recurses **uniformly into both stack types of every
    /// `Quote` arrow** (§4: "do not skip arrow interiors"). Depth-bounded so a
    /// pathological term cannot overflow the stack (§13 invariant 4).
    fn occurs_ty(&self, v: TyVar, term: &Ty, depth: usize) -> bool {
        if depth > UNIFY_MAX_DEPTH {
            return true;
        }
        let term = self.resolve_ty(term);
        match &term.kind {
            TyKind::Var(u) => *u == v,
            TyKind::Con(_) => false,
            TyKind::App(_, args) => args.iter().any(|a| self.occurs_ty(v, a, depth + 1)),
            TyKind::Quote(w) => {
                self.occurs_ty_in_stack(v, &w.input, depth + 1)
                    || self.occurs_ty_in_stack(v, &w.output, depth + 1)
            }
        }
    }

    /// Occurs check for a type variable, scanning a stack's element types (a row
    /// variable lives in a different namespace, so only elements can hold `v`).
    fn occurs_ty_in_stack(&self, v: TyVar, s: &StackTy, depth: usize) -> bool {
        if depth > UNIFY_MAX_DEPTH {
            return true;
        }
        let s = self.resolve_stack(s);
        s.elems.iter().any(|e| self.occurs_ty(v, e, depth + 1))
    }

    /// Occurs check for a **row** variable over a stack type: does `v` appear as
    /// the tail of this stack, or inside any `Quote` interior reachable from its
    /// elements? Canonicalizes first and recurses uniformly through arrows.
    fn occurs_row(&self, v: RowVar, s: &StackTy, depth: usize) -> bool {
        if depth > UNIFY_MAX_DEPTH {
            return true;
        }
        let s = self.resolve_stack(s);
        if s.row == v {
            return true;
        }
        s.elems
            .iter()
            .any(|e| self.occurs_row_in_ty(v, e, depth + 1))
    }

    /// Occurs check for a row variable, descending into a type's `Quote` interiors.
    fn occurs_row_in_ty(&self, v: RowVar, term: &Ty, depth: usize) -> bool {
        if depth > UNIFY_MAX_DEPTH {
            return true;
        }
        let term = self.resolve_ty(term);
        match &term.kind {
            TyKind::Var(_) | TyKind::Con(_) => false,
            TyKind::App(_, args) => args.iter().any(|a| self.occurs_row_in_ty(v, a, depth + 1)),
            TyKind::Quote(w) => {
                self.occurs_row(v, &w.input, depth + 1) || self.occurs_row(v, &w.output, depth + 1)
            }
        }
    }

    /// Bind a type variable to a term, doing the full §4 sequence:
    /// (1) canonicalize the term, (2) run the occurs check, (3) on a positive
    /// occurrence emit a typed cyclic-type error, (4) otherwise write the binding.
    fn bind_ty_checked(&mut self, v: TyVar, term: &Ty) -> Result<(), UnifyError> {
        let term = self.resolve_ty(term);
        if let TyKind::Var(u) = term.kind
            && u == v
        {
            return Ok(());
        }
        if self.occurs_ty(v, &term, 0) {
            return Err(UnifyError::Cyclic {
                detail: describe_ty(&term),
                span: term.span,
            });
        }
        self.subst.bind_ty(v, term);
        Ok(())
    }

    /// Bind a row variable to a stack, with the same canonicalize-then-occurs
    /// discipline as [`InferCtx::bind_ty_checked`].
    fn bind_row_checked(&mut self, v: RowVar, stack: &StackTy) -> Result<(), UnifyError> {
        let stack = self.resolve_stack(stack);
        if stack.elems.is_empty() && stack.row == v {
            return Ok(());
        }
        if self.occurs_row(v, &stack, 0) {
            return Err(UnifyError::Cyclic {
                detail: "a stack that contains itself".to_string(),
                span: stack.span,
            });
        }
        self.subst.bind_row(v, stack);
        Ok(())
    }

    /// Unify two row variables: equal ⇒ done; else bind one to the other (as an
    /// empty stack carrying the other row), via the checked bind path.
    fn unify_row(&mut self, a: RowVar, b: RowVar, span: Span) -> Result<(), UnifyError> {
        if a == b {
            return Ok(());
        }
        self.bind_row_checked(a, &StackTy::empty(b, span))
    }

    /// Unify two element types (§4 `unify_ty`).
    ///
    /// `Var` binds (via the checked bind path); `Con(n)`/`Con(m)` agree iff
    /// `n == m`; `App` agrees iff same head + arity, then unifies args pairwise;
    /// `Quote(p)`/`Quote(q)` unifies `p.input ~ q.input` **and**
    /// `p.output ~ q.output` component-wise (it does **not** generalize here); any
    /// other pairing is a typed mismatch carrying both origin spans (§7). No
    /// label/record/field machinery exists (§13 invariant 3).
    pub fn unify_ty(&mut self, a: &Ty, b: &Ty) -> Result<(), UnifyError> {
        self.unify_ty_d(a, b, 0)
    }

    fn unify_ty_d(&mut self, a: &Ty, b: &Ty, depth: usize) -> Result<(), UnifyError> {
        if depth > UNIFY_MAX_DEPTH {
            return Err(UnifyError::Cyclic {
                detail: describe_ty(a),
                span: a.span,
            });
        }
        let a = self.resolve_ty(a);
        let b = self.resolve_ty(b);
        match (&a.kind, &b.kind) {
            (TyKind::Var(va), TyKind::Var(vb)) if va == vb => Ok(()),
            (TyKind::Var(va), _) => self.bind_ty_checked(*va, &b),
            (_, TyKind::Var(vb)) => self.bind_ty_checked(*vb, &a),
            (TyKind::Con(n), TyKind::Con(m)) => {
                if n == m {
                    Ok(())
                } else {
                    Err(UnifyError::Mismatch {
                        left: describe_ty(&a),
                        left_span: a.span,
                        right: describe_ty(&b),
                        right_span: b.span,
                    })
                }
            }
            (TyKind::App(n, xs), TyKind::App(m, ys)) => {
                if n != m || xs.len() != ys.len() {
                    return Err(UnifyError::Mismatch {
                        left: describe_ty(&a),
                        left_span: a.span,
                        right: describe_ty(&b),
                        right_span: b.span,
                    });
                }
                for (x, y) in xs.iter().zip(ys.iter()) {
                    self.unify_ty_d(x, y, depth + 1)?;
                    // Re-resolve happens lazily inside unify_ty_d via resolve_ty.
                }
                Ok(())
            }
            (TyKind::Quote(p), TyKind::Quote(q)) => {
                // Component-wise; no generalization here (§4).
                self.unify_stack_d(&p.input, &q.input, depth + 1)?;
                self.unify_stack_d(&p.output, &q.output, depth + 1)
            }
            _ => Err(UnifyError::Mismatch {
                left: describe_ty(&a),
                left_span: a.span,
                right: describe_ty(&b),
                right_span: b.span,
            }),
        }
    }

    /// Unify two stack types head-directed (§4 `unify_stack`).
    ///
    /// Peel matching elements from the **top** (last element first), re-applying
    /// the substitution after each step; then if both sides are exhausted unify
    /// the tail rows, and if exactly one side is exhausted let its row **absorb**
    /// the other side's remaining elements + row.
    pub fn unify_stack(&mut self, a: &StackTy, b: &StackTy) -> Result<(), UnifyError> {
        self.unify_stack_d(a, b, 0)
    }

    fn unify_stack_d(&mut self, a: &StackTy, b: &StackTy, depth: usize) -> Result<(), UnifyError> {
        if depth > UNIFY_MAX_DEPTH {
            return Err(UnifyError::Cyclic {
                detail: "a stack that contains itself".to_string(),
                span: a.span,
            });
        }
        // Canonicalize both sides so any row already bound to a concrete stack
        // exposes its elements before we peel (transitive canonicalization, §4).
        let mut ra = self.resolve_stack(a);
        let mut rb = self.resolve_stack(b);
        while !ra.elems.is_empty() && !rb.elems.is_empty() {
            // pop_last from each (top of stack) and unify.
            let ta = ra.elems.pop().expect("nonempty checked by loop guard");
            let tb = rb.elems.pop().expect("nonempty checked by loop guard");
            self.unify_ty_d(&ta, &tb, depth + 1)?;
            // Re-apply the substitution: a binding made above may turn a row into
            // a longer stack, so re-resolve before the next peel.
            ra = self.resolve_stack(&ra);
            rb = self.resolve_stack(&rb);
        }
        if ra.elems.is_empty() && rb.elems.is_empty() {
            self.unify_row(ra.row, rb.row, ra.span)
        } else if ra.elems.is_empty() {
            // ra is just a row; absorb rb's remaining elems + row.
            self.bind_row_checked(ra.row, &rb)
        } else {
            self.bind_row_checked(rb.row, &ra)
        }
    }
}

fn map_ty(v: TyVar, ty_map: &[(TyVar, TyVar)]) -> TyVar {
    ty_map
        .iter()
        .find(|(old, _)| *old == v)
        .map(|(_, new)| *new)
        .unwrap_or(v)
}

fn map_row(v: RowVar, row_map: &[(RowVar, RowVar)]) -> RowVar {
    row_map
        .iter()
        .find(|(old, _)| *old == v)
        .map(|(_, new)| *new)
        .unwrap_or(v)
}

fn rename_ty(ty: &Ty, ty_map: &[(TyVar, TyVar)], row_map: &[(RowVar, RowVar)]) -> Ty {
    let kind = match &ty.kind {
        TyKind::Var(v) => TyKind::Var(map_ty(*v, ty_map)),
        TyKind::Con(name) => TyKind::Con(name.clone()),
        TyKind::App(name, args) => TyKind::App(
            name.clone(),
            args.iter().map(|a| rename_ty(a, ty_map, row_map)).collect(),
        ),
        TyKind::Quote(arrow) => TyKind::Quote(Box::new(rename_word(arrow, ty_map, row_map))),
    };
    Ty {
        kind,
        span: ty.span,
    }
}

fn rename_stack(
    stack: &StackTy,
    ty_map: &[(TyVar, TyVar)],
    row_map: &[(RowVar, RowVar)],
) -> StackTy {
    StackTy {
        elems: stack
            .elems
            .iter()
            .map(|e| rename_ty(e, ty_map, row_map))
            .collect(),
        row: map_row(stack.row, row_map),
        span: stack.span,
    }
}

fn rename_word(word: &WordTy, ty_map: &[(TyVar, TyVar)], row_map: &[(RowVar, RowVar)]) -> WordTy {
    WordTy {
        input: rename_stack(&word.input, ty_map, row_map),
        output: rename_stack(&word.output, ty_map, row_map),
        // The refinement payload (§3) is over named binders, not type/row
        // variables, so renaming never touches it — forward it untouched.
        refinement: word.refinement.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp() -> Span {
        Span { start: 0, end: 1 }
    }

    #[test]
    fn every_node_carries_a_span_from_parser() {
        // The span type is parser::Span re-exported; no invented span type.
        let s = sp();
        let t = Ty::num(s);
        assert_eq!(t.span, s);
        let st = StackTy::new(vec![Ty::num(s), Ty::var(0, s)], 0, s);
        assert_eq!(st.span, s);
        for e in &st.elems {
            assert_eq!(e.span, s);
        }
    }

    #[test]
    fn stack_top_is_last() {
        let s = sp();
        // ( 'S a b ) with top-of-stack b last.
        let st = StackTy::new(vec![Ty::var(0, s), Ty::con("a", s), Ty::con("b", s)], 9, s);
        assert_eq!(st.elems.last().unwrap().kind, TyKind::Con("b".into()));
    }

    #[test]
    fn subst_bind_and_read() {
        let mut subst = Subst::new();
        assert!(subst.get_ty(0).is_none());
        subst.bind_ty(0, Ty::num(sp()));
        assert_eq!(subst.get_ty(0).unwrap().kind, TyKind::Con(NUM.into()));
        subst.bind_row(3, StackTy::empty(7, sp()));
        assert_eq!(subst.get_row(3).unwrap().row, 7);
    }

    #[test]
    fn trail_rewinds_to_checkpoint() {
        let mut subst = Subst::new();
        subst.bind_ty(0, Ty::num(sp()));
        let cp = subst.checkpoint();
        subst.bind_ty(1, Ty::bool(sp()));
        subst.bind_row(2, StackTy::empty(5, sp()));
        // rebind an already-bound slot; rewind must restore the OLD value.
        subst.bind_ty(0, Ty::bool(sp()));
        assert_eq!(subst.get_ty(0).unwrap().kind, TyKind::Con(BOOL.into()));

        subst.rewind(cp);
        // Everything since the checkpoint is undone.
        assert_eq!(subst.get_ty(0).unwrap().kind, TyKind::Con(NUM.into()));
        assert!(subst.get_ty(1).is_none());
        assert!(subst.get_row(2).is_none());
    }

    #[test]
    fn frames_push_pop_and_breadcrumb_persists() {
        let s = sp();
        let mut frames = FrameStack::new();
        let effect = WordTy::new(StackTy::empty(0, s), StackTy::empty(0, s));
        frames.push(TypingFrame {
            span: s,
            expected: effect.clone(),
        });
        assert_eq!(frames.depth(), 1);
        // Capture a breadcrumb while the frame is live...
        let crumb = frames.breadcrumb();
        // ...then pop the frame off the live stack.
        frames.pop();
        assert_eq!(frames.depth(), 0);
        // The captured breadcrumb still carries the frame (durable spine).
        assert_eq!(crumb.len(), 1);
        assert_eq!(crumb[0].span, s);
    }

    #[test]
    fn instantiate_renames_to_fresh_vars() {
        let s = sp();
        // Scheme: ( 'R a -- 'R a a ) generalized over row 0 and ty 0 (DUP-like).
        let input = StackTy::new(vec![Ty::var(0, s)], 0, s);
        let output = StackTy::new(vec![Ty::var(0, s), Ty::var(0, s)], 0, s);
        let scheme = Scheme::new(vec![0], vec![0], WordTy::new(input, output));

        let mut ctx = InferCtx::new();
        let a = ctx.instantiate(&scheme);
        let b = ctx.instantiate(&scheme);

        // Two instantiations must not share variables.
        let row_a = a.input.row;
        let row_b = b.input.row;
        assert_ne!(row_a, row_b);
        // Within one instantiation the shared variable stays shared.
        assert_eq!(a.output.elems[0].kind, a.output.elems[1].kind);
    }

    #[test]
    fn empty_program_effect_is_identity_over_one_row() {
        let mut ctx = InferCtx::new();
        let eff = ctx.empty_program_effect(sp());
        assert!(eff.input.elems.is_empty());
        assert!(eff.output.elems.is_empty());
        assert_eq!(eff.input.row, eff.output.row);
    }

    // ----- §12 M1 acceptance: the unifier -----

    /// Helper: a row-only stack `( 'row -- )` shape born at `sp()`.
    fn row_stack(row: RowVar, elems: Vec<Ty>) -> StackTy {
        StackTy::new(elems, row, sp())
    }

    #[test]
    fn m1_top_peel_unifies_distinct_rows() {
        // ( 'a Num ) vs ( 'b Num ): peel the Num pair, then 'a ~ 'b.
        let s = sp();
        let mut ctx = InferCtx::new();
        let a = row_stack(0, vec![Ty::num(s)]);
        let b = row_stack(1, vec![Ty::num(s)]);
        ctx.unify_stack(&a, &b).expect("top-peel must unify");
        // 'a and 'b are now the same row: resolving 'a's tail reaches 'b (or
        // vice-versa). Unifying the two bare rows again is a no-op.
        let ra = ctx.resolve_stack(&row_stack(0, vec![]));
        let rb = ctx.resolve_stack(&row_stack(1, vec![]));
        assert_eq!(ra.row, rb.row, "'a and 'b must canonicalize to one row");
    }

    #[test]
    fn m1_mismatched_head_is_a_typed_mismatch() {
        // ( 'a Num ) vs ( 'b Bool ): the heads conflict; provenance pair reported.
        let s = sp();
        let mut ctx = InferCtx::new();
        let a = row_stack(0, vec![Ty::num(s)]);
        let b = row_stack(1, vec![Ty::bool(s)]);
        let err = ctx.unify_stack(&a, &b).unwrap_err();
        match err {
            UnifyError::Mismatch { left, right, .. } => {
                assert!(left == NUM || right == NUM);
                assert!(left == BOOL || right == BOOL);
            }
            other => panic!("expected a typed mismatch, got {other:?}"),
        }
    }

    #[test]
    fn m1_row_absorption_binds_the_short_row() {
        // ( 'a -- ) unifies with ( 'b Num Bool -- ) by 'a := 'b Num Bool.
        let s = sp();
        let mut ctx = InferCtx::new();
        let a = row_stack(0, vec![]);
        let b = row_stack(1, vec![Ty::num(s), Ty::bool(s)]);
        ctx.unify_stack(&a, &b).expect("row absorption must unify");
        // Resolving 'a now exposes [Num, Bool] over row 'b.
        let ra = ctx.resolve_stack(&row_stack(0, vec![]));
        assert_eq!(ra.row, 1, "'a's tail must be 'b after absorption");
        assert_eq!(ra.elems.len(), 2);
        assert_eq!(ra.elems[0].kind, TyKind::Con(NUM.into()));
        assert_eq!(ra.elems[1].kind, TyKind::Con(BOOL.into()));
    }

    #[test]
    fn m1_transitive_canonicalization_no_false_cyclic() {
        // 'r1 -> 'r2 -> concrete: a further unification canonicalizes correctly
        // before the occurs check, with no false cyclic rejection.
        let s = sp();
        let mut ctx = InferCtx::new();
        // Bind 'r1 := ( 'r2 ), then 'r2 := ( 'r3 Num ).
        ctx.subst.bind_row(1, StackTy::empty(2, s));
        ctx.subst.bind_row(2, StackTy::new(vec![Ty::num(s)], 3, s));
        // Resolving 'r1 should expose [Num] over 'r3 (transitive chase).
        let r1 = ctx.resolve_stack(&row_stack(1, vec![]));
        assert_eq!(r1.elems.len(), 1);
        assert_eq!(r1.row, 3);
        // Now unify ( 'r1 ) with ( 'b Num ): the Num peels, leaving 'r3 ~ 'b.
        let lhs = row_stack(1, vec![]);
        let rhs = row_stack(4, vec![Ty::num(s)]);
        ctx.unify_stack(&lhs, &rhs)
            .expect("transitive canonicalization must unify without false cyclic");
    }

    #[test]
    fn m1_occurs_rejects_a_eq_a_num() {
        // 'a := App("Num", ['a]) — 'a occurs inside the term it is bound to.
        let s = sp();
        let mut ctx = InferCtx::new();
        let a = Ty::var(0, s);
        let a_num = Ty {
            kind: TyKind::App(NUM.into(), vec![Ty::var(0, s)]),
            span: s,
        };
        let err = ctx.unify_ty(&a, &a_num).unwrap_err();
        assert!(
            matches!(err, UnifyError::Cyclic { .. }),
            "occurs must reject 'a := 'a Num as cyclic, got {err:?}"
        );
    }

    #[test]
    fn m1_cyclic_via_arrows_is_clean_rejection_no_overflow() {
        // `[ DUP CALL ] DUP CALL` forces a ~ ( 'X a -- 'Y ): a type variable that
        // must equal an arrow whose INPUT contains that very variable. Occurs
        // recurses into the Quote interior and rejects cleanly — no overflow.
        let s = sp();
        let mut ctx = InferCtx::new();
        let a = Ty::var(0, s); // the quote value on the stack
        // arrow ( 'X a -- 'Y ) with 'a (var 0) appearing in the input stack.
        let arrow = WordTy::new(
            StackTy::new(vec![Ty::var(0, s)], 10, s), // input: 'X a   (a == var 0)
            StackTy::empty(11, s),                    // output: 'Y
        );
        let quote = Ty::quote(arrow, s);
        let err = ctx.unify_ty(&a, &quote).unwrap_err();
        assert!(
            matches!(err, UnifyError::Cyclic { .. }),
            "cyclic-via-arrows must be a typed cyclic-type error, got {err:?}"
        );
    }

    #[test]
    fn m1_quote_arrows_unify_component_wise() {
        // Two quotation arrows unify by input AND output, component-wise.
        let s = sp();
        let mut ctx = InferCtx::new();
        // ( 'a Num -- 'a Bool ) and ( 'c X -- 'c Y ): peeling forces X~Num, Y~Bool
        // and the rows to agree.
        let left = WordTy::new(
            StackTy::new(vec![Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        );
        let right = WordTy::new(
            StackTy::new(vec![Ty::var(0, s)], 1, s),
            StackTy::new(vec![Ty::var(1, s)], 1, s),
        );
        ctx.unify_ty(&Ty::quote(left, s), &Ty::quote(right, s))
            .expect("quote arrows must unify component-wise");
        // var 0 ~ Num, var 1 ~ Bool.
        assert_eq!(ctx.resolve_ty(&Ty::var(0, s)).kind, TyKind::Con(NUM.into()));
        assert_eq!(
            ctx.resolve_ty(&Ty::var(1, s)).kind,
            TyKind::Con(BOOL.into())
        );
    }

    #[test]
    fn m1_quote_arrows_with_conflicting_components_mismatch() {
        // ( -- Num ) vs ( -- Bool ) inside a quote: a typed mismatch, not silent.
        let s = sp();
        let mut ctx = InferCtx::new();
        let left = WordTy::new(StackTy::empty(0, s), StackTy::new(vec![Ty::num(s)], 0, s));
        let right = WordTy::new(StackTy::empty(1, s), StackTy::new(vec![Ty::bool(s)], 1, s));
        let err = ctx
            .unify_ty(&Ty::quote(left, s), &Ty::quote(right, s))
            .unwrap_err();
        assert!(matches!(err, UnifyError::Mismatch { .. }));
    }

    #[test]
    fn m1_error_display_leaks_no_internal_variable_names() {
        // §7: diagnostics must never surface internal variable names like 't7.
        let s = Span { start: 5, end: 8 };
        let err = UnifyError::Mismatch {
            left: NUM.into(),
            left_span: s,
            right: BOOL.into(),
            right_span: Span { start: 12, end: 16 },
        };
        let text = err.to_string();
        assert!(
            !text.contains('\''),
            "no leading-quote variable names: {text}"
        );
        assert!(text.contains("byte 5") && text.contains("byte 12"));
    }

    // §3 / invariant 10: the Tier 1 refinement payload on a `Quote` arrow is
    // forwarded untouched and NEVER read by Tier 0. Two arrows that differ ONLY
    // in their refinement payload must unify identically (shape-only), and the
    // payload must survive resolution untouched.
    #[test]
    fn refinement_payload_is_inert_to_tier0_unification() {
        let s = sp();
        let sig = crate::parse_signature(
            "sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 and r * r = n )",
        )
        .expect("signature parses");

        // Same shape ( 'r -- 'r Num ), one refined and one bare.
        let refined = WordTy::with_refinement(
            StackTy::empty(0, s),
            StackTy::new(vec![Ty::num(s)], 0, s),
            sig.clone(),
        );
        let bare = WordTy::new(StackTy::empty(1, s), StackTy::new(vec![Ty::num(s)], 1, s));

        let mut ctx = InferCtx::new();
        // Tier 0 unifies on shape alone: the refinement payload is invisible.
        ctx.unify_ty(&Ty::quote(refined.clone(), s), &Ty::quote(bare, s))
            .expect("refined and bare quotes unify on shape");

        // The payload survives deep resolution untouched (forwarded, not read).
        let resolved = ctx.resolve_word_deep(&refined);
        assert_eq!(resolved.refinement.as_deref(), Some(&sig));
    }

    #[test]
    fn refinement_payload_does_not_change_mismatch_outcome() {
        // A shape mismatch is still a mismatch regardless of payload presence:
        // the unifier never consults the refinement to accept or reject.
        let s = sp();
        let sig = crate::parse_signature("f : ( a: Num  --  b: Num )").unwrap();
        let refined_num = WordTy::with_refinement(
            StackTy::empty(0, s),
            StackTy::new(vec![Ty::num(s)], 0, s),
            sig,
        );
        let bare_bool = WordTy::new(StackTy::empty(1, s), StackTy::new(vec![Ty::bool(s)], 1, s));
        let mut ctx = InferCtx::new();
        let err = ctx
            .unify_ty(&Ty::quote(refined_num, s), &Ty::quote(bare_bool, s))
            .unwrap_err();
        assert!(matches!(err, UnifyError::Mismatch { .. }));
    }

    #[test]
    fn numeric_literal_decision_is_recorded() {
        assert!(is_numeric_literal("1"));
        assert!(is_numeric_literal("42"));
        assert!(is_numeric_literal("-3.5"));
        assert!(!is_numeric_literal("DUP"));
        assert!(!is_numeric_literal(""));
        // There is no core `+`: it is not a numeric literal and must arrive via
        // registration.
        assert!(!is_numeric_literal("+"));
    }
}
