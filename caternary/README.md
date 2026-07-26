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

## Command-Line Tools

`caternary check` reads a whole program from standard input, loads its
definitions, and runs the whole-program checker. A checked program must define
`main`:

```sh
printf '[ 1 2 + ] :main\n' | caternary check
```

`caternary repl` starts the development REPL. Normal input is parsed, loaded,
and evaluated against a persistent definition table and stack. Definition pairs
such as `[ 1 + ] :inc` are loaded but not pushed onto the runtime stack.

REPL meta-commands start with `:`:

| Command | Behavior |
|---|---|
| `:help` | Show REPL commands. |
| `:quit`, `:q` | Exit the REPL. |
| `:stack` | Print the current stack. |
| `:clear` | Clear the current stack. |
| `:reset` | Clear stack and session definitions. |
| `:defs` | List loaded user definitions as `name = [body]`. |
| `:ops` | List typed core and registered operators. |
| `:type <program>` | Infer and print the stack effect of a program snippet. |
| `:check [program]` | Run the whole-program checker on the session, optionally plus a temporary program. |
| `:load <path>` | Load and evaluate a source file transactionally. |

REPL input is transactional: a parse, load, type, check, or evaluation error
leaves the previous stack and definitions intact. If an input line has an
unclosed `[` or shell quote, the REPL continues with a `... ` prompt until the
program is complete. History is persisted in `~/.caternary_history` when
available.

## Language Features By Example

Assume the evaluator has all builtins registered. Examples that use `EVEN` assume
a host predicate with that name is also registered.

### Stack Builtins

| Operator | Example | Resulting stack |
|---|---|---|
| `DUP` | `1 DUP` | `1 1` |
| `DROP` | `1 2 DROP` | `1` |
| `SWAP` | `1 2 SWAP` | `2 1` |
| `OVER` | `1 2 OVER` | `1 2 1` |
| `ROT` | `1 2 3 ROT` | `2 3 1` |
| `-ROT` | `1 2 3 -ROT` | `3 1 2` |
| `NIP` | `1 2 NIP` | `2` |
| `TUCK` | `1 2 TUCK` | `2 1 2` |
| `2DUP` | `1 2 2DUP` | `1 2 1 2` |
| `2DROP` | `1 2 3 4 2DROP` | `1 2` |
| `2SWAP` | `1 2 3 4 2SWAP` | `3 4 1 2` |
| `2OVER` | `1 2 3 4 2OVER` | `1 2 3 4 1 2` |
| `2ROT` | `1 2 3 4 5 6 2ROT` | `3 4 5 6 1 2` |

### Core Combinators

| Operator | Example | Resulting stack |
|---|---|---|
| `CALL` | `1 2 [+] CALL` | `3` |
| `DIP` | `1 2 3 [+] DIP` | `3 3` |
| `2DIP` | `1 2 3 4 [+] 2DIP` | `3 3 4` |
| `3DIP` | `1 2 3 4 5 [+] 3DIP` | `3 3 4 5` |
| `KEEP` | `5 [DUP *] KEEP` | `25 5` |
| `2KEEP` | `3 4 [+] 2KEEP` | `7 3 4` |
| `3KEEP` | `2 3 4 [+ +] 3KEEP` | `9 2 3 4` |
| `BI` | `5 [DUP +] [DUP *] BI` | `10 25` |
| `BI*` | `3 4 [DUP *] [DUP +] BI*` | `9 8` |
| `BI@` | `3 4 [DUP *] BI@` | `9 16` |
| `TRI` | `5 [DUP +] [DUP *] [1 +] TRI` | `10 25 6` |
| `TRI*` | `3 4 5 [DUP *] [DUP +] [1 +] TRI*` | `9 8 6` |
| `TRI@` | `3 4 5 [DUP *] TRI@` | `9 16 25` |
| `COMPOSE` | `[1 +] [2 *] COMPOSE` | `[1 + 2 *]` |
| `CURRY` | `10 [+] CURRY` | `[10 +]` |
| `2CURRY` | `10 20 [+ +] 2CURRY` | `[10 20 + +]` |
| `3CURRY` | `1 2 3 [+ + +] 3CURRY` | `[1 2 3 + + +]` |

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
| `MAP` | `[1 2 3] [DUP *] MAP` | `[1 4 9]` |
| `FILTER` | `[1 2 3 4] [EVEN] FILTER` | `[2 4]` |
| `FOLD` | `[1 2 3] 0 [+] FOLD` | `6` |
| `EACH` | `[1 2 3] [DROP] EACH` | *(unchanged)* |

