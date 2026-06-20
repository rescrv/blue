//! Tier 1 — refinement signature & predicate parsing (M6, §10.1 / §14.8).
//!
//! This module is **parse-only**. It builds the AST for refinement signatures
//! and the `where` predicate language, and nothing else: there is **no** shadow
//! stack, **no** VC generation, **no** subsumption, and **no** solver here.
//! Those are M7+ (§10.3–§10.6); M6 is gated to the parser and ASTs that every
//! later Tier 1 milestone consumes.
//!
//! # Two parsers, kept separate (§10.1)
//!
//! Caternary's *surface* is a **postfix** concatenative language, shell-tokenized
//! into [`crate::Token`]s by `parser.rs`. The refinement `where` language is a
//! completely different beast: an **ordinary infix predicate language**
//! (arithmetic, comparison, boolean connectives, and uninterpreted function
//! application like `length xs`) targeting SMT. The spec is explicit — "keep the
//! two parsers separate." So this module carries its **own** lexer and recursive
//! descent parser over raw `&str`, and it never touches `parser.rs`'s token
//! stream. The postfix parser never sees a `where` clause; this parser never sees
//! a quotation.
//!
//! # Recorded design — the refinement signature attachment surface
//!
//! `docs/typing.md` is read-only, so the *concrete source surface* by which a
//! refinement signature attaches to a definition is **defined and recorded
//! here**, reconciled against the two existing attachment surfaces:
//!
//!   * `[ body ] :name`  — the runtime definition (postfix tokens; `evaluator.rs`).
//!   * `[ effect ] @name` — the Tier-0 stack-effect annotation (postfix tokens,
//!     the rank-2 case; M4 / §8 / §10.11(b)).
//!
//! A refinement signature **cannot** ride on the postfix token stream the way
//! those two do: its body is infix (`n >= 0 and r * r = n`), and shell
//! tokenization would shred it into meaningless words and lose the operator
//! structure entirely. Forcing it through `parser.rs` would violate the
//! "two parsers separate" mandate. So the refinement signature gets its **own
//! source channel**, written exactly as §10.1 shows it:
//!
//! ```text
//! sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 and r * r = n )
//! push : ( xs: List  n: Num  --  ys: List where length ys = length xs + 1 )
//! ```
//!
//! The head identifier (`sqrt`, `push`) names the **definition** the signature
//! refines; the parenthesized body is the infix signature. The host attaches one
//! by calling [`crate::Evaluator::attach_refinement`] with this raw text; that
//! method parses it here and binds the result to the named definition in a side
//! table (a sibling of the `[ effect ] @name` annotation table). The postfix
//! `load`/`load_with_spans` path never sees this text, and this parser never sees
//! a postfix token — the two parsers stay separate, by construction.
//!
//! Input-side predicates are **demands** (obligations on the caller); output-side
//! predicates are **guarantees** (gifts to the next word) — §10.1.
//!
//! # Forwarded payload, never read by Tier 0 (§3, invariant 10)
//!
//! A parsed [`RefinementSig`] is the §3-reserved optional `pre`/`post` payload on
//! a quotation arrow. It is wired into [`crate::WordTy::refinement`] and
//! **forwarded untouched** — Tier 0 unifies on shape alone and never reads it.
//! See the inertness tests in this module and in `check.rs`.

use std::fmt;

// ===========================================================================
// Spans & errors
// ===========================================================================

/// A span into the **refinement signature source text**, in byte offsets.
///
/// This is deliberately a *separate* span type from [`crate::Span`]: refinement
/// text lives in its own source channel (see the module docs), so its offsets are
/// measured against that text, not the postfix program. A malformed `where`
/// clause produces a [`RefineParseError`] carrying one of these, so the diagnostic
/// is **located**, never a bare unlocated message (§12 M6 acceptance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefineSpan {
    /// Inclusive starting byte offset into the refinement source text.
    pub start: usize,
    /// Exclusive ending byte offset into the refinement source text.
    pub end: usize,
}

impl RefineSpan {
    fn new(start: usize, end: usize) -> Self {
        RefineSpan { start, end }
    }
}

/// A **located** parse error in a refinement signature or `where` predicate.
///
/// Always carries a [`RefineSpan`] (byte offsets into the refinement source), so
/// a malformed `where` yields a located error rather than a panic or an unlocated
/// message — the §12 M6 acceptance bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineParseError {
    /// Human-readable description of what was expected / what went wrong.
    pub message: String,
    /// The location in the refinement source text where the error was detected.
    pub span: RefineSpan,
}

