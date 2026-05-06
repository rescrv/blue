# Caternary

Caternary is a small concatenative language with first-class quotations and a pattern-rewrite optimizer.

## Syntax

- Program = `shvar::split` shell words, then FORTH-style bracket processing.
- `word` pushes a word token (or invokes an operator if one is registered under that exact name).
- `[ ... ]` creates a quotation token.
- Shell quotes and escapes are resolved before FORTH parsing, so `"hello world"` is one word and `hello\ world` is also one word.
- Case is preserved and significant (`Scan`, `SCAN`, and `scan` are different words).
- Brackets nest arbitrarily.

Concrete examples (top of stack is on the right):

```text
Program: 2 DUP
Meaning: push 2, then duplicate it
Stack:   2 2

Program: 1 2 SWAP
Meaning: push 1 and 2, then swap top two values
Stack:   2 1

Program: 2 [DUP] CALL
Meaning: push 2, push quotation [DUP], then execute the quotation
Stack:   2 2

Program: ["hello world"] CALL
Meaning: push a quotation containing one word token `hello world`, then execute it
Stack:   hello world
```

Parse errors include byte spans for unmatched `[` or `]`.

```rust
use caternary::parse_with_spans;
let tokens = parse_with_spans("aa [bbb]")?;
assert_eq!(tokens[0].span.start, 0);
assert_eq!(tokens[1].span.start, 3);
```

## Minimal Evaluator Setup

```rust
use caternary::{Evaluator, Quotable, Token, parse, register_all_builtins};

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Int(i64),
    Bool(bool),
    Word(String),
    Quotation(Vec<Token>),
    Sequence(Vec<Value>),
}

impl From<Token> for Value {
    fn from(token: Token) -> Self {
        match token {
            Token::Word(w) => {
                if let Ok(n) = w.parse::<i64>() { Value::Int(n) }
                else if w == "true" { Value::Bool(true) }
                else if w == "false" { Value::Bool(false) }
                else { Value::Word(w) }
            }
            Token::Bracket(q) => Value::Quotation(q),
        }
    }
}

impl Quotable for Value {
    fn as_quotation(&self) -> Option<&[Token]> {
        match self { Value::Quotation(q) => Some(q), _ => None }
    }

    fn to_tokens(&self) -> Vec<Token> {
        match self {
            Value::Int(n) => vec![Token::Word(n.to_string())],
            Value::Bool(b) => vec![Token::Word(b.to_string())],
            Value::Word(w) => vec![Token::Word(w.clone())],
            Value::Quotation(q) => vec![Token::Bracket(q.clone())],
            Value::Sequence(xs) => vec![Token::Bracket(xs.iter().flat_map(|x| x.to_tokens()).collect())],
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            _ => true,
        }
    }

    fn as_sequence(&self) -> Option<Vec<Self>> {
        match self {
            Value::Sequence(xs) => Some(xs.clone()),
            Value::Quotation(q) => Some(q.iter().cloned().map(Value::from).collect()),
            _ => None,
        }
    }

    fn from_sequence(elements: Vec<Self>) -> Self {
        Value::Sequence(elements)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut eval: Evaluator<Value> = Evaluator::new();
    register_all_builtins(&mut eval);

    let program = parse("2 [DUP] CALL")?;
    let stack = eval.eval(&program)?;
    println!("{stack:?}"); // [Int(2), Int(2)]
    Ok(())
}
```

## Language Features By Example

Assume the evaluator has all builtins registered and helper operators like `ADD`, `MUL`, `GT`, `EVEN`.

### Stack Builtins

| Operator | Example | Resulting stack |
|---|---|---|
| `DUP` | `1 DUP` | `1 1` |
| `DROP` | `1 2 DROP` | `1` |
| `SWAP` | `1 2 SWAP` | `2 1` |
| `OVER` | `1 2 OVER` | `1 2 1` |
| `ROT` | `1 2 3 ROT` | `2 3 1` |
| `NIP` | `1 2 NIP` | `2` |
| `TUCK` | `1 2 TUCK` | `2 1 2` |
| `2DUP` | `1 2 2DUP` | `1 2 1 2` |
| `2DROP` | `1 2 3 4 2DROP` | `1 2` |
| `2SWAP` | `1 2 3 4 2SWAP` | `3 4 1 2` |
| `2OVER` | `1 2 3 4 2OVER` | `1 2 3 4 1 2` |

