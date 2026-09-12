//! Standard stack-manipulation builtins.

// Operators must match the `Operator<T>` type signature which requires `&mut Vec<T>`.
#![allow(clippy::ptr_arg)]

use crate::EvalError;
use crate::Evaluator;
use crate::Quotable;
use crate::Scheme;
use crate::Span;
use crate::StackTy;
use crate::Token;
use crate::Ty;
use crate::WordTy;
use crate::evaluator::operator_error;

fn stack_underflow(expected: usize, found: usize) -> EvalError {
    operator_error(format!(
        "stack underflow: need at least {expected} values, found {found}"
    ))
}

fn require_len<T>(stack: &[T], expected: usize) -> Result<(), EvalError> {
    if stack.len() < expected {
        return Err(stack_underflow(expected, stack.len()));
    }
    Ok(())
}

fn dup<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let top = stack.last().unwrap().clone();
    stack.push(top);
    Ok(())
}

fn drop<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    stack.pop();
    Ok(())
}

fn swap<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    stack.swap(len - 2, len - 1);
    Ok(())
}

fn over<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let second = stack[len - 2].clone();
    stack.push(second);
    Ok(())
}

fn rot<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let len = stack.len();
    stack[len - 3..].rotate_left(1);
    Ok(())
}

fn minus_rot<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 3)?;
    let len = stack.len();
    stack[len - 3..].rotate_right(1);
    Ok(())
}

fn nip<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    stack.remove(len - 2);
    Ok(())
}

fn tuck<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let top = stack[len - 1].clone();
    stack.insert(len - 2, top);
    Ok(())
}

fn two_dup<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let len = stack.len();
    let a = stack[len - 2].clone();
    let b = stack[len - 1].clone();
    stack.push(a);
    stack.push(b);
    Ok(())
}

fn two_drop<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    stack.pop();
    stack.pop();
    Ok(())
}

fn two_swap<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let len = stack.len();
    stack[len - 4..].rotate_left(2);
    Ok(())
}

fn two_over<T: Clone>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 4)?;
    let len = stack.len();
    let a = stack[len - 4].clone();
    let b = stack[len - 3].clone();
    stack.push(a);
    stack.push(b);
    Ok(())
}

fn two_rot<T>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 6)?;
    let len = stack.len();
    stack[len - 6..].rotate_left(2);
    Ok(())
}

fn scalar_word<T: Quotable>(value: &T) -> Result<String, EvalError> {
    match value.to_tokens().as_slice() {
        [Token::Word(w)] => Ok(w.clone()),
        [Token::Bracket(_)] => Err(operator_error("expected a scalar word, found quotation")),
        _ => Err(operator_error(
            "expected a scalar value that renders as one word",
        )),
    }
}

/// The runtime `Num` scalar — the **deliberate numeric semantics** (recorded
/// here; `docs/typing.md` is read-only).
///
/// The solver models `Num` as exact `Real` (QF_LRA), so the runtime computes
/// **exactly wherever it can**: an integer-lexeme operand is an `i128` and
/// integer arithmetic is checked (overflow is a runtime error, never a silent
/// wrap or rounding — `9007199254740993 1 +` is `9007199254740994`, not
/// `…992`). An integer lexeme **beyond** the `i128` range is likewise a loud
/// runtime error ([`literal_overflow`]), never a rounded `f64`: the reasoner
/// models the lexeme exactly, so rounding it at runtime would let two distinct
/// integers compare equal (BUGS §A4). Fractional lexemes fall back to `f64` as
/// the **documented escape hatch**: `f64` arithmetic rounds, so a proven
/// refinement over fractional values holds only up to `f64` rounding.
/// Non-finite lexemes (`inf`, `NaN`) have no `Real` semantics and are **not**
/// numbers — the Tier-0 literal grammar
/// ([`crate::types::is_numeric_literal`]) rejects them and so does this
/// parser.
#[derive(Clone, Copy, Debug)]
enum Num {
    /// An exactly-represented integer.
    Int(i128),
    /// The documented escape hatch: a finite `f64` for fractional lexemes.
    Float(f64),
}