impl RefineParseError {
    fn new(message: impl Into<String>, span: RefineSpan) -> Self {
        RefineParseError {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for RefineParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "refinement parse error (bytes {}..{}): {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for RefineParseError {}

// ===========================================================================
// Predicate AST (the infix `where` language)
// ===========================================================================

/// A binary operator in the `where` predicate language.
///
/// Covers arithmetic, comparison, and boolean connectives — an ordinary infix
/// language (§10.1), distinct from Caternary's postfix surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// Addition `+`.
    Add,
    /// Subtraction `-`.
    Sub,
    /// Multiplication `*`.
    Mul,
    /// Division `/`.
    Div,
    /// Greater-or-equal `>=`.
    Ge,
    /// Less-or-equal `<=`.
    Le,
    /// Strictly greater `>`.
    Gt,
    /// Strictly less `<`.
    Lt,
    /// Equality `=`.
    Eq,
    /// Boolean conjunction `and`.
    And,
    /// Boolean disjunction `or`.
    Or,
    /// Logical implication `=>` (also accepted as `==>`).
    Implies,
}

/// A unary operator in the `where` predicate language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// Boolean negation `not`.
    Not,
    /// Arithmetic negation (unary `-`).
    Neg,
}

/// A predicate expression — the AST of the infix `where` language (§10.1).
///
/// Targets SMT (a later milestone consumes this); for M6 it is the parse result
/// only. Number literals keep their original lexeme so the exact source form is
/// preserved (one numeric type `Num`, §1 — no Int/Float split).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pred {
    /// A reference to a named binder, e.g. `n`, `xs`, `ys`.
    Var(String),
    /// A numeric literal, stored as its source lexeme (one numeric type `Num`).
    Num(String),
    /// A binary application, e.g. `r * r`, `n >= 0`, `a and b`.
    Bin(BinOp, Box<Pred>, Box<Pred>),
    /// A unary application, e.g. `not p`, `- n`.
    Un(UnOp, Box<Pred>),
    /// An **uninterpreted function** application, e.g. `length xs`. The function
    /// symbol is uninterpreted at parse time (its axioms, if any, arrive later).
    App(String, Vec<Pred>),
}

// ===========================================================================
// Signature AST
// ===========================================================================

/// A single named binder `name: Type` in a refinement signature, with its span.
///
/// The type is stored as its source identifier (e.g. `Num`, `List`); Tier 0 owns
/// shape inference, so the refinement layer keeps only the surface name.
///
/// # Higher-order binders (§3-reserved Quote pre/post payload)
///
/// A binder whose type is a **quotation arrow** — `q: ( … -- … )` — carries its
/// own refined pre/post contract in [`Binder::quote`]. This is the §3 "Tier 1
/// payload" that a `Quote` arrow reserves: a refined quotation parameter
/// declaring the contract it is *expected* to satisfy when it crosses a
/// higher-order boundary (§10.6). For such a binder, [`Binder::ty`] is the
/// surface marker `"Quote"` and [`Binder::quote`] is `Some`; for an ordinary
/// scalar binder, [`Binder::quote`] is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    /// The binder's name (the SMT-visible variable, §10.2).
    pub name: String,
    /// The binder's surface type name, e.g. `Num` or `List`. For a higher-order
    /// (quotation-typed) binder this is the marker `"Quote"`; the actual refined
    /// arrow lives in [`Binder::quote`].
    pub ty: String,
    /// The location of this binder in the refinement source text.
    pub span: RefineSpan,
    /// The refined quotation contract this binder declares, when it is a
    /// **higher-order** (quotation-typed) parameter `q: ( pre -- post )` — the
    /// §3-reserved Quote pre/post payload. `None` for a scalar binder. This is
    /// the *expected* contract checked by subsumption (§10.6) when a quotation
    /// value crosses into this parameter position.
    pub quote: Option<Box<QuoteContract>>,
}

/// The refined arrow a higher-order (quotation-typed) binder declares (§3 /
/// §10.6): the pre/post contract the quotation parameter is expected to satisfy.
///
/// Structurally this is a [`RefinementSig`] without a `name` — two
/// [`RefinementSide`]s, demands (the quotation's expected precondition) and
/// guarantees (its expected postcondition). It is the *expected* side of a
/// higher-order subsumption check (§10.6); the *provided* side is the contract
/// of the quotation value actually passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteContract {
    /// Input side: the quotation's expected demand predicates (pre).
    pub demands: RefinementSide,
    /// Output side: the quotation's expected guarantee predicates (post).
    pub guarantees: RefinementSide,
}

impl QuoteContract {
    /// View this contract as a nameless [`RefinementSig`] so it can be handed to
    /// the §10.6 subsumption checker, which speaks `RefinementSig`. The synthetic
    /// `name` is never read by subsumption (it aligns binders positionally).
    pub fn as_sig(&self) -> RefinementSig {
        RefinementSig {
            name: "<quote>".to_string(),
            demands: self.demands.clone(),
            guarantees: self.guarantees.clone(),
        }
    }
}

/// One side of a refinement signature: the named binders plus an optional
/// `where` predicate over them.
///
/// On the **input** side the predicate is a **demand** (an obligation on the
/// caller); on the **output** side it is a **guarantee** (a gift to the next
/// word) — §10.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementSide {
    /// The named binders, in source order (left-to-right).
    pub binders: Vec<Binder>,
    /// The optional `where` predicate over the binders. `None` means an absent
    /// refinement, which §10.7 reads as `where true` (M6 only records absence).
    pub predicate: Option<Pred>,
}