### Core Combinators

| Operator | Example | Resulting stack |
|---|---|---|
| `CALL` | `1 2 [ADD] CALL` | `3` |
| `DIP` | `1 2 3 [ADD] DIP` | `3 3` |
| `KEEP` | `5 [DUP MUL] KEEP` | `25 5` |
| `BI` | `5 [DUP ADD] [DUP MUL] BI` | `10 25` |
| `BI*` | `3 4 [DUP MUL] [DUP ADD] BI*` | `9 8` |
| `BI@` | `3 4 [DUP MUL] BI@` | `9 16` |
| `CLEAVE` | `5 [[DUP ADD] [DUP MUL] [1 ADD]] CLEAVE` | `10 25 6` |
| `SPREAD` | `1 2 3 [[DUP ADD] [DUP MUL] [1 ADD]] SPREAD` | `2 4 4` |
| `COMPOSE` | `[1 ADD] [2 MUL] COMPOSE` | `[1 ADD 2 MUL]` |
| `CURRY` | `10 [ADD] CURRY` | `[10 ADD]` |

### Conditionals

| Operator | Example | Resulting stack |
|---|---|---|
| `IF` | `1 [10] [20] IF` | `10` |
| `IF` | `0 [10] [20] IF` | `20` |
| `WHEN` | `true [99] WHEN` | `99` |
| `UNLESS` | `false [99] UNLESS` | `99` |

### Sequence Combinators

`[1 2 3]` is a quotation, and in the default `Quotable` style above it can also act as a sequence.

| Operator | Example | Resulting stack |
|---|---|---|
| `MAP` | `[1 2 3] [DUP MUL] MAP` | `[1 4 9]` |
| `FILTER` | `[1 2 3 4] [EVEN] FILTER` | `[2 4]` |
| `FOLD` | `[1 2 3] 0 [ADD] FOLD` | `6` |
| `EACH` | `[1 2 3] [DROP] EACH` | *(unchanged)* |

Notes:

- `MAP` quotation must consume one element and leave one result.
- `FILTER` quotation must consume one element and leave one truthy/falsy value.
- `FOLD` quotation must consume `(accumulator, element)` and leave one accumulator.
- `EACH` quotation must consume one element and leave no extra values.

## Optimizer Features

The optimizer rewrites token streams with pattern rules until fixpoint, with cycle detection.

```rust
use caternary::{Optimizer, parse};

let mut opt = Optimizer::new();
opt.add_rule("DUP $X FILTER", "$X FILTER DUP")?;
let out = opt.optimize(parse("A SCAN DUP [foo < 5] FILTER")?);
assert_eq!(out, parse("A SCAN [foo < 5] FILTER DUP")?);
```

### Pattern Language

- `$x` matches exactly one token (word or bracket).
- `$*xs` matches zero or more tokens (greedy, backtracking).
- Literal brackets match nested structure.
- Repeated variables must match identical captures.

Examples:

| Rule | Input | Output |
|---|---|---|
| `"$X DUP" -> "$X $X"` | `A DUP` | `A A` |
| `"[$*X] UNWRAP" -> "$*X"` | `[A B C] UNWRAP` | `A B C` |
| `"[INNER $X] OUTER" -> "RESULT $X"` | `[INNER foo] OUTER` | `RESULT foo` |
| `"BEGIN $*X MID $*X END" -> "MATCHED"` | `BEGIN A B MID A B END` | `MATCHED` |

### Rule Behavior

- Rules are tried in insertion order.
- One successful rewrite is applied per iteration, then matching restarts.
- Optimization stops at fixpoint or when a cycle is detected.
- `$*` matching uses a configurable backtracking budget:

```rust
opt.set_max_backtracks(1);
```

### Rule Errors

`Rule::new` validates rules and returns:

- `UnboundVariable` if replacement references variables not in the pattern.
- `InvalidVariableName` for invalid identifiers after `$`/`$*`.
- `VariableArityMismatch` if the same variable is used as both `$x` and `$*x`.