impl Num {
    /// Parse a numeric lexeme: `i128` first (exact), then finite `f64` — but
    /// **only for fractional lexemes**. An integer lexeme beyond the `i128`
    /// range must never fall through to a rounded `f64` (the reasoner models
    /// the lexeme exactly, and "an integer-lexeme operand is an `i128`" is the
    /// documented contract): it parses as no number at all, and the operator
    /// that popped it raises the loud overflow error ([`literal_overflow`]).
    /// Non-finite lexemes are not numbers.
    fn parse(word: &str) -> Option<Num> {
        if let Ok(n) = word.parse::<i128>() {
            return Some(Num::Int(n));
        }
        if is_integer_lexeme(word) {
            // Out-of-range integer: exactness is unrepresentable — not a
            // rounded float, not a number.
            return None;
        }
        match word.parse::<f64>() {
            Ok(f) if f.is_finite() => Some(Num::Float(f)),
            _ => None,
        }
    }

    fn as_f64(self) -> f64 {
        match self {
            Num::Int(n) => n as f64,
            Num::Float(f) => f,
        }
    }

    fn render(self) -> String {
        match self {
            Num::Int(n) => n.to_string(),
            Num::Float(f) => f.to_string(),
        }
    }

    fn is_zero(self) -> bool {
        match self {
            Num::Int(n) => n == 0,
            Num::Float(f) => f == 0.0,
        }
    }

    /// Exact equality where both operands are exact; the `f64` escape hatch
    /// compares as `f64` when either side is fractional.
    fn num_eq(self, other: Num) -> bool {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => a == b,
            (a, b) => a.as_f64() == b.as_f64(),
        }
    }

    /// Exact ordering where both operands are exact (see [`Num::num_eq`]).
    fn num_cmp(self, other: Num) -> std::cmp::Ordering {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => a.cmp(&b),
            (a, b) => a
                .as_f64()
                .partial_cmp(&b.as_f64())
                .expect("finite floats always compare"),
        }
    }
}

fn overflow(op: &str) -> EvalError {
    operator_error(format!(
        "numeric overflow: `{op}` exceeds the exact integer range (i128)"
    ))
}

// An integer-form lexeme is exact by contract (`Num` docs above): one that
// overflows `i128` must fail loudly, never round through `f64`.
use crate::types::is_integer_literal as is_integer_lexeme;

/// The loud error for an integer lexeme beyond the exact `i128` range —
/// the literal-operand sibling of [`overflow`] ("overflow is a runtime error,
/// never a silent wrap or rounding").
fn literal_overflow(word: &str) -> EvalError {
    operator_error(format!(
        "numeric overflow: integer literal `{word}` exceeds the exact integer range (i128)"
    ))
}

fn pop_num<T: Quotable>(stack: &mut Vec<T>) -> Result<Num, EvalError> {
    let value = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
    let word = scalar_word(&value)?;
    Num::parse(&word).ok_or_else(|| {
        if is_integer_lexeme(&word) {
            literal_overflow(&word)
        } else {
            operator_error(format!("expected numeric value, found `{word}`"))
        }
    })
}

fn pop_int<T: Quotable>(stack: &mut Vec<T>) -> Result<i128, EvalError> {
    let value = stack.pop().ok_or_else(|| stack_underflow(1, stack.len()))?;
    let word = scalar_word(&value)?;
    word.parse::<i128>()
        .map_err(|_| operator_error(format!("expected integer value, found `{word}`")))
}

fn push_word<T: From<Token>>(stack: &mut Vec<T>, word: impl Into<String>) {
    stack.push(T::from(Token::Word(word.into())));
}

fn push_num<T: From<Token>>(stack: &mut Vec<T>, n: Num) {
    push_word(stack, n.render());
}

fn push_int<T: From<Token>>(stack: &mut Vec<T>, n: i128) {
    push_word(stack, n.to_string());
}

fn numeric_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(Num, Num) -> Result<Num, EvalError>,
{
    require_len(stack, 2)?;
    let b = pop_num(stack)?;
    let a = pop_num(stack)?;
    let c = f(a, b)?;
    push_num(stack, c);
    Ok(())
}

fn integer_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(i128, i128) -> Result<i128, EvalError>,
{
    require_len(stack, 2)?;
    let b = pop_int(stack)?;
    let a = pop_int(stack)?;
    let c = f(a, b)?;
    push_int(stack, c);
    Ok(())
}

fn bool_bin<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(bool, bool) -> bool,
{
    require_len(stack, 2)?;
    let b = stack.pop().unwrap().is_truthy();
    let a = stack.pop().unwrap().is_truthy();
    push_word(stack, f(a, b).to_string());
    Ok(())
}

