# Caternary TODO — known holes

Found by code review (repros verified against the CLI/library as noted).
Holes 1, 2, and 4 are fixed and pinned by regression tests in
`src/solver.rs` and `src/check.rs` (grep for "Regression:"; see the
"Fixed-or-pinned elsewhere" section); the rest are tracked below.

## High

(none open)

## Medium

### Optimizer: no termination bound for size-increasing rules

- `Optimizer::optimize` (`src/optimizer.rs:427`) stops only at fixpoint or a
  revisited 64-bit `DefaultHasher` program hash. `A -> A A` doubles the token
  stream forever (OOM); a hash collision silently terminates early
  ("false cycle").
- Fix direction: add a max-iteration / max-program-size cap, and confirm
  hash hits with a real equality check (or store the programs).

### API sharp edges

- `Evaluator::load` is **non-atomic**: `[ 1 ] :a :2x` errors but `:a` stays
  defined (`src/evaluator.rs:479-543`). The REPL is transactional only
  because it clones first. Fix: validate everything, then insert.
- Ghost annotations are silently ignored: `[ -- Num ] @ghost` with no
  `:ghost` definition passes `check` (exit 0). A typo'd `@name` silently
  unconstrains. Fix: reject an annotation whose definition is absent.
- Duplicate annotations are first-wins silently:
  `[ -- Num ] @foo [ -- Bool ] @foo` passes; `:name` redefinition is a hard
  error. Fix: make duplicate `@name` an error, matching `:name`.

### Builtin contracts overpromise (green gate ≠ crash-free)

- Bitwise ops carry `( Num Num -- Num )` contracts but reject fractional
  values at runtime: `[ 1 0.5 | ] :main` passes `caternary check`, then fails
  with ``expected integer value, found `0.5` ``. Consequence of ratified
  decision (a) (one `Num`, no Int/Float split), but currently undischarged:
  nothing in Tier 1 demands "integer-valued" for `|`/`&`/`^`/`<<`/`>>`/`~`.
- Fix direction: either attach Tier-1 refinements demanding integrality
  (needs floor/frac predicates) or document explicitly that gate-green
  programs can still fail at runtime on value domains.

## Low / polish

- Refinement signatures are Rust-API-only (`attach_refinement`); the binary
  never parses them, so through `caternary check` Tier 1 is exercisable only
  via `assume(...)` and the four builtin arith axioms. If the surface is meant
  to be user-writable, add a source channel (and teach `check_command`).
- The refinement lexer accepts a trailing-dot literal (`1.`);
  `render_smtlib` emits it verbatim — invalid SMT-LIB for the z3 backend
  (its `from_string` error path is unverified). Conversely `1e3` is
  opaque/`Unknown` to the embedded reasoner but decidable by z3 — a parity
  break despite the M13 "bit-for-bit aligned" claim.
- Refinement binder type names are unvalidated (`n: Banana` parses and is
  treated as a `Real`); only `Quote` is load-bearing.
- `SmtLibSolver::pop_scope` / `Z3Solver::pop_scope` guard base-scope
  underflow only with `debug_assert` (release: silent base pop, later
  `unwrap` panic).
- `Parser::finish` reports the *innermost* unmatched `[`, not the outermost.

## Fixed-or-pinned elsewhere

- Hole 1 (reasoner i128 overflow): pinned by
  `solver::tests::check_sat_*_chain_with_large_coefficients_*` and
  `check::tests::gate_rejects_opaque_divisor_despite_large_assume_coefficients`.
- Hole 2 (definition-shadows-builtin desync, both tiers): pinned by
  `check::tests::definition_shadowing_core_word_*` and
  `check::tests::gate_accepts_definition_shadowing_core_word`.
- Hole 4 (spurious cyclic-type rejection via `DUP`/`DIP`/`CALL`; the
  `shadow_conforms_under_combinator_pressure` flake): pinned by
  `check::tests::dup_dip_rot_drop_call_does_not_spuriously_cycle`.
  When fixed, also re-run the shadow-conformance proptest repeatedly.
- `assume(...)` runtime no-op: pinned by
  `evaluator::tests::assume_word_is_a_runtime_no_op` and
  `check::tests::gate_passing_assume_program_runtime_matches_proven_effect`.
- Runtime `Num` semantics decided and recorded (`builtins::Num`): exact
  `i128` integers with checked overflow; finite `f64` as the documented
  escape hatch for fractional lexemes; `inf`/`NaN` rejected by the literal
  grammar and the runtime; `=`/`==`/`!=` numeric on numeric operands.
  Pinned by `builtins::tests::integer_arithmetic_is_exact_beyond_f64_precision`,
  `…::equality_is_numeric_not_rendering_dependent`, and neighbors. Remaining
  documented gap: fractional arithmetic rounds (f64) while the model's
  division is exact rational.
- Attestation hash covers Tier-1 refinement signatures: pinned by
  `attestation::tests::attestation_hash_covers_refinement_signatures`.
- Parser literal brackets + nesting cap: pinned by
  `parser::tests::{double,single}_quoted_brackets_stay_literal`,
  `parser::tests::backslash_escaped_brackets_stay_literal`, and
  `parser::tests::nesting_beyond_the_cap_is_a_clean_error`.