/// A full refinement signature attached to a definition (§10.1).
///
/// `name` is the definition this signature refines; `demands` is the input side
/// (pre / obligations), `guarantees` is the output side (post / gifts). This is
/// the §3-reserved `pre`/`post` payload — forwarded to [`crate::WordTy`] and
/// never read by Tier 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementSig {
    /// The name of the definition this signature refines.
    pub name: String,
    /// Input side: demand predicates (pre) over the input binders.
    pub demands: RefinementSide,
    /// Output side: guarantee predicates (post) over the output binders.
    pub guarantees: RefinementSide,
}

// ===========================================================================
// Lexer
// ===========================================================================

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Num(String),
    Colon,
    Arrow, // --
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
    Implies,
    // keywords
    Where,
    And,
    Or,
    Not,
}

#[derive(Debug, Clone)]
struct Spanned {
    tok: Tok,
    span: RefineSpan,
}

fn lex(src: &str) -> Result<Vec<Spanned>, RefineParseError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        match c {
            b'(' => {
                out.push(Spanned {
                    tok: Tok::LParen,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b')' => {
                out.push(Spanned {
                    tok: Tok::RParen,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b':' => {
                out.push(Spanned {
                    tok: Tok::Colon,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b'+' => {
                out.push(Spanned {
                    tok: Tok::Plus,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b'*' => {
                out.push(Spanned {
                    tok: Tok::Star,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b'/' => {
                out.push(Spanned {
                    tok: Tok::Slash,
                    span: RefineSpan::new(start, i + 1),
                });
                i += 1;
            }
            b'-' => {
                // `--` is the input/output arrow; a single `-` is subtraction or
                // unary minus.
                if bytes.get(i + 1) == Some(&b'-') {
                    out.push(Spanned {
                        tok: Tok::Arrow,
                        span: RefineSpan::new(start, i + 2),
                    });
                    i += 2;
                } else {
                    out.push(Spanned {
                        tok: Tok::Minus,
                        span: RefineSpan::new(start, i + 1),
                    });
                    i += 1;
                }
            }
            b'>' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Spanned {
                        tok: Tok::Ge,
                        span: RefineSpan::new(start, i + 2),
                    });
                    i += 2;
                } else {
                    out.push(Spanned {
                        tok: Tok::Gt,
                        span: RefineSpan::new(start, i + 1),
                    });
                    i += 1;
                }
            }
            b'<' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Spanned {
                        tok: Tok::Le,
                        span: RefineSpan::new(start, i + 2),
                    });
                    i += 2;
                } else {
                    out.push(Spanned {
                        tok: Tok::Lt,
                        span: RefineSpan::new(start, i + 1),
                    });
                    i += 1;
                }
            }
            b'=' => {
                // `=>` / `==>` is implication; a lone `=` is equality.
                if bytes.get(i + 1) == Some(&b'>') {
                    out.push(Spanned {
                        tok: Tok::Implies,
                        span: RefineSpan::new(start, i + 2),
                    });
                    i += 2;
                } else if bytes.get(i + 1) == Some(&b'=') && bytes.get(i + 2) == Some(&b'>') {
                    out.push(Spanned {
                        tok: Tok::Implies,
                        span: RefineSpan::new(start, i + 3),
                    });
                    i += 3;
                } else {
                    out.push(Spanned {
                        tok: Tok::Eq,
                        span: RefineSpan::new(start, i + 1),
                    });
                    i += 1;
                }
            }
            _ if c.is_ascii_digit() => {
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                    j += 1;
                }
                let lexeme = src[i..j].to_string();
                // Reject a malformed numeric literal (e.g. `1.2.3`) so it is a
                // located error rather than surviving as a bad atom.
                if lexeme.matches('.').count() > 1 {
                    return Err(RefineParseError::new(
                        format!("malformed numeric literal `{lexeme}`"),
                        RefineSpan::new(i, j),
                    ));
                }
                out.push(Spanned {
                    tok: Tok::Num(lexeme),
                    span: RefineSpan::new(i, j),
                });
                i = j;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                let word = &src[i..j];
                let tok = match word {
                    "where" => Tok::Where,
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Ident(word.to_string()),
                };
                out.push(Spanned {
                    tok,
                    span: RefineSpan::new(i, j),
                });
                i = j;
            }
            _ => {
                return Err(RefineParseError::new(
                    format!("unexpected character `{}`", c as char),
                    RefineSpan::new(i, i + 1),
                ));
            }
        }
    }
    Ok(out)
}

// ===========================================================================
// Parser
// ===========================================================================

struct Parser<'a> {
    toks: &'a [Spanned],
    pos: usize,
    /// One past the last byte of the source, for end-of-input error spans.
    end: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Spanned], end: usize) -> Self {
        Parser { toks, pos: 0, end }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|s| &s.tok)
    }

    fn peek_span(&self) -> RefineSpan {
        match self.toks.get(self.pos) {
            Some(s) => s.span,
            None => RefineSpan::new(self.end, self.end),
        }
    }

    fn bump(&mut self) -> Option<&Spanned> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, want: &Tok, what: &str) -> Result<RefineSpan, RefineParseError> {
        match self.toks.get(self.pos) {
            Some(s) if &s.tok == want => {
                let span = s.span;
                self.pos += 1;
                Ok(span)
            }
            Some(s) => Err(RefineParseError::new(format!("expected {what}"), s.span)),
            None => Err(RefineParseError::new(
                format!("expected {what}, found end of input"),
                RefineSpan::new(self.end, self.end),
            )),
        }
    }

    // ---- predicate grammar (lowest → highest precedence) ----
    //
    //   implication := disjunction ('=>' disjunction)*   (right-assoc)
    //   disjunction := conjunction ('or' conjunction)*
    //   conjunction := unary_bool ('and' unary_bool)*
    //   unary_bool  := 'not' unary_bool | comparison
    //   comparison  := additive (CMP additive)?
    //   additive    := multiplicative (('+'|'-') multiplicative)*
    //   multiplicative := unary_arith (('*'|'/') unary_arith)*
    //   unary_arith := '-' unary_arith | application
    //   application := atom atom*           (juxtaposition: `length xs`)
    //   atom        := IDENT | NUM | '(' implication ')'

    fn parse_pred(&mut self) -> Result<Pred, RefineParseError> {
        self.parse_implication()
    }

    fn parse_implication(&mut self) -> Result<Pred, RefineParseError> {
        let lhs = self.parse_disjunction()?;
        if matches!(self.peek(), Some(Tok::Implies)) {
            self.bump();
            // right-associative
            let rhs = self.parse_implication()?;
            Ok(Pred::Bin(BinOp::Implies, Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_disjunction(&mut self) -> Result<Pred, RefineParseError> {
        let mut lhs = self.parse_conjunction()?;
        while matches!(self.peek(), Some(Tok::Or)) {
            self.bump();
            let rhs = self.parse_conjunction()?;
            lhs = Pred::Bin(BinOp::Or, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_conjunction(&mut self) -> Result<Pred, RefineParseError> {
        let mut lhs = self.parse_unary_bool()?;
        while matches!(self.peek(), Some(Tok::And)) {
            self.bump();
            let rhs = self.parse_unary_bool()?;
            lhs = Pred::Bin(BinOp::And, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary_bool(&mut self) -> Result<Pred, RefineParseError> {
        if matches!(self.peek(), Some(Tok::Not)) {
            self.bump();
            let inner = self.parse_unary_bool()?;
            Ok(Pred::Un(UnOp::Not, Box::new(inner)))
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> Result<Pred, RefineParseError> {
        let lhs = self.parse_additive()?;
        let op = match self.peek() {
            Some(Tok::Ge) => Some(BinOp::Ge),
            Some(Tok::Le) => Some(BinOp::Le),
            Some(Tok::Gt) => Some(BinOp::Gt),
            Some(Tok::Lt) => Some(BinOp::Lt),
            Some(Tok::Eq) => Some(BinOp::Eq),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let rhs = self.parse_additive()?;
            Ok(Pred::Bin(op, Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_additive(&mut self) -> Result<Pred, RefineParseError> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_multiplicative()?;
            lhs = Pred::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Pred, RefineParseError> {
        let mut lhs = self.parse_unary_arith()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary_arith()?;
            lhs = Pred::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary_arith(&mut self) -> Result<Pred, RefineParseError> {
        if matches!(self.peek(), Some(Tok::Minus)) {
            self.bump();
            let inner = self.parse_unary_arith()?;
            Ok(Pred::Un(UnOp::Neg, Box::new(inner)))
        } else {
            self.parse_application()
        }
    }

    fn parse_application(&mut self) -> Result<Pred, RefineParseError> {
        let head = self.parse_atom()?;
        // Uninterpreted function application is juxtaposition: `length xs`. Only
        // an identifier head can be applied; collect following atoms as args.
        if let Pred::Var(name) = &head {
            let mut args = Vec::new();
            while self.starts_atom() {
                args.push(self.parse_atom()?);
            }
            if args.is_empty() {
                Ok(head)
            } else {
                Ok(Pred::App(name.clone(), args))
            }
        } else {
            Ok(head)
        }
    }

    /// True if the next token can begin an atom (so juxtaposition continues).
    fn starts_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(Tok::Ident(_)) | Some(Tok::Num(_)) | Some(Tok::LParen)
        )
    }

    fn parse_atom(&mut self) -> Result<Pred, RefineParseError> {
        match self.peek() {
            Some(Tok::Ident(_)) => {
                let Some(Spanned {
                    tok: Tok::Ident(name),
                    ..
                }) = self.bump()
                else {
                    unreachable!("peeked Ident")
                };
                Ok(Pred::Var(name.clone()))
            }
            Some(Tok::Num(_)) => {
                let Some(Spanned {
                    tok: Tok::Num(lexeme),
                    ..
                }) = self.bump()
                else {
                    unreachable!("peeked Num")
                };
                Ok(Pred::Num(lexeme.clone()))
            }
            Some(Tok::LParen) => {
                self.bump();
                let inner = self.parse_implication()?;
                self.expect(&Tok::RParen, "`)` to close a parenthesized predicate")?;
                Ok(inner)
            }
            Some(_) => Err(RefineParseError::new(
                "expected a predicate atom (variable, number, or `(`)",
                self.peek_span(),
            )),
            None => Err(RefineParseError::new(
                "expected a predicate atom, found end of input",
                RefineSpan::new(self.end, self.end),
            )),
        }
    }

    // ---- signature grammar ----
    //
    //   sig   := IDENT ':' '(' side '--' side ')'
    //   side  := binder* ('where' pred)?
    //   binder:= IDENT ':' IDENT

    fn parse_signature(&mut self) -> Result<RefinementSig, RefineParseError> {
        // Head identifier: the definition name.
        let name = match self.peek() {
            Some(Tok::Ident(_)) => {
                let Some(Spanned {
                    tok: Tok::Ident(n), ..
                }) = self.bump()
                else {
                    unreachable!("peeked Ident")
                };
                n.clone()
            }
            _ => {
                return Err(RefineParseError::new(
                    "expected the definition name a refinement signature attaches to",
                    self.peek_span(),
                ));
            }
        };
        self.expect(&Tok::Colon, "`:` after the definition name")?;
        self.expect(&Tok::LParen, "`(` to open the refinement signature")?;
        let demands = self.parse_side(SideEnd::Arrow)?;
        self.expect(&Tok::Arrow, "`--` separating inputs from outputs")?;
        let guarantees = self.parse_side(SideEnd::RParen)?;
        self.expect(&Tok::RParen, "`)` to close the refinement signature")?;
        if self.pos != self.toks.len() {
            return Err(RefineParseError::new(
                "trailing tokens after the refinement signature",
                self.peek_span(),
            ));
        }
        Ok(RefinementSig {
            name,
            demands,
            guarantees,
        })
    }

    fn parse_side(&mut self, end: SideEnd) -> Result<RefinementSide, RefineParseError> {
        let mut binders = Vec::new();
        // Binders run until `where`, the side terminator, or end of input.
        while matches!(self.peek(), Some(Tok::Ident(_))) {
            binders.push(self.parse_binder()?);
        }
        let predicate = if matches!(self.peek(), Some(Tok::Where)) {
            self.bump();
            Some(self.parse_pred()?)
        } else {
            None
        };
        // Sanity: the side must now sit on its terminator.
        let on_terminator = match end {
            SideEnd::Arrow => matches!(self.peek(), Some(Tok::Arrow)),
            SideEnd::RParen => matches!(self.peek(), Some(Tok::RParen)),
        };
        if !on_terminator {
            let what = match end {
                SideEnd::Arrow => "`--` or `where` after the input binders",
                SideEnd::RParen => "`)` or `where` after the output binders",
            };
            return Err(RefineParseError::new(
                format!("expected {what}"),
                self.peek_span(),
            ));
        }
        Ok(RefinementSide { binders, predicate })
    }

    fn parse_binder(&mut self) -> Result<Binder, RefineParseError> {
        let (name, name_span) = match self.bump() {
            Some(Spanned {
                tok: Tok::Ident(n),
                span,
            }) => (n.clone(), *span),
            other => {
                let span = other
                    .map(|s| s.span)
                    .unwrap_or(RefineSpan::new(self.end, self.end));
                return Err(RefineParseError::new("expected a binder name", span));
            }
        };
        self.expect(&Tok::Colon, "`:` between a binder name and its type")?;
        // A binder's type is either a scalar type identifier (`Num`, `List`) or a
        // **higher-order** refined quotation arrow `( pre -- post )` — the §3
        // Quote pre/post payload (§10.6). The opening `(` distinguishes them.
        if matches!(self.peek(), Some(Tok::LParen)) {
            self.expect(&Tok::LParen, "`(` to open a quotation contract")?;
            let demands = self.parse_side(SideEnd::Arrow)?;
            self.expect(
                &Tok::Arrow,
                "`--` separating a quotation contract's inputs from outputs",
            )?;
            let guarantees = self.parse_side(SideEnd::RParen)?;
            let close = self.expect(&Tok::RParen, "`)` to close a quotation contract")?;
            return Ok(Binder {
                name,
                ty: "Quote".to_string(),
                span: RefineSpan::new(name_span.start, close.end),
                quote: Some(Box::new(QuoteContract {
                    demands,
                    guarantees,
                })),
            });
        }
        let (ty, ty_span) = match self.bump() {
            Some(Spanned {
                tok: Tok::Ident(t),
                span,
            }) => (t.clone(), *span),
            other => {
                let span = other
                    .map(|s| s.span)
                    .unwrap_or(RefineSpan::new(self.end, self.end));
                return Err(RefineParseError::new(
                    "expected a binder type (a type name or a `( … -- … )` quotation contract)",
                    span,
                ));
            }
        };
        Ok(Binder {
            name,
            ty,
            span: RefineSpan::new(name_span.start, ty_span.end),
            quote: None,
        })
    }
}

enum SideEnd {
    Arrow,
    RParen,
}

// ===========================================================================
// Public entry points
// ===========================================================================

/// Parse a standalone `where` predicate (the infix language) from raw text.
///
/// This is the §10.1 predicate language only — arithmetic, comparison, boolean
/// connectives, and uninterpreted function application. A malformed predicate
/// returns a **located** [`RefineParseError`]. Kept public so the predicate
/// parser is independently exercisable (and reusable by later milestones).
pub fn parse_predicate(src: &str) -> Result<Pred, RefineParseError> {
    let toks = lex(src)?;
    let mut p = Parser::new(&toks, src.len());
    let pred = p.parse_pred()?;
    if p.pos != p.toks.len() {
        return Err(RefineParseError::new(
            "trailing tokens after the predicate",
            p.peek_span(),
        ));
    }
    Ok(pred)
}

/// Parse a full refinement signature `name : ( … -- … )` from raw text (§10.1).
///
/// The result's [`RefinementSig::demands`] holds the input-side **demand**
/// predicates and [`RefinementSig::guarantees`] the output-side **guarantee**
/// predicates. A malformed signature (including a malformed `where`) returns a
/// **located** [`RefineParseError`].
pub fn parse_signature(src: &str) -> Result<RefinementSig, RefineParseError> {
    let toks = lex(src)?;
    let mut p = Parser::new(&toks, src.len());
    p.parse_signature()
}

// ===========================================================================
// The `assume` surface (§10.7 / M12)
// ===========================================================================
//
// **Where the user writes it.** `assume` is a **word in the program token
// stream**, written as `assume( PRED )` where PRED is the §10.1 `where`-language
// predicate. It is *near-vestigial* (§10.7): the foreign frontier is the
// operator table (embedder-attested contracts), so the user reaches for
// `assume` only for the rare local assertion about a *Caternary* construct the
// solver cannot reach.
//
// Tokenization note (reconciled with the real shell tokenizer): the postfix
// tokenizer (`parser::parse`) splits on whitespace, so a *spaced* predicate
// (`assume(result > 0)`) must be written quoted — `"assume(result > 0)"` — to
// arrive as one word, while a tight predicate (`assume(result>0)`) needs no
// quotes. Both reach this parser as the single string `assume( … )`; this is the
// recorded assume source surface (the spec text in typing.md is read-only, so the
// concrete surface convention lives here in code).

/// The `assume` surface prefix. A program word is an `assume` clause iff it
/// starts with this and ends with `)` (§10.7 / M12).
pub const ASSUME_PREFIX: &str = "assume(";

/// Recognize and parse an `assume( PRED )` surface word (§10.7 / M12).
///
/// Returns `None` if `word` is not an `assume` clause at all (an ordinary
/// program word). Returns `Some(Ok(pred))` for a well-formed clause and
/// `Some(Err(_))` for a malformed one (an `assume(` opener whose body is not a
/// valid §10.1 predicate, or which is missing its closing `)`), so a malformed
/// `assume` is a **located** error rather than being silently treated as an
/// ordinary word.
pub fn parse_assume(word: &str) -> Option<Result<Pred, RefineParseError>> {
    let rest = word.strip_prefix(ASSUME_PREFIX)?;
    let Some(inner) = rest.strip_suffix(')') else {
        return Some(Err(RefineParseError::new(
            "`assume(` is missing its closing `)`",
            RefineSpan {
                start: word.len(),
                end: word.len(),
            },
        )));
    };
    Some(parse_predicate(inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(name: &str) -> Pred {
        Pred::Var(name.to_string())
    }
    fn n(lex: &str) -> Pred {
        Pred::Num(lex.to_string())
    }
    fn bin(op: BinOp, a: Pred, b: Pred) -> Pred {
        Pred::Bin(op, Box::new(a), Box::new(b))
    }

    // --- §12 M6: sqrt parses into the expected demand/guarantee predicates ---
    #[test]
    fn sqrt_parses_demand_and_guarantee() {
        let sig = parse_signature(
            "sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 and r * r = n )",
        )
        .expect("sqrt signature parses");

        assert_eq!(sig.name, "sqrt");

        // Input side: one binder `n: Num`, demand `n >= 0`.
        assert_eq!(sig.demands.binders.len(), 1);
        assert_eq!(sig.demands.binders[0].name, "n");
        assert_eq!(sig.demands.binders[0].ty, "Num");
        assert_eq!(sig.demands.predicate, Some(bin(BinOp::Ge, v("n"), n("0"))));

        // Output side: one binder `r: Num`, guarantee `r >= 0 and r * r = n`.
        assert_eq!(sig.guarantees.binders.len(), 1);
        assert_eq!(sig.guarantees.binders[0].name, "r");
        assert_eq!(sig.guarantees.binders[0].ty, "Num");
        let expected_guarantee = bin(
            BinOp::And,
            bin(BinOp::Ge, v("r"), n("0")),
            bin(BinOp::Eq, bin(BinOp::Mul, v("r"), v("r")), v("n")),
        );
        assert_eq!(sig.guarantees.predicate, Some(expected_guarantee));
    }

    // --- §12 M6: push parses; uninterpreted `length`, no input `where` ---
    #[test]
    fn push_parses_with_uninterpreted_length() {
        let sig = parse_signature(
            "push : ( xs: List  n: Num  --  ys: List where length ys = length xs + 1 )",
        )
        .expect("push signature parses");

        assert_eq!(sig.name, "push");

        // Input side: two binders, no demand predicate.
        assert_eq!(sig.demands.binders.len(), 2);
        assert_eq!(sig.demands.binders[0].name, "xs");
        assert_eq!(sig.demands.binders[0].ty, "List");
        assert_eq!(sig.demands.binders[1].name, "n");
        assert_eq!(sig.demands.binders[1].ty, "Num");
        assert_eq!(sig.demands.predicate, None);

        // Output side: one binder `ys: List`, guarantee
        // `length ys = length xs + 1` with `length` an uninterpreted function.
        assert_eq!(sig.guarantees.binders.len(), 1);
        assert_eq!(sig.guarantees.binders[0].name, "ys");
        let length_ys = Pred::App("length".to_string(), vec![v("ys")]);
        let length_xs = Pred::App("length".to_string(), vec![v("xs")]);
        let expected = bin(BinOp::Eq, length_ys, bin(BinOp::Add, length_xs, n("1")));
        assert_eq!(sig.guarantees.predicate, Some(expected));
    }

    // --- §12 M6: a malformed `where` produces a LOCATED parse error ---
    #[test]
    fn malformed_where_is_located_error() {
        // Dangling operator: `n >=` with no right operand.
        let src = "sqrt : ( n: Num where n >=  --  r: Num )";
        let err = parse_signature(src).expect_err("malformed where must error");
        // The error is located (carries a span into the source text)…
        assert!(err.span.start <= src.len());
        assert!(err.span.end <= src.len());
        // …and it points at or after the offending `>=` (byte offset of `--`,
        // where the missing right operand is detected).
        let arrow = src.find("--").unwrap();
        assert!(
            err.span.start >= src.find(">=").unwrap(),
            "error span should be at/after the dangling operator, got {:?}",
            err.span
        );
        assert!(err.span.start <= arrow + 2);
    }

    #[test]
    fn malformed_where_unbalanced_paren_is_located() {
        let src = "f : ( a: Num where ( a > 0  --  b: Num )";
        let err = parse_signature(src).expect_err("unbalanced paren must error");
        assert!(err.span.start <= src.len() && err.span.end <= src.len());
    }

    #[test]
    fn predicate_precedence_arith_over_comparison() {
        // `length xs + 1` must group as `(length xs) + 1`, then `= ` binds the
        // whole additive expression (comparison is lower precedence than +).
        let p = parse_predicate("length ys = length xs + 1").unwrap();
        let expected = bin(
            BinOp::Eq,
            Pred::App("length".to_string(), vec![v("ys")]),
            bin(
                BinOp::Add,
                Pred::App("length".to_string(), vec![v("xs")]),
                n("1"),
            ),
        );
        assert_eq!(p, expected);
    }

    #[test]
    fn predicate_and_binds_looser_than_comparison() {
        // `r >= 0 and r * r = n` → And( r>=0 , (r*r)=n )
        let p = parse_predicate("r >= 0 and r * r = n").unwrap();
        let expected = bin(
            BinOp::And,
            bin(BinOp::Ge, v("r"), n("0")),
            bin(BinOp::Eq, bin(BinOp::Mul, v("r"), v("r")), v("n")),
        );
        assert_eq!(p, expected);
    }

    #[test]
    fn predicate_not_and_implication() {
        // `not a and b => c` → Implies( And(Not a, b), c )
        let p = parse_predicate("not a and b => c").unwrap();
        let expected = bin(
            BinOp::Implies,
            bin(BinOp::And, Pred::Un(UnOp::Not, Box::new(v("a"))), v("b")),
            v("c"),
        );
        assert_eq!(p, expected);
    }

    #[test]
    fn predicate_unary_minus() {
        let p = parse_predicate("- n < 0").unwrap();
        let expected = bin(BinOp::Lt, Pred::Un(UnOp::Neg, Box::new(v("n"))), n("0"));
        assert_eq!(p, expected);
    }

    #[test]
    fn empty_predicate_is_located_error() {
        let err = parse_predicate("").expect_err("empty predicate errors");
        assert_eq!(err.span, RefineSpan::new(0, 0));
    }

    #[test]
    fn missing_arrow_is_located_error() {
        let src = "f : ( a: Num )";
        let err = parse_signature(src).expect_err("missing -- errors");
        // The `)` appears where `--` (or more binders/where) was expected.
        assert!(err.span.start <= src.len());
    }

    #[test]
    fn unexpected_char_is_located_error() {
        let err = parse_predicate("a & b").expect_err("`&` is not a token");
        assert_eq!(err.span, RefineSpan::new(2, 3));
    }

    // --- §10.6 / §3: a higher-order (quotation-typed) binder carrying its own
    // refined pre/post arrow parses into a QuoteContract ---------------------
    #[test]
    fn higher_order_binder_parses_into_a_quote_contract() {
        // `apply` takes a refined quotation `q: ( -- r: Num where r > 0 )` and
        // returns a refined `s: Num where s > 0`. The quotation binder must carry
        // its declared pre/post contract (§3 Quote payload), not a scalar type.
        let sig =
            parse_signature("apply : ( q: ( -- r: Num where r > 0 )  --  s: Num where s > 0 )")
                .expect("higher-order signature parses");

        assert_eq!(sig.name, "apply");

        // Input side: one binder `q`, which is a quotation contract (not scalar).
        assert_eq!(sig.demands.binders.len(), 1);
        let q = &sig.demands.binders[0];
        assert_eq!(q.name, "q");
        assert_eq!(
            q.ty, "Quote",
            "a higher-order binder carries the Quote marker"
        );
        let contract = q
            .quote
            .as_ref()
            .expect("a quotation-typed binder carries its declared contract");
        // The quotation's declared post is `r > 0` over `r: Num`; its pre is absent.
        assert!(contract.demands.binders.is_empty());
        assert_eq!(contract.demands.predicate, None);
        assert_eq!(contract.guarantees.binders.len(), 1);
        assert_eq!(contract.guarantees.binders[0].name, "r");
        assert_eq!(
            contract.guarantees.predicate,
            Some(bin(BinOp::Gt, v("r"), n("0")))
        );

        // The outer output side is an ordinary scalar binder.
        assert_eq!(sig.guarantees.binders.len(), 1);
        assert_eq!(sig.guarantees.binders[0].name, "s");
        assert!(sig.guarantees.binders[0].quote.is_none());
        assert_eq!(
            sig.guarantees.predicate,
            Some(bin(BinOp::Gt, v("s"), n("0")))
        );
    }

    #[test]
    fn higher_order_binder_with_pre_and_post_parses() {
        // A fully refined quotation arrow: pre `n >= 0`, post `r >= 0`.
        let sig = parse_signature(
            "g : ( q: ( n: Num where n >= 0 -- r: Num where r >= 0 )  --  s: Num )",
        )
        .expect("higher-order signature with pre and post parses");
        let q = &sig.demands.binders[0];
        let c = q.quote.as_ref().unwrap();
        assert_eq!(c.demands.binders.len(), 1);
        assert_eq!(c.demands.binders[0].name, "n");
        assert_eq!(c.demands.predicate, Some(bin(BinOp::Ge, v("n"), n("0"))));
        assert_eq!(c.guarantees.binders.len(), 1);
        assert_eq!(c.guarantees.binders[0].name, "r");
        assert_eq!(c.guarantees.predicate, Some(bin(BinOp::Ge, v("r"), n("0"))));
    }

    // --- negative: a malformed higher-order signature is a LOCATED error -----
    #[test]
    fn malformed_higher_order_signature_missing_inner_arrow_is_located() {
        // The quotation contract `( r: Num where r > 0 )` is missing its inner
        // `--`: a quotation arrow must separate inputs from outputs. The error is
        // located (a byte span into the source), never a panic.
        let src = "apply : ( q: ( r: Num where r > 0 )  --  s: Num )";
        let err = parse_signature(src).expect_err("a quotation contract without `--` must error");
        assert!(err.span.start <= src.len() && err.span.end <= src.len());
        // It is detected at/after the inner `(` (where the contract opens).
        let inner_open = src.find("( r:").unwrap();
        assert!(
            err.span.start >= inner_open,
            "error should be inside the quotation contract, got {:?}",
            err.span
        );
    }

    #[test]
    fn malformed_higher_order_signature_unclosed_quote_is_located() {
        // The quotation contract never closes its `(`.
        let src = "apply : ( q: ( -- r: Num where r > 0  --  s: Num )";
        let err = parse_signature(src).expect_err("an unclosed quotation contract must error");
        assert!(err.span.start <= src.len() && err.span.end <= src.len());
    }
}