fn plus<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| match (a, b) {
        (Num::Int(a), Num::Int(b)) => a.checked_add(b).map(Num::Int).ok_or_else(|| overflow("+")),
        (a, b) => Ok(Num::Float(a.as_f64() + b.as_f64())),
    })
}

fn minus<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| match (a, b) {
        (Num::Int(a), Num::Int(b)) => a.checked_sub(b).map(Num::Int).ok_or_else(|| overflow("-")),
        (a, b) => Ok(Num::Float(a.as_f64() - b.as_f64())),
    })
}

fn multiply<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| match (a, b) {
        (Num::Int(a), Num::Int(b)) => a.checked_mul(b).map(Num::Int).ok_or_else(|| overflow("*")),
        (a, b) => Ok(Num::Float(a.as_f64() * b.as_f64())),
    })
}

fn divide<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| {
        if b.is_zero() {
            return Err(operator_error("division by zero"));
        }
        match (a, b) {
            // Exact when the quotient is an integer; otherwise the f64 escape
            // hatch (the model's division is exact rational — documented gap).
            // `a % b` and `a / b` both overflow for `i128::MIN / -1`; checked
            // arithmetic raises a runtime error instead of panicking (debug)
            // or silently wrapping to `i128::MIN` (release).
            (Num::Int(a), Num::Int(b)) => {
                let exact = a.checked_rem(b).map(|r| r == 0).ok_or_else(|| overflow("/"))?;
                if exact {
                    Ok(Num::Int(a.checked_div(b).ok_or_else(|| overflow("/"))?))
                } else {
                    Ok(Num::Float((a as f64) / (b as f64)))
                }
            }
            (a, b) => Ok(Num::Float(a.as_f64() / b.as_f64())),
        }
    })
}

fn modulo<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_bin(stack, |a, b| {
        if b.is_zero() {
            return Err(operator_error("modulo by zero"));
        }
        match (a, b) {
            (Num::Int(a), Num::Int(b)) => {
                // `a % b` overflows for `i128::MIN % -1`; checked arithmetic
                // raises a runtime error instead of panicking or wrapping.
                Ok(Num::Int(a.checked_rem(b).ok_or_else(|| overflow("%"))?))
            }
            (a, b) => Ok(Num::Float(a.as_f64() % b.as_f64())),
        }
    })
}

fn bit_or<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a | b))
}

fn bit_and<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a & b))
}

fn bit_xor<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| Ok(a ^ b))
}

fn bit_not<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let a = pop_int(stack)?;
    push_int(stack, !a);
    Ok(())
}

fn shift_amount(n: i128) -> Result<u32, EvalError> {
    u32::try_from(n).map_err(|_| operator_error(format!("invalid shift amount `{n}`")))
}

fn shift_left<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| {
        a.checked_shl(shift_amount(b)?)
            .ok_or_else(|| operator_error(format!("invalid shift amount `{b}`")))
    })
}

fn shift_right<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    integer_bin(stack, |a, b| {
        a.checked_shr(shift_amount(b)?)
            .ok_or_else(|| operator_error(format!("invalid shift amount `{b}`")))
    })
}

fn bool_or<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    bool_bin(stack, |a, b| a || b)
}

fn bool_and<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    bool_bin(stack, |a, b| a && b)
}

fn bool_not<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 1)?;
    let a = stack.pop().unwrap().is_truthy();
    push_word(stack, (!a).to_string());
    Ok(())
}

/// `=`/`==`/`!=` are **numeric** on numeric operands (the shadow models `=` as
/// real equality, so `1 1.0 =` is `true` under every embedder `Value` — never
/// rendering-dependent); non-numeric operands fall back to token equality
/// (their Tier-0 contract is same-type polymorphic equality, and quotations /
/// free words have no numeric reading). An **overflowing integer lexeme** is
/// numeric but unrepresentable: comparing it is the loud
/// [`literal_overflow`] error, never a token comparison of digit strings (nor,
/// pre-fix, a rounded-`f64` equality that called two distinct integers equal).
fn values_equal<T: Quotable>(a: &T, b: &T) -> Result<bool, EvalError> {
    let a_tokens = a.to_tokens();
    let b_tokens = b.to_tokens();
    if let ([Token::Word(aw)], [Token::Word(bw)]) = (a_tokens.as_slice(), b_tokens.as_slice()) {
        for w in [aw, bw] {
            if is_integer_lexeme(w) && w.parse::<i128>().is_err() {
                return Err(literal_overflow(w));
            }
        }
        if let (Some(an), Some(bn)) = (Num::parse(aw), Num::parse(bw)) {
            return Ok(an.num_eq(bn));
        }
    }
    Ok(a_tokens == b_tokens)
}

