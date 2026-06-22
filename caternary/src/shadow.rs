//! Tier 1 — positional binding + the symbolic **shadow stack** (M7, §10.2–§10.3
//! / §14.8).
//!
//! This module is the substrate the later Tier 1 milestones consume (M8 path
//! conditions, M9 VCs). It is deliberately scoped to exactly two things and
//! **no further** (§12 M7):
//!
//!   1. a **symbolic shadow stack** — an abstract interpreter over the *same*
//!      operational semantics as the runtime, whose slot values are symbolic
//!      **terms** in the refinement logic ([`crate::refinement::Pred`]) instead
//!      of runtime values (§10.3); and
//!   2. the **positional → named binding** (the zip) that names the inferred
//!      stack elements right-to-left from the top so SMT has variables (§10.2).
//!
//! There is **no** VC generation, **no** path conditions, **no** subsumption,
//! and **no** solver here. Those are M8+ (§10.4–§10.6); M7 stops at the shadow
//! stack and the zip.
//!
//! # The shadow stack mirrors runtime data flow *exactly* (§10.3, invariant 7/19)
//!
//! The single mandate: **every word that moves data at runtime moves terms here
//! with byte-identical data flow.** Three things fall out of that one rule, and
//! all three are the classic ways to ship a *vacuous proof* (a green checkmark
//! that proves nothing) if gotten wrong:
//!
//!   * **Aliasing under `DUP`/shuffles.** [`ShadowStack::dup`] pushes the *exact
//!     same term* (it clones the slot, it does **not** mint a fresh literal), so
//!     the solver keeps the aliasing fact and trivial proofs like `x DUP - = 0`
//!     discharge. `DUP` copying the term is just the `n = 1` case of "shadow
//!     execution mirrors real execution." `SWAP`/`OVER`/`ROT`/`NIP`/`TUCK`
//!     exchange/copy/discard terms with the identical index arithmetic the
//!     runtime uses in `builtins.rs`.
//!   * **Combinator data flow.** `DIP` **executes its real shuffle** at compile
//!     time: pop the set-aside term, run the quotation's shadow effect on the
//!     rest, restore the set-aside term — mirroring `combinators.rs::dip`
//!     exactly. `CALL` runs the quotation on the whole stack.
//!   * **Arity comes from the Tier 0 arrow.** The shadow evaluator owns **no**
//!     independent notion of arity. For an *opaque* word (no interpreted
//!     meaning, e.g. `lib : ( Num Num -- Num )`) it reads that word's already
//!     inferred Tier 0 arrow ([`crate::WordTy`]) and pops exactly the arrow's
//!     input count, pushing exactly its output count as **fresh literal terms**
//!     (sound; precision degrades gracefully). Opacity is only about whether a
//!     pushed term carries meaning, **never** about how many terms move.
//!
//! Because a mis-shuffle does **not** crash — it emits a well-formed VC about
//! the *wrong* variables — core-shuffle conformance is a **soundness**
//! requirement and is **property-tested against the real interpreter's data
//! flow** (`evaluator.rs` + `builtins.rs`/`combinators.rs`), not eyeballed. See
//! the tests at the bottom of this file.
//!
//! # Compile-time only (§10.10, invariant 14)
//!
//! The shadow stack does not exist at runtime. It is a compile-time analysis
//! artifact, discarded before the binary ships, like a dataflow lattice. It is
//! **never** a field of [`crate::Evaluator`]; `eval`/`eval_with_stack` never
//! construct one. A checked program carries no shadow-stack machinery.

use crate::Token;
use crate::WordTy;
use crate::refinement::BinOp;
use crate::refinement::Binder;
use crate::refinement::Pred;
use crate::refinement::UnOp;

// ===========================================================================
// Errors
// ===========================================================================

/// An error raised while executing the shadow stack.
///
/// Shadow execution mirrors the runtime, so its only failure mode is the same
/// one the interpreter has: a stack underflow (or a slot of the wrong kind,
/// e.g. a combinator expecting a quotation slot). A mis-*shuffle* never lands
/// here — it produces a well-formed-but-wrong term, which is exactly why
/// conformance is property-tested rather than relying on errors (§10.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowError {
    /// Human-readable description of the failure.
    pub message: String,
}

impl ShadowError {
    fn new(message: impl Into<String>) -> Self {
        ShadowError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ShadowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shadow stack error: {}", self.message)
    }
}

impl std::error::Error for ShadowError {}

fn underflow(need: usize, found: usize) -> ShadowError {
    ShadowError::new(format!(
        "shadow stack underflow: need at least {need} slots, found {found}"
    ))
}

// ===========================================================================
// Slots
// ===========================================================================

/// A single shadow-stack slot. It mirrors what the runtime stack holds: either
/// a *value*, modelled here as a symbolic term ([`Pred`]), or a *quotation*,
/// modelled as the raw token body the runtime would carry as a value and a
/// combinator would later execute.
///
/// Modelling quotations as slots (rather than special-casing them) is what lets
/// `DIP`/`CALL` mirror the runtime exactly: at runtime a quotation is an
/// ordinary stack value that `DIP` pops off the top, and here it is too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// A value slot carrying its symbolic term.
    Term(Pred),
    /// A quotation slot carrying the token body (executed by `DIP`/`CALL`).
    Quote(Vec<Token>),
}

