# Caternary TODO — known holes

Found by code review (repros verified against the CLI/library as noted).
Holes 1, 2, and 4 are fixed and pinned by regression tests in
`src/solver.rs` and `src/check.rs` (grep for "Regression:"; see the
"Fixed-or-pinned elsewhere" section); the rest are tracked below.

## High

### Higher-order definitions fail closed: applying a quotation parameter cannot pass the gate (BUGS §B1)

Tier 1 seeds a definition's inputs as opaque *terms* (`seed_opaque_inputs`),
so a body that `CALL`s/`DIP`s a quotation **parameter** is a hard shadow error
("expected a quotation, found a value term") — `[ CALL ] :apply`,
`[ >q q CALL ] :apply`, the `@name`-annotated rank-2 pattern, and callers of
quotation-*returning* definitions are all rejected while the runtime runs
them. (A quotation bound from a **literal in the same body** works:
`[ 5 [ 1 + ] >f f CALL DROP ] :main` verifies — the §B2 locals fix.)

The direction is safe (pure over-rejection), and — important — it currently
**masks a soundness hole**: nothing verifies a literal quotation argument's
body at a call site (`[ 1 0 / DROP ]` passed to `apply` is consumed opaquely;
its division is only checked because `apply` itself CALLs it — which is
exactly the part that fails closed today). A fix that merely lets the
definition-side CALL move data opaquely would open that hole gate-wide.

The sound fix is the **definition side of §10.6** (higher-order contracts at
both ends of the boundary):

1. A definition whose signature declares a quotation binder
   `q: ( pre -- post )` seeds that input as a **contract-carrying symbolic
   quotation**: running it inside the body discharges `pre` as an obligation
   (bound at the invocation point), moves data per the contract's binder
   counts, and asserts `post` on the fresh outputs.
2. At each call site, the provided quotation must satisfy the expected
   contract. Relay handles `[ w ]` (already implemented,
   `relay_provided_contract`); a **literal body** must be *verified against
   the expected contract* right there (seed `pre`, verify the body, prove
   `post`) — and its possible reach past the contract's declared slots (the
   §A1b row-absorption problem) must be excluded or havocked.
3. Unrefined quotation parameters (including the `@name`-annotation-only
   rank-2 pattern) stay fail-closed: with no contract there is nothing sound
   to assume at the definition and nothing to check at the boundary.

Pinned (fail-closed today, and the same-body-local carve-out) by
`check::tests::quotation_parameter_application_fails_closed`.

## Medium

### BI@/TRI@ rank-1 schemes: one shared row, two applications (BUGS §A3/§B6)

`BI@ : ( 'S a a ('r a -- 'r b) -- 'S b b )` (and TRI@'s triple) reuse a single
row/element pair for every application of the quotation, but the two runtime
applications occur at *different* stack depths (the second sees the first's
result) — which no rank-1 arrow can state. Two directions of imprecision:

- **§A3 (unsound at Tier 0 only):** the shared row can absorb a tail element,
  so `1 2 [ SWAP ] BI@` types `( 'S -- 'S Num Num )` on the Tier-0-only
  surfaces (`check()`, `infer_quote_type`, REPL `:type`) while the runtime
  underflows. The **full gate is unaffected** — the Tier-1 shadow re-executes
  the real shuffle and rejects.
- **§B6 (over-rejection everywhere):** the same single row forces exactly
  `( a -- b )` per application, so runnable `[ DROP ]` / `[ DUP ]` bodies
  occurs-check out ("cyclic type").

Fixing either direction needs per-application instantiation of the quotation's
arrow (rank-2 style generalization of a *value*), which the rank-1 substitution
cannot express. Pinned by `check::tests::bi_at_rank1_scheme_limits_are_recorded`.

### API sharp edges

(none open)

### Builtin contracts overpromise (green gate ≠ crash-free) — documented

- Resolved by documentation (the ratified single-`Num` decision stands and
  the predicate language has no floor/frac): the gap is recorded on
  `register_scalar_builtins` and in the README's `caternary check` section.
  Embedders needing integrality in the gate should attest their own bitwise
  contracts. Revisit if the refinement language grows floor/frac predicates.

## Low / polish

(none open)



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
- `Evaluator::load` atomicity: pinned by
  `evaluator::tests::load_is_atomic_on_error`.
- Refinement-signature source channel (a quoted top-level word of the §10.1
  grammar, load-attached, definitions-only): pinned by
  `evaluator::tests::signature_word_attaches_a_refinement_at_load` and
  `check::tests::source_attached_signature_{discharges_through_the_gate,
  rejects_a_violating_body}`.
- Refinement binder type names validated (`Num`/`Bool`/`List`/quote arrow):
  pinned by `refinement::tests::unknown_binder_type_is_rejected`.
- Numeric-lexeme parity: the refinement lexer rejects trailing-dot literals,
  `Rat::parse` handles scientific notation exactly (capped exponent), and
  `render_smtlib` canonicalizes numerals into valid SMT-LIB. Pinned by
  `refinement::tests::trailing_dot_numeric_literal_is_rejected` and
  `solver::tests::scientific_notation_is_decidable_and_renders_canonically`.
- `pop_scope` base underflow is a hard assert in all build profiles: pinned
  by `solver::tests::smtlib_pop_scope_underflow_panics_with_its_name`.
- `Parser::finish` reports the outermost unmatched `[`: pinned by
  `parser::tests::unmatched_open_bracket_reports_the_outermost`.
- Ghost / duplicate `@name` annotations rejected at load: pinned by
  `evaluator::tests::ghost_annotation_is_rejected_at_load` and
  `evaluator::tests::duplicate_annotation_is_rejected_at_load`.
- Optimizer termination: iteration + program-size budgets, cycle detection
  by exact program equality. Pinned by
  `optimizer::tests::size_increasing_rule_terminates_at_the_size_cap` and
  neighbors.
- Attestation hash covers Tier-1 refinement signatures: pinned by
  `attestation::tests::attestation_hash_covers_refinement_signatures`.
- Parser literal brackets + nesting cap: pinned by
  `parser::tests::{double,single}_quoted_brackets_stay_literal`,
  `parser::tests::backslash_escaped_brackets_stay_literal`, and
  `parser::tests::nesting_beyond_the_cap_is_a_clean_error`.