fn eq<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let b = stack.pop().unwrap();
    let a = stack.pop().unwrap();
    push_word(stack, values_equal(&a, &b)?.to_string());
    Ok(())
}

fn ne<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    require_len(stack, 2)?;
    let b = stack.pop().unwrap();
    let a = stack.pop().unwrap();
    push_word(stack, (!values_equal(&a, &b)?).to_string());
    Ok(())
}

fn numeric_cmp<T, F>(stack: &mut Vec<T>, f: F) -> Result<(), EvalError>
where
    T: Quotable,
    F: FnOnce(std::cmp::Ordering) -> bool,
{
    require_len(stack, 2)?;
    let b = pop_num(stack)?;
    let a = pop_num(stack)?;
    push_word(stack, f(a.num_cmp(b)).to_string());
    Ok(())
}

fn lt<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, std::cmp::Ordering::is_lt)
}

fn le<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, std::cmp::Ordering::is_le)
}

fn gt<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, std::cmp::Ordering::is_gt)
}

fn ge<T: Quotable>(stack: &mut Vec<T>, _eval: &Evaluator<T>) -> Result<(), EvalError> {
    numeric_cmp(stack, std::cmp::Ordering::is_ge)
}

fn span() -> Span {
    Span { start: 0, end: 0 }
}

fn num_num_num_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::num(s)], 0, s),
        ),
    )
}