Notes:

- `MAP` quotation must consume one element and leave one result.
- `FILTER` quotation must consume one element and leave one truthy/falsy value.
- `FOLD` quotation must consume `(accumulator, element)` and leave one accumulator.
- `EACH` quotation must consume one element and leave no extra values.

### Locals

A word `>name` is a *binder*: it pops the top of the stack and binds it to a
local named `name`. A bare word that matches a bound local is a *reference*: it
pushes that local's value back onto the stack. Reference lookup happens
scope-first, ahead of definitions and operators, so a local shadows any
definition or builtin of the same name.

| Program | Meaning | Resulting stack |
|---|---|---|
| `5 >x x x` | bind `5` to `x`, then push it twice | `5 5` |
| `1 2 >b >a a b` | bind `2`→`b`, `1`→`a`, then push `a` then `b` | `1 2` |

A name is a leading ASCII letter or `_` followed by ASCII alphanumerics or `_`
(the same grammar as optimizer pattern variables).

Scoping rules:

- **Single-assignment.** Binding a name that is already bound in the same scope
  is an error (`5 >x 6 >x` fails).
- **Per-call lifetime.** Locals live in a per-call environment and are reset at
  the top level between calls; they do not persist from one `eval` to the next.
- **By-value capture.** A quotation captures the locals in scope *by value* at
  the point it is pushed. A reference that is free in the quotation body (not
  shadowed by a `>name` binder lexically preceding it) is baked in, so a
  quotation that escapes its defining scope still resolves its references. A
  quotation pushed with no locals in scope is byte-for-byte the original token
  stream, so programs without locals are unaffected.

```text
Program: 10 >x [x +] CALL
Meaning: bind 10 to x, push a quotation that captures x by value, then call it
Stack (with 5 on the stack before): 15
```

### User Function Definitions

A top-level `[ body ] :name` pair *defines* a function: the quotation before the
`:name` binder becomes the body of a function callable by `name`. Definitions
are collected by a static pre-pass, `Evaluator::load`, run before any
evaluation:

```rust
use caternary::{Evaluator, parse, register_all_builtins};

let mut eval: Evaluator<Value> = Evaluator::new();
register_all_builtins(&mut eval);

// Load definitions once, then evaluate many programs against them.
eval.load(&parse("[1 +] :inc [inc inc] :twice")?)?;

let stack = eval.eval(&parse("5 twice")?)?; // 7
```

Because `load` sees the whole token stream before execution:

- **Order does not matter.** Forward references resolve: `[inc inc] :twice` may
  appear before `[1 +] :inc`.
- **Recursion works.** A body resolves its own name at call time against the
  fully populated table.
- **Load is additive.** Call `load` repeatedly to assemble a library
  incrementally; a redefinition is rejected both within a call and across calls.

Resolution order is **locals, then definitions, then operators**: a `>name`
local shadows a definition, and a definition may shadow a builtin operator.
Definitions are visible inside quotations (`5 [inc] CALL` calls `inc`).

A `:`-prefixed word is legal *only* as the second half of a top-level
`[ body ] :name` pair. Every other occurrence is a static definition error,
reported by `load`:

- a binder nested inside a quotation,
- a binder with no preceding quotation,
- a binder following a non-quotation word,
- a malformed name (e.g. `:2x` or a bare `:`),
- a redefinition of an existing name.

As a backstop, evaluation itself fails loudly if a binder is reached without
having been loaded, so a stray `:name` can never be mistaken for a literal.

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