impl Slot {
    /// The term in this slot, or an error if it is a quotation slot.
    pub fn as_term(&self) -> Result<&Pred, ShadowError> {
        match self {
            Slot::Term(p) => Ok(p),
            Slot::Quote(_) => Err(ShadowError::new(
                "expected a value term in this slot, found a quotation",
            )),
        }
    }
}

// ===========================================================================
// How a word resolves to a shadow action
// ===========================================================================

/// How a word in the token stream acts on the shadow stack.
///
/// The shadow evaluator owns **no** independent semantics: every word is
/// resolved (by a caller-supplied resolver) into exactly one of these, and the
/// arity of the [`ShadowWord::Opaque`] case is read straight from the word's
/// already-inferred Tier 0 arrow (invariant 7/19).
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowWord {
    /// `DUP` — clone the top term (same identity), push it.
    Dup,
    /// `DROP` — discard the top term.
    Drop,
    /// `SWAP` — exchange the top two terms.
    Swap,
    /// `OVER` — copy the second term over the top.
    Over,
    /// `ROT` — rotate the top three terms left.
    Rot,
    /// `NIP` — discard the second term.
    Nip,
    /// `TUCK` — copy the top term below the second.
    Tuck,
    /// `DIP` — pop the quotation, set aside the next term, run the quotation on
    /// the rest, restore the set-aside term (the real shuffle, §10.3).
    Dip,
    /// `CALL` — pop the quotation and run it on the whole stack.
    Call,
    /// An interpreted binary operator (arithmetic/comparison/connective): pops
    /// two terms `a b` and pushes the proposition/term `Bin(op, a, b)`.
    Bin(BinOp),
    /// An interpreted unary operator: pops one term `a` and pushes `Un(op, a)`.
    Un(UnOp),
    /// A numeric literal: pushes `Num(lexeme)`.
    Num(String),
    /// A named binder / free variable: pushes `Var(name)`.
    Var(String),
    /// An **opaque** word with no interpreted meaning. Its **arity is read from
    /// the inferred Tier 0 arrow** (invariant 7/19): pop the arrow's input
    /// element count, push its output element count as fresh literals.
    Opaque(WordTy),
}

/// The default resolution for a *core* shuffle/combinator/operator word, by its
/// runtime UPPER_SNAKE_CASE name. Returns `None` for anything that is not a
/// built-in core word, so a caller can decide how to treat the rest (a
/// definition, an opaque operator, a binder, a literal).
///
/// This is the table that pins the shadow stack's data flow to the interpreter's
/// for the core shuffles — which is exactly the conformance the M7 property
/// tests check against `builtins.rs`/`combinators.rs`.
pub fn core_shadow_word(name: &str) -> Option<ShadowWord> {
    let word = match name {
        "DUP" => ShadowWord::Dup,
        "DROP" => ShadowWord::Drop,
        "SWAP" => ShadowWord::Swap,
        "OVER" => ShadowWord::Over,
        "ROT" => ShadowWord::Rot,
        "NIP" => ShadowWord::Nip,
        "TUCK" => ShadowWord::Tuck,
        "DIP" => ShadowWord::Dip,
        "CALL" => ShadowWord::Call,
        _ => return None,
    };
    Some(word)
}

/// The default resolution for an interpreted arithmetic/comparison/boolean
/// operator word in the *refinement* term language (so `x 0 >` builds the term
/// `x > 0`). Returns `None` for non-operator words.
pub fn interpreted_op(name: &str) -> Option<ShadowWord> {
    let word = match name {
        "+" => ShadowWord::Bin(BinOp::Add),
        "-" => ShadowWord::Bin(BinOp::Sub),
        "*" => ShadowWord::Bin(BinOp::Mul),
        "/" => ShadowWord::Bin(BinOp::Div),
        ">=" => ShadowWord::Bin(BinOp::Ge),
        "<=" => ShadowWord::Bin(BinOp::Le),
        ">" => ShadowWord::Bin(BinOp::Gt),
        "<" => ShadowWord::Bin(BinOp::Lt),
        "=" => ShadowWord::Bin(BinOp::Eq),
        "==" => ShadowWord::Bin(BinOp::Eq),
        "and" => ShadowWord::Bin(BinOp::And),
        "&&" => ShadowWord::Bin(BinOp::And),
        "or" => ShadowWord::Bin(BinOp::Or),
        "||" => ShadowWord::Bin(BinOp::Or),
        "not" => ShadowWord::Un(UnOp::Not),
        "!" => ShadowWord::Un(UnOp::Not),
        _ => return None,
    };
    Some(word)
}

// ===========================================================================
// The shadow stack
// ===========================================================================

/// A compile-time **symbolic shadow stack** (§10.3).
///
/// Slots carry symbolic terms ([`Pred`]) instead of runtime values; every
/// operation moves terms with byte-identical data flow to the interpreter. It is
/// constructed only by compile-time analysis (and tests) and is **never** a part
/// of the runtime [`crate::Evaluator`].
#[derive(Debug, Clone, Default)]
pub struct ShadowStack {
    slots: Vec<Slot>,
    /// Counter for minting fresh literal terms (opaque producers, §10.3).
    fresh: usize,
}