fn num_num_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::num(s), Ty::num(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn bool_bool_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::bool(s), Ty::bool(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn bool_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::bool(s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn same_same_bool_scheme() -> Scheme {
    let s = span();
    Scheme::new(
        vec![0],
        vec![0],
        WordTy::new(
            StackTy::new(vec![Ty::var(0, s), Ty::var(0, s)], 0, s),
            StackTy::new(vec![Ty::bool(s)], 0, s),
        ),
    )
}

fn num_num_num_refinements() -> [(&'static str, &'static str); 7] {
    [
        ("+", "+ : ( a: Num b: Num -- c: Num where c = a + b )"),
        ("-", "- : ( a: Num b: Num -- c: Num where c = a - b )"),
        ("*", "* : ( a: Num b: Num -- c: Num where c = a * b )"),
        (
            "/",
            "/ : ( a: Num b: Num where b * b > 0 -- c: Num where c = a / b )",
        ),
        // `%` mirrors `/`'s divisor demand (`b * b > 0`) so the gate rejects
        // `[ 5 0 % DROP ]` just as it rejects `[ 5 0 / DROP ]`. No output
        // guarantee: the modulo remainder is not expressible in the Real
        // predicate language (the same value-domain gap as the bitwise case),
        // and the demand alone closes the soundness hole.
        (
            "%",
            "% : ( a: Num b: Num where b * b > 0 -- c: Num )",
        ),
        // The shifts demand the runtime's accepted amount range (`checked_shl`
        // / `checked_shr` on i128: 0..=127) so the gate rejects `[ 1 -1 << ]`
        // and `[ 1 200 << ]` instead of admitting a runtime "invalid shift
        // amount" (BUGS §A5). `b * (127 - b) >= 0` is `0 <= b <= 127` as one
        // atom: a conjunction's negated goal is a disjunction the linear
        // reasoner treats as opaque (never discharges) — the same reason `/`
        // demands `b * b > 0` rather than `b != 0`. No output guarantee, and
        // the *integrality* of the amount stays unmodeled — that is the
        // documented bitwise gap.
        (
            "<<",
            "<< : ( a: Num b: Num where b * ( 127 - b ) >= 0 -- c: Num )",
        ),
        (
            ">>",
            ">> : ( a: Num b: Num where b * ( 127 - b ) >= 0 -- c: Num )",
        ),
    ]
}

/// Register standard stack combinators/manipulators on an evaluator.
pub fn register_stack_builtins<T>(evaluator: &mut Evaluator<T>)
where
    T: From<Token> + Clone,
{
    evaluator.define("DUP", dup::<T>);
    evaluator.define("DROP", drop::<T>);
    evaluator.define("SWAP", swap::<T>);
    evaluator.define("OVER", over::<T>);
    evaluator.define("ROT", rot::<T>);
    evaluator.define("-ROT", minus_rot::<T>);
    evaluator.define("NIP", nip::<T>);
    evaluator.define("TUCK", tuck::<T>);
    evaluator.define("2DUP", two_dup::<T>);
    evaluator.define("2DROP", two_drop::<T>);
    evaluator.define("2SWAP", two_swap::<T>);
    evaluator.define("2OVER", two_over::<T>);
    evaluator.define("2ROT", two_rot::<T>);
}

/// Register scalar arithmetic, comparison, boolean, and integer bitwise builtins.
///
/// Arithmetic operators use the language's single `Num` type. Bitwise operators
/// accept integer-valued `Num` lexemes at runtime and reject fractional values.
///
/// # Recorded limitation: bitwise contracts overpromise (gate-green ≠ crash-free)
///
/// The bitwise operators (`|`/`&`/`^`/`<<`/`>>`/`~`) are attested at
/// `( Num Num -- Num )` — a consequence of the ratified single-`Num` decision
/// (no Int/Float split) — but their runtime demands **integer-valued**
/// operands: `[ 1 0.5 | ] :main` passes `caternary check` and then fails at
/// runtime with ``expected integer value, found `0.5` ``. Nothing in Tier 1
/// demands integrality (the predicate language has no floor/frac), so this is
/// an **explicitly documented** soundness gap of the builtin contract set: a
/// green gate proves shape and refinement obligations, not freedom from
/// value-domain rejections by these operators. Embedders who need the gate to
/// carry that guarantee should register bitwise operators under their own
/// attested, integrality-aware contracts instead of these.
///
/// # Recorded limitation: arithmetic overflow is unmodeled (gate-green ≠ crash-free)
///
/// The same shape of gap affects `+`/`-`/`*`. The solver models `Num` as
/// unbounded `Real`, so these operators' guarantees (`c = a + b`, `c = a - b`,
/// `c = a * b`) can never overflow in the model; the runtime uses checked
/// `i128` and errors on overflow, so
/// `[ 170141183460469231731687303715884105727 1 + ] :main` (`i128::MAX + 1`)
/// passes `caternary check` and then fails at runtime with
/// ``numeric overflow: `+` exceeds the exact integer range (i128) ``. Closing
/// it needs a bounded-integer refinement (a range/integrality obligation) the
/// predicate language currently lacks, so — like the bitwise case above — this
/// is an explicitly documented soundness gap, not a silent one.
pub fn register_scalar_builtins<T>(evaluator: &mut Evaluator<T>)
where
    T: Quotable,
{
    evaluator.define("+", plus::<T>);
    evaluator.define("-", minus::<T>);
    evaluator.define("*", multiply::<T>);
    evaluator.define("/", divide::<T>);
    evaluator.define("%", modulo::<T>);
    evaluator.define("|", bit_or::<T>);
    evaluator.define("&", bit_and::<T>);
    evaluator.define("^", bit_xor::<T>);
    evaluator.define("~", bit_not::<T>);
    evaluator.define("<<", shift_left::<T>);
    evaluator.define(">>", shift_right::<T>);
    evaluator.define("||", bool_or::<T>);
    evaluator.define("or", bool_or::<T>);
    evaluator.define("&&", bool_and::<T>);
    evaluator.define("and", bool_and::<T>);
    evaluator.define("!", bool_not::<T>);
    evaluator.define("not", bool_not::<T>);
    evaluator.define("=", eq::<T>);
    evaluator.define("==", eq::<T>);
    evaluator.define("!=", ne::<T>);
    evaluator.define("<", lt::<T>);
    evaluator.define("<=", le::<T>);
    evaluator.define(">", gt::<T>);
    evaluator.define(">=", ge::<T>);

    for op in ["+", "-", "*", "/", "%", "|", "&", "^", "<<", ">>"] {
        evaluator.register_operator_with_contract(op, num_num_num_scheme());
    }
    evaluator.register_operator_with_contract("~", {
        let s = span();
        Scheme::new(
            vec![],
            vec![0],
            WordTy::new(
                StackTy::new(vec![Ty::num(s)], 0, s),
                StackTy::new(vec![Ty::num(s)], 0, s),
            ),
        )
    });
    for op in ["||", "or", "&&", "and"] {
        evaluator.register_operator_with_contract(op, bool_bool_bool_scheme());
    }
    for op in ["!", "not"] {
        evaluator.register_operator_with_contract(op, bool_bool_scheme());
    }
    for op in ["=", "==", "!="] {
        evaluator.register_operator_with_contract(op, same_same_bool_scheme());
    }
    for op in ["<", "<=", ">", ">="] {
        evaluator.register_operator_with_contract(op, num_num_bool_scheme());
    }
    for (_, refinement) in num_num_num_refinements() {
        evaluator
            .attach_refinement(refinement)
            .expect("builtin arithmetic refinement must parse");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Evaluator, Quotable, QuoteItem, Scheme, Span, StackTy, Token, Ty, WordTy,
        check_whole_program, parse, parse_with_spans, quote_items_from_tokens,
        quote_items_to_tokens, quote_items_to_values,
    };

    use super::{register_scalar_builtins, register_stack_builtins};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Number(i32);

    impl From<Token> for Number {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => Number(w.parse().unwrap_or(0)),
                Token::Bracket(_) => Number(0),
            }
        }
    }

    impl Quotable for Number {
        fn as_quotation(&self) -> Option<&[QuoteItem<Self>]> {
            None
        }

        fn from_quotation(_items: Vec<QuoteItem<Self>>) -> Self {
            Number(0)
        }

        fn to_tokens(&self) -> Vec<Token> {
            vec![Token::Word(self.0.to_string())]
        }

        fn is_truthy(&self) -> bool {
            self.0 != 0
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            None
        }

        fn from_sequence(_elements: Vec<Self>) -> Self {
            Number(0)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Value {
        Word(String),
        /// Exact integers, mirroring the REPL's recommended embedder shape.
        Int(i128),
        /// The documented f64 escape hatch for fractional lexemes.
        Float(f64),
        Bool(bool),
        Quotation(Vec<QuoteItem<Value>>),
    }

    impl From<Token> for Value {
        fn from(token: Token) -> Self {
            match token {
                Token::Word(w) => {
                    if let Ok(n) = w.parse::<i128>() {
                        Value::Int(n)
                    } else if crate::types::is_integer_literal(&w) {
                        // Out-of-range integer: keep the exact lexeme (the
                        // operators reject it loudly) — never a rounded f64.
                        Value::Word(w)
                    } else if let Ok(f) = w.parse::<f64>()
                        && f.is_finite()
                    {
                        Value::Float(f)
                    } else if w == "true" {
                        Value::Bool(true)
                    } else if w == "false" {
                        Value::Bool(false)
                    } else {
                        Value::Word(w)
                    }
                }
                Token::Bracket(tokens) => Value::Quotation(quote_items_from_tokens(&tokens)),
            }
        }
    }

    impl Quotable for Value {
        fn as_quotation(&self) -> Option<&[QuoteItem<Self>]> {
            match self {
                Value::Quotation(tokens) => Some(tokens),
                _ => None,
            }
        }

        fn from_quotation(items: Vec<QuoteItem<Self>>) -> Self {
            Value::Quotation(items)
        }

        fn to_tokens(&self) -> Vec<Token> {
            match self {
                Value::Word(w) => vec![Token::Word(w.clone())],
                Value::Int(n) => vec![Token::Word(n.to_string())],
                Value::Float(f) => vec![Token::Word(f.to_string())],
                Value::Bool(b) => vec![Token::Word(b.to_string())],
                Value::Quotation(tokens) => vec![Token::Bracket(quote_items_to_tokens(tokens))],
            }
        }

        fn is_truthy(&self) -> bool {
            match self {
                Value::Bool(b) => *b,
                Value::Int(n) => *n != 0,
                Value::Float(f) => *f != 0.0,
                _ => true,
            }
        }

        fn as_sequence(&self) -> Option<Vec<Self>> {
            match self {
                Value::Quotation(tokens) => Some(quote_items_to_values(tokens)),
                _ => None,
            }
        }

        fn from_sequence(elements: Vec<Self>) -> Self {
            Value::Quotation(elements.into_iter().map(QuoteItem::Push).collect())
        }
    }

    #[test]
    fn registers_and_runs_builtins() {
        let mut eval: Evaluator<Number> = Evaluator::new();
        register_stack_builtins(&mut eval);

        let tokens = parse("1 2 DUP SWAP OVER ROT 2DUP 2DROP").unwrap();
        let stack = eval.eval(&tokens).unwrap();

        assert_eq!(stack, vec![Number(1), Number(2), Number(2), Number(2)]);
    }

    // =======================================================================
    // Regression: the deliberate runtime numeric semantics (the solver models
    // `Num` as exact Real; the runtime computes exactly wherever it can)
    // =======================================================================

    /// Integer arithmetic is exact `i128`, not `f64`: `9007199254740993 1 +`
    /// is `…994`, not the f64-rounded `…992`.
    #[test]
    fn integer_arithmetic_is_exact_beyond_f64_precision() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let tokens = parse("9007199254740993 1 +").unwrap();
        let stack = eval.eval(&tokens).unwrap();
        assert_eq!(stack, vec![Value::Int(9007199254740994)]);
    }

    /// Exact-integer overflow is a runtime error — never a silent wrap or a
    /// silent fall to rounded floats.
    #[test]
    fn integer_overflow_is_a_runtime_error() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let tokens = parse("170141183460469231731687303715884105727 1 +").unwrap();
        assert!(eval.eval(&tokens).is_err());
    }

    /// `=` is numeric on numeric operands — `1 1.0 =` is `true` regardless of
    /// how the embedder's `Value` renders numbers (it used to compare token
    /// renderings, making the result rendering-dependent).
    #[test]
    fn equality_is_numeric_not_rendering_dependent() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let stack = eval.eval(&parse("1 1.0 =").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Bool(true)]);
        let stack = eval.eval(&parse("1 2 =").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Bool(false)]);
        // Non-numeric operands keep token equality.
        let stack = eval.eval(&parse("[ 1 ] [ 1 ] =").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Bool(true)]);
        let stack = eval.eval(&parse("[ 1 ] [ 2 ] !=").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Bool(true)]);
    }

    /// Ordering comparisons are exact for integers beyond f64 precision.
    #[test]
    fn comparison_is_exact_beyond_f64_precision() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let stack = eval
            .eval(&parse("9007199254740993 9007199254740992 >").unwrap())
            .unwrap();
        assert_eq!(stack, vec![Value::Bool(true)]);
    }

    /// `inf`/`NaN` are not numbers: the literal grammar rejects them and the
    /// runtime refuses to compute with them.
    #[test]
    fn non_finite_lexemes_are_not_numbers() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        assert!(eval.eval(&parse("inf 1 +").unwrap()).is_err());
        assert!(eval.eval(&parse("NaN 1 +").unwrap()).is_err());
    }

    /// Fractional values take the documented `f64` escape hatch; an integer
    /// quotient stays exact.
    #[test]
    fn division_is_exact_when_integral_and_f64_otherwise() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let stack = eval.eval(&parse("6 3 /").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Int(2)]);
        let stack = eval.eval(&parse("1 2 /").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Float(0.5)]);
    }

    /// `i128::MIN / -1` and `i128::MIN % -1` overflow `i128` (the quotient
    /// `2^127` is out of range). The unchecked `%`/`/` used to panic in debug
    /// and silently wrap in release; both must be a runtime error instead.
    #[test]
    fn min_div_minus_one_is_a_runtime_error_not_a_crash() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        // `-170141183460469231731687303715884105728` is `i128::MIN`.
        let err = eval
            .eval(&parse("-170141183460469231731687303715884105728 -1 /").unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("overflow"), "got: {err}");
        let err = eval
            .eval(&parse("-170141183460469231731687303715884105728 -1 %").unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("overflow"), "got: {err}");
    }

    /// BUGS §A4: an integer lexeme beyond `i128` used to fall through to a
    /// rounded `f64`, so two *distinct* integers compared equal (the reasoner
    /// models the lexemes exactly). The runtime must fail loudly instead —
    /// for `=`/`!=` and for the numeric operators alike — never round.
    #[test]
    fn out_of_range_integer_literals_error_instead_of_rounding() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        // 2^127 and 2^127 + 1: distinct integers, identical as f64.
        let src = "170141183460469231731687303715884105728 \
                   170141183460469231731687303715884105729 =";
        let err = eval.eval(&parse(src).unwrap()).unwrap_err();
        assert!(err.to_string().contains("overflow"), "got: {err}");
        let err = eval
            .eval(&parse("170141183460469231731687303715884105728 1 +").unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("overflow"), "got: {err}");
        // In-range integers and fractional lexemes are untouched.
        let stack = eval.eval(&parse("1 1.0 =").unwrap()).unwrap();
        assert_eq!(stack, vec![Value::Bool(true)]);
    }

    #[test]
    fn forth_rotations_run() {
        let mut eval: Evaluator<Number> = Evaluator::new();
        register_stack_builtins(&mut eval);

        let tokens = parse("1 2 3 -ROT 4 5 6 2ROT").unwrap();
        let stack = eval.eval(&tokens).unwrap();

        assert_eq!(
            stack,
            vec![
                Number(2),
                Number(4),
                Number(5),
                Number(6),
                Number(3),
                Number(1)
            ]
        );
    }

    #[test]
    fn reports_underflow() {
        let mut eval: Evaluator<Number> = Evaluator::new();
        register_stack_builtins(&mut eval);

        let tokens = parse("DROP").unwrap();
        let err = eval.eval(&tokens).unwrap_err();

        assert!(
            err.to_string()
                .contains("stack underflow: need at least 1 values, found 0")
        );
    }

    #[test]
    fn scalar_builtins_run_common_operations() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);

        let tokens = parse(
            "false true || false true && 0 2 | 1 3 & 1 3 ^ \
             2 3 + 2 3 * 2 3 - 2 4 /",
        )
        .unwrap();
        let stack = eval.eval(&tokens).unwrap();

        assert_eq!(
            stack,
            vec![
                Value::Bool(true),
                Value::Bool(false),
                Value::Int(2),
                Value::Int(1),
                Value::Int(2),
                Value::Int(5),
                Value::Int(6),
                Value::Int(-1),
                Value::Float(0.5),
            ]
        );
    }

    #[test]
    fn scalar_builtins_register_type_contracts() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let tokens =
            parse_with_spans("[ 2 3 + DROP 2 4 / DROP false true || DROP 1 3 ^ DROP ] :main")
                .unwrap();
        eval.load_with_spans(&tokens).unwrap();

        check_whole_program(&eval, crate::SmtLibSolver::new).unwrap();
    }

    #[test]
    fn arithmetic_refinement_publishes_exact_sum() {
        let mut eval: Evaluator<Value> = Evaluator::new();
        register_scalar_builtins(&mut eval);
        let s = Span { start: 0, end: 0 };
        eval.register_operator_with_contract(
            "need5",
            Scheme::new(
                vec![],
                vec![0],
                WordTy::new(StackTy::new(vec![Ty::num(s)], 0, s), StackTy::empty(0, s)),
            ),
        );
        eval.attach_refinement("need5 : ( n: Num where n >= 5 -- )")
            .unwrap();
        let tokens = parse_with_spans("[ 2 3 + need5 ] :main").unwrap();
        eval.load_with_spans(&tokens).unwrap();

        check_whole_program(&eval, crate::SmtLibSolver::new).unwrap();
    }

    /// BUGS §A5: the shift amount's runtime range (0..=127 — `checked_shl` /
    /// `checked_shr` on i128) is modeled as a demand, so an out-of-range
    /// amount fails the gate instead of surfacing only as a runtime "invalid
    /// shift amount"; in-range amounts still discharge.
    #[test]
    fn shift_amount_range_is_gated() {
        let gate = |src: &str| {
            let mut eval: Evaluator<Value> = Evaluator::new();
            register_scalar_builtins(&mut eval);
            let tokens = parse_with_spans(src).unwrap();
            eval.load_with_spans(&tokens).unwrap();
            check_whole_program(&eval, crate::SmtLibSolver::new)
        };
        for src in [
            "[ 1 -1 << DROP ] :main",
            "[ 1 200 << DROP ] :main",
            "[ 1 128 >> DROP ] :main",
        ] {
            assert!(gate(src).is_err(), "out-of-range shift must fail: {src}");
        }
        for src in [
            "[ 1 0 << DROP ] :main",
            "[ 1 3 << DROP ] :main",
            "[ 1 127 << DROP ] :main",
            "[ 8 2 >> DROP ] :main",
        ] {
            assert!(gate(src).is_ok(), "in-range shift must pass: {src}");
        }
    }
}
