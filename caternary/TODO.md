# Caternary TODO — known holes

Found by code review (repros verified against the CLI/library as noted).
Holes 1, 2, and 4 are fixed and pinned by regression tests in
`src/solver.rs` and `src/check.rs` (grep for "Regression:"; see the
"Fixed-or-pinned elsewhere" section); the rest are tracked below.

## High

### Runtime `Num` is f64, but the solver models it as exact `Real`

- `src/solver.rs:342,488` declares every variable `Real` (QF_LRA);
  `src/builtins.rs:138` (`pop_num`) computes in f64.
- Proven refinements need not hold at runtime: `9007199254740993 1 +`
  evaluates to `9007199254740992`; `is_numeric_literal` (`src/types.rs:59`)
  accepts `inf`/`NaN` as literals, which have no Real semantics. Division in
  the model is exact rational division; the runtime rounds.
- `=`/`==`/`!=` compare **token renderings** (`to_tokens`,
  `src/builtins.rs:286-302`) while the shadow models `=` as real equality
  (`src/shadow.rs` `interpreted_op`). Rendering-dependent: in the README's
  i64-based `Value`, `1 1.0 =` is false; in the REPL's f64 `Value`, true.
- Fix direction: decide the semantics deliberately — e.g. runtime integers
  via i128 with f64 only as a documented escape hatch, plus a `Num` literal
  grammar that rejects `inf`/`NaN`; make `=` numeric (or add `eqv?`-style
  token equality under a separate name and keep `=` out of the solver's Eq).

## Medium

### Attestation hash omits Tier-1 content

- `ContractSet::attestation_hash` (`src/attestation.rs:222`) covers only
  Tier-0 schemes + the operator table — not refinement signatures
  (`Evaluator::attach_refinement`). Two builds differing only in refinement
  axioms hash identically, contrary to the architecture section's
  "all definition signatures + the operator table" (core entries are documented
  to carry "Tier-0 scheme (and Tier-1 axiom)").
- Fix direction: fold each name's `RefinementSig` (and the operator axioms)
  into the canonical rendering before hashing.

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
- Parser literal brackets + nesting cap: pinned by
  `parser::tests::{double,single}_quoted_brackets_stay_literal`,
  `parser::tests::backslash_escaped_brackets_stay_literal`, and
  `parser::tests::nesting_beyond_the_cap_is_a_clean_error`.