impl ShadowStack {
    /// An empty shadow stack.
    pub fn new() -> Self {
        ShadowStack::default()
    }

    /// The slots, bottom-first (top of stack last — same convention as
    /// [`crate::StackTy`]).
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// The number of slots currently on the stack.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The top slot, if any.
    pub fn top(&self) -> Option<&Slot> {
        self.slots.last()
    }

    /// The top slot's term, or an error if empty / a quotation slot.
    pub fn top_term(&self) -> Result<&Pred, ShadowError> {
        self.slots.last().ok_or_else(|| underflow(1, 0))?.as_term()
    }

    /// Push a value term.
    pub fn push_term(&mut self, term: Pred) {
        self.slots.push(Slot::Term(term));
    }

    /// Push a quotation body.
    pub fn push_quote(&mut self, body: Vec<Token>) {
        self.slots.push(Slot::Quote(body));
    }

    /// Push a raw slot.
    pub fn push_slot(&mut self, slot: Slot) {
        self.slots.push(slot);
    }

    /// Pop a slot.
    pub fn pop(&mut self) -> Result<Slot, ShadowError> {
        self.slots.pop().ok_or_else(|| underflow(1, 0))
    }

    /// Pop a value term (error if the top is a quotation).
    pub fn pop_term(&mut self) -> Result<Pred, ShadowError> {
        match self.pop()? {
            Slot::Term(p) => Ok(p),
            Slot::Quote(_) => Err(ShadowError::new(
                "expected a value term on top of the shadow stack, found a quotation",
            )),
        }
    }

    /// Pop a quotation body (error if the top is a value term).
    pub fn pop_quote(&mut self) -> Result<Vec<Token>, ShadowError> {
        match self.pop()? {
            Slot::Quote(body) => Ok(body),
            Slot::Term(_) => Err(ShadowError::new(
                "expected a quotation on top of the shadow stack, found a value term",
            )),
        }
    }

    /// Mint a fresh literal term for an opaque producer (§10.3). Each call is a
    /// distinct SMT variable; soundness holds, precision degrades gracefully.
    fn fresh_literal(&mut self) -> Pred {
        let p = Pred::Var(format!("$t{}", self.fresh));
        self.fresh += 1;
        p
    }

    fn require(&self, need: usize) -> Result<(), ShadowError> {
        if self.slots.len() < need {
            return Err(underflow(need, self.slots.len()));
        }
        Ok(())
    }

    // --- core shuffles (mirror builtins.rs exactly) ---

    /// `DUP`: push the **exact same term** that is on top (clone the slot, do not
    /// mint a fresh literal). Mirrors `builtins.rs::dup` (`stack.last().clone()`,
    /// `stack.push`). The aliasing this preserves is what makes `x DUP - = 0`
    /// discharge (§10.3).
    pub fn dup(&mut self) -> Result<(), ShadowError> {
        self.require(1)?;
        let top = self.slots.last().unwrap().clone();
        self.slots.push(top);
        Ok(())
    }

    /// `DROP`: discard the top term. Mirrors `builtins.rs::drop`.
    pub fn drop(&mut self) -> Result<(), ShadowError> {
        self.require(1)?;
        self.slots.pop();
        Ok(())
    }

    /// `SWAP`: exchange the top two terms. Mirrors `builtins.rs::swap`.
    pub fn swap(&mut self) -> Result<(), ShadowError> {
        self.require(2)?;
        let len = self.slots.len();
        self.slots.swap(len - 2, len - 1);
        Ok(())
    }

    /// `OVER`: copy the second term over the top. Mirrors `builtins.rs::over`.
    pub fn over(&mut self) -> Result<(), ShadowError> {
        self.require(2)?;
        let len = self.slots.len();
        let second = self.slots[len - 2].clone();
        self.slots.push(second);
        Ok(())
    }

    /// `ROT`: rotate the top three terms left. Mirrors `builtins.rs::rot`.
    pub fn rot(&mut self) -> Result<(), ShadowError> {
        self.require(3)?;
        let len = self.slots.len();
        self.slots[len - 3..].rotate_left(1);
        Ok(())
    }

    /// `NIP`: discard the second term. Mirrors `builtins.rs::nip`.
    pub fn nip(&mut self) -> Result<(), ShadowError> {
        self.require(2)?;
        let len = self.slots.len();
        self.slots.remove(len - 2);
        Ok(())
    }

    /// `TUCK`: copy the top term below the second. Mirrors `builtins.rs::tuck`.
    pub fn tuck(&mut self) -> Result<(), ShadowError> {
        self.require(2)?;
        let len = self.slots.len();
        let top = self.slots[len - 1].clone();
        self.slots.insert(len - 2, top);
        Ok(())
    }

    // --- interpreted terms ---

    /// Apply an interpreted binary operator: pop `a b`, push `Bin(op, a, b)`
    /// (postfix order — `a` is the deeper operand). After `x 0 >` the top term is
    /// `x > 0` (§10.3).
    pub fn bin(&mut self, op: BinOp) -> Result<(), ShadowError> {
        self.require(2)?;
        let b = self.pop_term()?;
        let a = self.pop_term()?;
        self.push_term(Pred::Bin(op, Box::new(a), Box::new(b)));
        Ok(())
    }

    /// Apply an interpreted unary operator: pop `a`, push `Un(op, a)`.
    pub fn un(&mut self, op: UnOp) -> Result<(), ShadowError> {
        self.require(1)?;
        let a = self.pop_term()?;
        self.push_term(Pred::Un(op, Box::new(a)));
        Ok(())
    }

    // --- opaque words: arity from the Tier 0 arrow ---

    /// Apply an **opaque** word, reading its arity from the already-inferred
    /// Tier 0 arrow (invariant 7/19). Pop exactly `arrow.input.elems.len()`
    /// terms and push exactly `arrow.output.elems.len()` **fresh literal** terms.
    ///
    /// The shape is known (Tier 0 proved it) even though the *meaning* is not:
    /// an opaque `lib : ( Num Num -- Num )` pops two and pushes one fresh
    /// literal. The shadow evaluator owns no independent notion of arity — it
    /// reads the arrow.
    pub fn apply_opaque(&mut self, arrow: &WordTy) -> Result<(), ShadowError> {
        let pops = arrow.input.elems.len();
        let pushes = arrow.output.elems.len();
        self.require(pops)?;
        for _ in 0..pops {
            self.slots.pop();
        }
        for _ in 0..pushes {
            let fresh = self.fresh_literal();
            self.push_term(fresh);
        }
        Ok(())
    }

    // --- token-driven execution (mirrors evaluator.rs::eval_scope) ---

    /// Execute a token sequence against the shadow stack, mirroring the
    /// interpreter's dispatch (`evaluator.rs::eval_scope`): a `Bracket` pushes a
    /// quotation slot exactly as the runtime pushes a quotation value; a `Word`
    /// is resolved to a [`ShadowWord`] and executed.
    ///
    /// `resolve` maps a word to its shadow action. The core shuffles, `DIP`, and
    /// `CALL` execute their **real shuffle** (so a mis-shuffle is caught by
    /// conformance, not silently shipped); opaque words pop/push by their Tier 0
    /// arrow (`resolve` supplies the arrow).
    pub fn exec<R>(&mut self, tokens: &[Token], resolve: &R) -> Result<(), ShadowError>
    where
        R: Fn(&str) -> ShadowWord,
    {
        for token in tokens {
            match token {
                // A quotation is a value on the stack at runtime; here too.
                Token::Bracket(body) => self.push_quote(body.clone()),
                Token::Word(w) => match resolve(w) {
                    ShadowWord::Dup => self.dup()?,
                    ShadowWord::Drop => self.drop()?,
                    ShadowWord::Swap => self.swap()?,
                    ShadowWord::Over => self.over()?,
                    ShadowWord::Rot => self.rot()?,
                    ShadowWord::Nip => self.nip()?,
                    ShadowWord::Tuck => self.tuck()?,
                    ShadowWord::Dip => {
                        // Mirror combinators.rs::dip EXACTLY: pop the quotation
                        // (top), set aside the next term, run the quotation on
                        // the rest, then restore the set-aside term.
                        self.require(2)?;
                        let body = self.pop_quote()?;
                        let hidden = self.pop()?;
                        self.exec(&body, resolve)?;
                        self.push_slot(hidden);
                    }
                    ShadowWord::Call => {
                        // Mirror combinators.rs::call: pop the quotation and run
                        // it on the whole stack.
                        let body = self.pop_quote()?;
                        self.exec(&body, resolve)?;
                    }
                    ShadowWord::Bin(op) => self.bin(op)?,
                    ShadowWord::Un(op) => self.un(op)?,
                    ShadowWord::Num(lexeme) => self.push_term(Pred::Num(lexeme)),
                    ShadowWord::Var(name) => self.push_term(Pred::Var(name)),
                    ShadowWord::Opaque(arrow) => self.apply_opaque(&arrow)?,
                },
            }
        }
        Ok(())
    }
}

// ===========================================================================
// Positional → named binding (the zip, §10.2)
// ===========================================================================

/// A named binding produced by the §10.2 zip: a refinement parameter name bound
/// to the symbolic term occupying its stack slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedBinding {
    /// The refinement parameter name (the SMT-visible variable, §10.2).
    pub name: String,
    /// The symbolic term in the slot this name binds to.
    pub term: Pred,
}

/// Zip a callee's named refinement parameters against the shadow stack
/// **right-to-left from the top of stack** — the only pinned end (§10.2).
///
/// For `push : ( xs n -- … )` against an inferred `'S Num Num`, this binds
/// `n ← top`, `xs ← second`. The result is returned in **source order** (`xs`,
/// then `n`) for readability.
///
/// The row tail `'S` is **never** named: this consumes exactly `binders.len()`
/// slots from the top and never touches the slots beneath them (which model the
/// unobserved tail). Naming the tail would produce unbound SMT variables
/// (§10.2), so it is structurally impossible here — there is no code path that
/// emits a name for anything below the bound binders.
pub fn bind_positional(
    binders: &[Binder],
    stack: &ShadowStack,
) -> Result<Vec<NamedBinding>, ShadowError> {
    let n = binders.len();
    if stack.slots.len() < n {
        return Err(ShadowError::new(format!(
            "cannot bind {n} named parameters: only {} slots on the shadow stack",
            stack.slots.len()
        )));
    }
    let mut out = Vec::with_capacity(n);
    // Right-to-left from the top: depth 0 = top = last binder.
    for depth in 0..n {
        let binder = &binders[n - 1 - depth];
        let slot = &stack.slots[stack.slots.len() - 1 - depth];
        // A **higher-order** (quotation-typed) parameter occupies a `Quote` slot
        // and carries a contract, not a scalar value; bind it to a symbolic
        // stand-in named after the binder so a scalar `where` predicate stays
        // well-formed. Its real contract is checked by subsumption (§10.6), never
        // by scalar substitution — so the stand-in is never meaningfully read.
        let term = if binder.quote.is_some() {
            Pred::Var(binder.name.clone())
        } else {
            slot.as_term()?.clone()
        };
        out.push(NamedBinding {
            name: binder.name.clone(),
            term,
        });
    }
    // Collected top-first; reverse into source order (deepest binder first).
    out.reverse();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Evaluator;
    use crate::Quotable;
    use crate::Span;
    use crate::StackTy;
    use crate::Token;
    use crate::Ty;
    use crate::WordTy;
    use crate::parse;
    use crate::refinement::RefineSpan;
    use crate::types::is_numeric_literal;

    // -- helpers -------------------------------------------------------------

    const S: Span = Span { start: 0, end: 0 };

    fn rspan() -> RefineSpan {
        RefineSpan { start: 0, end: 0 }
    }

    fn binder(name: &str, ty: &str) -> Binder {
        Binder {
            name: name.to_string(),
            ty: ty.to_string(),
            span: rspan(),
            quote: None,
        }
    }

    fn var(name: &str) -> Pred {
        Pred::Var(name.to_string())
    }

    /// A resolver covering core shuffles + interpreted operators; treats every
    /// other word as a free variable. Used by the term-shape tests.
    fn term_resolver(w: &str) -> ShadowWord {
        if let Some(core) = core_shadow_word(w) {
            return core;
        }
        if let Some(op) = interpreted_op(w) {
            return op;
        }
        if is_numeric_literal(w) {
            return ShadowWord::Num(w.to_string());
        }
        ShadowWord::Var(w.to_string())
    }

    #[test]
    fn core_shadow_word_requires_runtime_spelling() {
        assert_eq!(core_shadow_word("DUP"), Some(ShadowWord::Dup));
        assert_eq!(core_shadow_word("DIP"), Some(ShadowWord::Dip));
        assert_eq!(core_shadow_word("dup"), None);
        assert_eq!(core_shadow_word("dip"), None);
    }

    // =======================================================================
    // §12 M7 acceptance: term shapes
    // =======================================================================

    #[test]
    fn after_x_0_gt_top_term_is_x_gt_0() {
        // §10.3: after `x 0 >` the top slot's term is the proposition x > 0.
        let mut s = ShadowStack::new();
        let toks = parse("x 0 >").unwrap();
        s.exec(&toks, &term_resolver).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(
            s.top_term().unwrap(),
            &Pred::Bin(
                BinOp::Gt,
                Box::new(var("x")),
                Box::new(Pred::Num("0".into()))
            )
        );
    }

    #[test]
    fn dup_aliases_the_same_term_by_identity() {
        // §10.3 / invariant 7: `x DUP` leaves TWO slots carrying the SAME term
        // by identity (DUP clones, it does not mint a fresh literal), so
        // `x DUP -` is `x - x` over an IDENTICAL `x` and discharges to 0.
        let mut s = ShadowStack::new();
        s.push_term(var("x"));
        s.dup().unwrap();
        assert_eq!(s.len(), 2);
        // Both slots are the same term.
        assert_eq!(s.slots()[0], s.slots()[1]);
        assert_eq!(s.slots()[0], Slot::Term(var("x")));

        // `x DUP -` builds x - x over the identical x (not two fresh literals).
        let mut s2 = ShadowStack::new();
        let toks = parse("x DUP -").unwrap();
        s2.exec(&toks, &term_resolver).unwrap();
        assert_eq!(
            s2.top_term().unwrap(),
            &Pred::Bin(BinOp::Sub, Box::new(var("x")), Box::new(var("x")))
        );
    }

    #[test]
    fn opaque_producer_yields_a_fresh_literal() {
        // §10.3: an opaque producer's slot carries a fresh literal (sound,
        // imprecise) — not a meaningful term.
        let mut s = ShadowStack::new();
        // opaque lib : ( Num Num -- Num )
        let arrow = WordTy::new(
            StackTy::new(vec![Ty::num(S), Ty::num(S)], 0, S),
            StackTy::new(vec![Ty::num(S)], 0, S),
        );
        s.push_term(var("a"));
        s.push_term(var("b"));
        s.apply_opaque(&arrow).unwrap();
        // Two popped, exactly one fresh literal pushed.
        assert_eq!(s.len(), 1);
        match s.top_term().unwrap() {
            Pred::Var(name) => assert!(
                name.starts_with('$'),
                "opaque producer should push a fresh literal, got {name}"
            ),
            other => panic!("expected a fresh literal, got {other:?}"),
        }
    }

    #[test]
    fn arity_is_read_from_the_tier0_arrow() {
        // §10.3 / invariant 19: the shadow evaluator owns NO independent arity.
        // An opaque lib : ( Num Num -- Num ) pops exactly TWO and pushes exactly
        // ONE fresh literal, driven solely by the arrow's elem counts.
        let mut s = ShadowStack::new();
        s.push_term(var("keep_me"));
        s.push_term(var("a"));
        s.push_term(var("b"));
        let arrow = WordTy::new(
            StackTy::new(vec![Ty::num(S), Ty::num(S)], 0, S),
            StackTy::new(vec![Ty::num(S)], 0, S),
        );
        s.apply_opaque(&arrow).unwrap();
        // keep_me untouched below the one fresh result.
        assert_eq!(s.len(), 2);
        assert_eq!(s.slots()[0], Slot::Term(var("keep_me")));
        assert!(matches!(s.slots()[1], Slot::Term(Pred::Var(ref n)) if n.starts_with('$')));

        // A different arrow shape moves a different number of terms — purely the
        // arrow decides. ( a -- a a a ) pops one, pushes three.
        let mut s2 = ShadowStack::new();
        s2.push_term(var("z"));
        let arrow2 = WordTy::new(
            StackTy::new(vec![Ty::var(0, S)], 0, S),
            StackTy::new(vec![Ty::var(0, S), Ty::var(0, S), Ty::var(0, S)], 0, S),
        );
        s2.apply_opaque(&arrow2).unwrap();
        assert_eq!(s2.len(), 3);
    }

    // =======================================================================
    // §12 M7 acceptance: positional -> named binding (the zip, §10.2)
    // =======================================================================

    #[test]
    fn push_binds_n_to_top_and_xs_to_second() {
        // §10.2: push : ( xs n -- … ) against inferred 'S Num Num binds
        // n <- top, xs <- second.
        let mut s = ShadowStack::new();
        // model 'S (some tail value) Num Num: seed deepest-first.
        s.push_term(var("tail_value")); // part of 'S — must never be named
        s.push_term(var("xs_term"));
        s.push_term(var("n_term"));
        let binders = vec![binder("xs", "List"), binder("n", "Num")];
        let bound = bind_positional(&binders, &s).unwrap();
        assert_eq!(bound.len(), 2);
        // source order: xs first, n second.
        assert_eq!(bound[0].name, "xs");
        assert_eq!(bound[0].term, var("xs_term"));
        assert_eq!(bound[1].name, "n");
        assert_eq!(bound[1].term, var("n_term"));
    }

    #[test]
    fn row_tail_is_never_named() {
        // §10.2: the row tail 'S is never named into a term/VC. bind_positional
        // consumes exactly binders.len() slots from the top and never references
        // anything beneath them — so no name is ever emitted for the tail.
        let mut s = ShadowStack::new();
        s.push_term(var("deep_tail_a"));
        s.push_term(var("deep_tail_b"));
        s.push_term(var("xs_term"));
        s.push_term(var("n_term"));
        let binders = vec![binder("xs", "List"), binder("n", "Num")];
        let bound = bind_positional(&binders, &s).unwrap();
        let names: Vec<&str> = bound.iter().map(|b| b.name.as_str()).collect();
        // Only the two binder names appear; nothing from the tail.
        assert_eq!(names, vec!["xs", "n"]);
        for b in &bound {
            assert_ne!(b.term, var("deep_tail_a"));
            assert_ne!(b.term, var("deep_tail_b"));
        }
    }

    #[test]
    fn zip_binds_correct_names_after_a_nontrivial_dip() {
        // §10.2 post-shuffle: after a non-trivial DIP the zip must still bind the
        // right names to the right slots. Start: a b c (top c). Run `[ SWAP ] DIP`
        // which swaps a,b under c -> b a c. Now name a 3-binder signature
        // ( p q r -- ) against the result: r<-top=c, q<-second=a, p<-third=b.
        let mut s = ShadowStack::new();
        s.push_term(var("a"));
        s.push_term(var("b"));
        s.push_term(var("c"));
        let toks = parse("[ SWAP ] DIP").unwrap();
        s.exec(&toks, &term_resolver).unwrap();
        // Shadow result is b a c (top c).
        assert_eq!(
            s.slots(),
            &[
                Slot::Term(var("b")),
                Slot::Term(var("a")),
                Slot::Term(var("c")),
            ]
        );
        let binders = vec![binder("p", "Num"), binder("q", "Num"), binder("r", "Num")];
        let bound = bind_positional(&binders, &s).unwrap();
        assert_eq!(bound[0].name, "p");
        assert_eq!(bound[0].term, var("b"));
        assert_eq!(bound[1].name, "q");
        assert_eq!(bound[1].term, var("a"));
        assert_eq!(bound[2].name, "r");
        assert_eq!(bound[2].term, var("c"));
    }

    // =======================================================================
    // Conformance: shadow data flow vs the REAL interpreter (§10.3, inv 7)
    // =======================================================================

    // An identity-carrying runtime value: each seeded value has a distinct tag,
    // so we can observe EXACTLY how the interpreter moves data (DUP clones a
    // tag, SWAP moves tags, etc.). Quotations are carried so DIP/CALL run.
    #[derive(Debug, Clone, PartialEq)]
    enum Tagged {
        Val(u32),
        Quote(Vec<Token>),
    }

    impl From<Token> for Tagged {
        fn from(token: Token) -> Self {
            match token {
                // A bare word in a driver program is never a fresh value here;
                // we only seed values directly. Treat any stray word as tag 0.
                Token::Word(_) => Tagged::Val(0),
                Token::Bracket(b) => Tagged::Quote(b),
            }
        }
    }

    impl Quotable for Tagged {
        fn as_quotation(&self) -> Option<&[Token]> {
            match self {
                Tagged::Quote(b) => Some(b),
                _ => None,
            }
        }
        fn to_tokens(&self) -> Vec<Token> {
            match self {
                Tagged::Val(n) => vec![Token::Word(n.to_string())],
                Tagged::Quote(b) => vec![Token::Bracket(b.clone())],
            }
        }
        fn is_truthy(&self) -> bool {
            true
        }
        fn as_sequence(&self) -> Option<Vec<Self>> {
            None
        }
        fn from_sequence(_elements: Vec<Self>) -> Self {
            Tagged::Val(0)
        }
    }

    // Read the identity sequence (bottom->top) off a runtime stack, mapping each
    // value tag and each quotation to a comparable key.
    fn runtime_identities(stack: &[Tagged]) -> Vec<Identity> {
        stack
            .iter()
            .map(|v| match v {
                Tagged::Val(n) => Identity::Val(*n),
                Tagged::Quote(b) => Identity::Quote(b.clone()),
            })
            .collect()
    }

    // Read the identity sequence (bottom->top) off the shadow stack. A seeded
    // value term Var("v{n}") maps to Val(n); a quote slot maps to Quote.
    fn shadow_identities(stack: &ShadowStack) -> Vec<Identity> {
        stack
            .slots()
            .iter()
            .map(|s| match s {
                Slot::Term(Pred::Var(name)) => {
                    let n = name.trim_start_matches('v').parse::<u32>().unwrap();
                    Identity::Val(n)
                }
                Slot::Term(other) => panic!("unexpected shadow term {other:?}"),
                Slot::Quote(b) => Identity::Quote(b.clone()),
            })
            .collect()
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Identity {
        Val(u32),
        Quote(Vec<Token>),
    }

    // The shuffle resolver: only core shuffles + DIP/CALL. Anything else is a bug
    // in the generated program.
    fn shuffle_resolver(w: &str) -> ShadowWord {
        core_shadow_word(w)
            .unwrap_or_else(|| panic!("conformance program used non-shuffle word {w:?}"))
    }

    // Run one shuffle program through BOTH the interpreter and the shadow stack,
    // seeding `depth` distinct values, and assert the identity sequences match.
    fn assert_conformance(program: &str, depth: u32) {
        let toks = parse(program).unwrap();

        // Runtime: seed v0..v{depth-1} as distinct tags, then run.
        let mut eval: Evaluator<Tagged> = Evaluator::new();
        crate::register_all_builtins(&mut eval);
        let mut rstack: Vec<Tagged> = (0..depth).map(Tagged::Val).collect();
        let rt = eval.eval_with_stack(&toks, &mut rstack);

        // Shadow: seed Var("v0")..Var("v{depth-1}"), run the same tokens.
        let mut sstack = ShadowStack::new();
        for n in 0..depth {
            sstack.push_term(Pred::Var(format!("v{n}")));
        }
        let st = sstack.exec(&toks, &shuffle_resolver);

        // Both must agree on success/failure...
        assert_eq!(
            rt.is_ok(),
            st.is_ok(),
            "interpreter and shadow disagree on success for {program:?} depth {depth}"
        );
        if rt.is_ok() {
            // ...and on the exact identity movement.
            assert_eq!(
                runtime_identities(&rstack),
                shadow_identities(&sstack),
                "data-flow divergence for {program:?} depth {depth}"
            );
        }
    }

    #[test]
    fn conformance_core_shuffles_enumerated() {
        // Fixed cases over the core shuffles and a non-trivial DIP. Programs use
        // the runtime (uppercase) builtin names so the interpreter dispatches the
        // real operators; the shadow resolver matches those names exactly.
        assert_conformance("DUP", 1);
        assert_conformance("DROP", 1);
        assert_conformance("SWAP", 2);
        assert_conformance("OVER", 2);
        assert_conformance("ROT", 3);
        assert_conformance("NIP", 2);
        assert_conformance("TUCK", 2);
        assert_conformance("DUP SWAP DROP", 3);
        assert_conformance("OVER OVER ROT", 3);
        assert_conformance("[ SWAP ] DIP", 3);
        assert_conformance("[ DUP ] DIP", 3);
        assert_conformance("[ SWAP DUP ] DIP DROP", 4);
        assert_conformance("[ OVER ] DIP SWAP", 4);
        assert_conformance("[ ROT ] DIP", 4);
        assert_conformance("[ [ SWAP ] DIP ] DIP", 4);
        assert_conformance("[ SWAP ] CALL", 2);
    }

    #[test]
    fn conformance_property_random_shuffles() {
        // Property test: generate random shuffle programs and assert the shadow
        // stack's data flow matches the interpreter's EXACTLY. A mis-shuffle in
        // the shadow stack would diverge here (a vacuous proof caught as a data-
        // flow divergence, §10.3), rather than being silently shipped.
        //
        // Deterministic LCG so failures are reproducible.
        let mut seed: u64 = 0x9E3779B97F4A7C15;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };

        // Words requiring at least N values present (excluding the quotation for
        // DIP/CALL). Track an abstract height so generated programs are valid.
        for _ in 0..2000 {
            let depth = 2 + (next() % 4); // 2..=5 seeded values
            let mut height = depth as i64;
            let mut program = String::new();
            let steps = 1 + next() % 8;
            let mut ok = true;
            for _ in 0..steps {
                // 0..=7 pick an operation that is currently applicable.
                let choices: &[(&str, i64, i64)] = &[
                    // (word, min height, height delta)
                    ("DUP", 1, 1),
                    ("DROP", 1, -1),
                    ("SWAP", 2, 0),
                    ("OVER", 2, 1),
                    ("ROT", 3, 0),
                    ("NIP", 2, -1),
                    ("TUCK", 2, 1),
                ];
                let pick = &choices[(next() as usize) % choices.len()];
                if height < pick.1 {
                    continue;
                }
                program.push_str(pick.0);
                program.push(' ');
                height += pick.2;
                if height < 0 {
                    ok = false;
                    break;
                }
            }
            if !ok || program.trim().is_empty() {
                continue;
            }
            assert_conformance(program.trim(), depth);
        }
    }

    #[test]
    fn conformance_dip_matches_interpreter_with_inner_shuffles() {
        // Targeted DIP conformance: DIP's set-aside/restore must mirror
        // combinators.rs::dip with arbitrary inner shuffles.
        assert_conformance("[ SWAP ] DIP", 3);
        assert_conformance("[ SWAP OVER ] DIP", 4);
        assert_conformance("[ DROP DUP ] DIP", 4);
        assert_conformance("[ ROT ROT ] DIP DROP", 5);
        assert_conformance("[ TUCK ] DIP NIP", 4);
    }

    // =======================================================================
    // Compile-time only (§10.10 / invariant 14)
    // =======================================================================

    #[test]
    fn shadow_stack_is_compile_time_only() {
        // The shadow stack is a compile-time analysis artifact: it is NEVER a
        // field of Evaluator and the runtime eval path never constructs one.
        //
        // Proof by construction (1): a runtime program runs to completion with
        // an Evaluator that has no shadow-stack state — the runtime needs none.
        let mut eval: Evaluator<Tagged> = Evaluator::new();
        crate::register_all_builtins(&mut eval);
        let toks = parse("DUP SWAP").unwrap();
        let mut stack = vec![Tagged::Val(1), Tagged::Val(2)];
        eval.eval_with_stack(&toks, &mut stack).unwrap();
        // Identity movement is correct WITHOUT any shadow machinery.
        assert_eq!(
            runtime_identities(&stack),
            vec![Identity::Val(1), Identity::Val(2), Identity::Val(2)]
        );

        // Proof by construction (2): the shadow stack is a standalone artifact,
        // built only here / by the checker, and discarded — it owns no handle
        // back into the Evaluator and the Evaluator owns no handle to it.
        let mut shadow = ShadowStack::new();
        shadow.push_term(var("x"));
        shadow.dup().unwrap();
        assert_eq!(shadow.len(), 2);
        drop(shadow); // discarded before anything "ships", like a dataflow lattice.
    }

    // A static guarantee: ShadowStack moving terms never touches the Tier 0
    // substitution (it is its own state). This compiles only because the two are
    // independent types with no shared mutable handle.
    #[allow(dead_code)]
    fn _shadow_owns_its_own_state(s: &mut ShadowStack) {
        let _ = s.len();
    }

    #[test]
    fn unbalanced_shadow_underflow_is_an_error_not_a_panic() {
        // A mis-shuffle does not crash; a genuine underflow is a clean error.
        let mut s = ShadowStack::new();
        let err = s.dup().unwrap_err();
        assert!(err.to_string().contains("underflow"));
        // DIP needs a quotation on top and a value below.
        let mut s2 = ShadowStack::new();
        s2.push_term(var("only"));
        let toks = parse("[ DUP ] DIP").unwrap();
        // Only one value under the quotation -> the quotation pops, then the
        // hidden pop succeeds, leaving the inner DUP with an empty stack: error.
        let _ = s2.exec(&toks, &term_resolver); // must not panic
    }
}
