# Caternary Type System — Implementation Spec (rev 4)

**Audience:** the implementing agent (Sid worker). You are working in the Caternary codebase on the latest `blue`. This document is the contract for what to build and how the judge will verify it. Read the whole thing before writing code. Builtin names in this document use the runtime **UPPER_SNAKE_CASE** spelling. Where this spec names a builtin or an internal symbol you cannot find, **locate the real one in the codebase and reconcile** — do not invent.

> **Ratified decisions (rev 4).** Argued and settled; do not relitigate without a recorded reason.
> (a) **One numeric type `Num`** — no Int/Float split, no overloading; specialization, if ever, is post-inference monomorphization.
> (b) **Flat substitution + trailed undo log** (Tier-0 inference backtracking only) for O(1)/zero-alloc — *not* a persistent HAMT, *not* union-find; **typing frames are the durable provenance spine** for anything the trail unwinds.
> (c) **`IF` is a combinator** with a hand-written scheme.
> (d) The **shadow stack is an abstract evaluator** mirroring runtime data flow exactly (`DUP` aliases the same term; combinators execute their real shuffle; **arity comes from the Tier 0 arrow**); core-shuffle conformance is property-tested because a mis-shuffle yields a *vacuous proof*, not a crash.
> (e) **Higher-order refinement subsumption is SMT-discharged only, never inferred**, and **fails closed**.
> (f) **Whole-program, no modules** — one compilation unit, one global ledger, no serialized contracts, no version skew.
> (g) **Embeddable; operators are contracted at registration** by two registrants (language core + embedder); the **user registers nothing**. Registration is **explicit attestation**; the operator table is the **modulo** of every proof. Operator contracts (axioms) and user `assume` are **separate buckets** — the strict anti-rot check applies to `assume` only, which is now **near-vestigial**.
> (h) **Tier seam is an immutability barrier** (Tier 1 treats the Tier 0 substitution read-only); **CI gate, free runtime** (`caternary check` pays all verification at build; a checked program links no solver).
> (i) **Origin spans + typing frames** are representation invariants present from the first inference commit.

---

## Architecture: whole-program, embeddable, operator-contracted (READ THIS FIRST)

This frames everything below. The trust story is sound **by construction**, not by protocol — because of three structural decisions.

### Whole-program, no modules
Caternary compiles as **a single unit, C-static-linker style**: every definition lives in one flat global namespace visible to every other (the order-independent global pre-pass — §6 — already gives this). **Separate the *program* from the *definitions*:** definitions are the `[ body ] :name` set; the *program* is the distinguished entry (`main`) whose effect must close against the **empty stack**. There are **no modules, no imports, no separate compilation, no serialized contracts, no version skew.**

Why this is the *correct* design, not a shortcut: every cross-module soundness hazard — a guarantee proven against a contract that later drifts, an assumption laundered into a theorem at a link step, an attestation detached from its axioms — exists **only because** of the time-and-trust gap between proving a module and consuming it elsewhere. Whole-program **deletes the gap.** Everything is proven in the same compilation, against the actual definitions present, under **one global ledger.** Ledger monotonicity is then free: it is one in-memory structure built by one pass, and `grep assume` over the source enumerates the *complete* user trusted base. This is static-vs-dynamic linking applied to contracts: give up separate compilation, get an artifact whose correctness is self-contained.

**Scale:** sound and fast for **tens of thousands of lines** — a known-good regime for whole-program analysis. Not for millions of lines of contract-dense code; that limit is accepted and stated.

**Recompilation is whole-program** — no incremental-verification story; touching one definition makes its SCC and everything downstream live again. Mitigations: **cache SMT discharge verdicts on a canonical content key for each obligation** (predicate + in-scope facts) so a warm compile pays the solver zero times for unchanged obligations; and content-hash the *whole-program* contract set (all definition signatures + the operator table) into **one artifact attestation hash** for build reproducibility and the CI gate. No `Cid`-per-module machinery — one program-level attestation, per-obligation exact keys for the cache.

### Embeddable: operators are contracted at registration
Caternary is **embedded** into a Rust host. Two populations, separated by a capability boundary that already exists:

- **The embedder** writes Rust, **registers operators**, and **provides each operator's type at registration.** Registration is the FFI boundary; the embedder is already trusted with the entire host process.
- **The user** writes Caternary against whatever operator set the embedder exposed. The user **cannot register operators, cannot write Rust, cannot reach the operator table.**

So there is **one trusted-contract mechanism — operator registration carries a type — with two registrants, and the user is neither:**

1. **The language core** registers the irreducible primitives (`CALL`, `DIP`, `IF` — the words with no Caternary body, because "apply a quotation" *is* the runtime operation that makes quotations mean anything). Each carries its **Tier 0 scheme** (§2) *and* its **Tier 1 refinement axiom** (§8 / §10.6).
2. **The embedder** registers host operators (syscalls, DB calls, whatever the host exposes), each with a type, at embed time.

The user's only unproven-fact mechanism is `assume` (§10.7). **Because every foreign thing arrives pre-contracted by the party who wrote it, the user inherits operator contracts automatically and the `assume` surface shrinks to near-zero** — `assume` survives only as a vestige for the rare local assertion about a *Caternary* construct the solver cannot reach.

### Registration is explicit attestation
Registering an operator with a contract is **visibly a trusted act**: `register_operator_with_contract(name, ty)` (or the real signature) is the embedder attesting *"I, who wrote this Rust, vouch that it satisfies this contract."* Caternary **cannot** check that the Rust actually satisfies it — that obligation lives in Rust-land, outside the language's reach (the FFI floor). Therefore:

- **The operator table is the modulo of every Caternary proof.** A verified program's honest status is *"proven sound, **modulo** the attested operator contracts."* The table — language-core entries plus embedder entries — is the exact, enumerable statement of that modulo. Different embeddings have different trusted bases: a sandbox embedding registers pure operators; a systems embedding registers syscalls. **The trusted base is the API surface the host chose to expose, attested.**
- **Two buckets, never merged.** *Operator contracts* — the **axiom** bucket: language-core + embedder registrations, attested, floor-level, enumerable as the registration table — vs. *user `assume`* — per-program, ledgered. **The strict drop-and-re-run anti-rot check (§10.7) applies to `assume` only.** Axioms are definitionally floor-level (you cannot prove a Rust function's behavior from within Caternary; the embedder asserting it *is* the floor), so "is this dischargeable without it?" is meaningless for them. A user **cannot** add to the axiom set — the wall is the embedding API, structural, not a verifier policy.

### Nothing is trusted silently
The closure property: **every trust transfer is explicit, at the point and by the party with standing.** The embedder attests operator contracts at registration; the user attests local assumptions via strict `assume`; the language attests its own primitive contracts as core registrations; everything else is *proven*. No implicit trust anywhere, and no party vouches for something they could not see.

### CI gate; checked programs run free
`caternary check` is the **CI gate**: it runs full Tier 0 + Tier 1 verification, **including discharge of every obligation against the operator-contract axioms.** A program that passes is *checked*. **A checked program carries zero verification machinery at runtime** — Z3 and the shadow stack are compile-time only (§10.10); there is nothing left to check when it runs, the way a borrow-checked binary carries no borrow checker. *"Running a checked program is free"* is the design goal, stated as such. Check in CI; run lean.

---

## 0. Goal, in one sentence

Give every Caternary word a **stack-effect type** `( before -- after )`; typing a program is **unifying** the `after` of each word with the `before` of the next. Everything below is consequence.

This is Hindley–Milner where the function arrow is the stack arrow, extended with **row variables** for the unobserved tail of the stack. Grounding: Kleffner's thesis (*A Foundation for Typed Concatenative Languages*) for the formal account; Kitten for an engineering reference. Do not deviate from those without a recorded reason.

---

## 1. Scope

### In scope
- **Tier 0 — shape safety:** row-polymorphic HM inference over word sequences, quotations, named locals (`>name`), top-level definitions (`[ body ] :name`), and **registered operators** (which contribute schemes to the environment). Catches underflow, arity errors, type mismatches. Build this first and completely.
- **Tier 1 — refinements:** predicate annotations in `where` clauses; a compile-time symbolic **shadow stack**; **path conditions** through control flow; **verification-condition (VC)** generation discharged by Z3; **higher-order subsumption** at combinator boundaries; **gradual** refined/unrefined interop; **operator refinement axioms** (language-core primitives + embedder operators). Gated behind Tier 0; do not begin until Tier 0 is green.

### Settled decisions / non-goals (do not build the banned forms)
- **One numeric type, `Num`.** There is no Int/Float distinction in Tier 0. `+`, `<`, etc. are single principal-typed words over `Num`. **No operator overloading.** If machine int vs float is ever needed, it is done as **monomorphization of a generic** — one source-level word, multiple instantiations chosen by the concrete type at the call site, resolved as a **lowering pass *after* inference**, never by branching in `unify_ty`. This keeps principal typing intact because inference always sees one type.
- **No *inferred* subtyping.** The type engine never solves inequalities; principal typing is preserved. The *one* admitted directional check is **refinement subsumption on quotation types**, discharged as an SMT implication, **never inferred** — see §10.6. Do not import value-language subtyping or HM(X)-style constraint solving anywhere else.
- **No modules / no separate compilation.** Whole-program, one ledger (architecture section). No serialized contracts, no `Cid`-per-module linking, no version skew.
- **No polymorphic recursion.** Undecidable (Henglein). A definition is monomorphic within its own recursive group — reject or require annotation (§6).
- **No Tier 2 (proof assistant) work.** Forward note only in §11.
- **No record/row-with-labels machinery.** Stack rows are *ordered and unlabeled*; the unifier is head-directed (§4).
- **No user-registered axioms.** Trusted contracts come *only* from operator registration, and only the language core and the embedder register. The user's sole trust tool is strict `assume`.

---

## 2. The type discipline (model you must hold)

- A **stack type** is an ordered list of element types, **top of stack on the right**, terminated by a **row variable** standing for "whatever was underneath, untouched."
- An **element type** is one of: a type variable, a base type (`Num`, `Bool`, …), or a **quotation type** `( S -- T )` (an arrow between two stack types).
- A **word's type** is itself an arrow `( S -- T )`.
- **Concatenation is composition.** `A B C` means: do `A`, feed its output stack into `B`, then into `C`. Typing the sequence = composing the arrows = unifying outputs against inputs.

Signatures (convention: `'R 'S 'T` are row variables, lowercase `a b` are element type variables, top on the right):

```
DUP   : ( 'S a        -- 'S a a )
DROP  : ( 'S a        -- 'S )
SWAP  : ( 'S a b       -- 'S b a )
CALL  : ( 'S ('S -- 'T) -- 'T )
DIP   : ( 'S a ('S -- 'T) -- 'T a )
IF    : ( 'S Bool ('S -- 'T) ('S -- 'T) -- 'T )
+     : ( 'S Num Num    -- 'S Num )
literal n  : ( 'S -- 'S Num )                       -- numeric literal (one numeric type)
literal [q]: ( 'S -- 'S (P -- Q) )  where q : (P -- Q)  -- quotation value
```

The row variable is what makes combinators expressible. You **cannot** state `DIP` or `IF` without quantifying over "the rest of the stack." `CALL`/`DIP`/`IF` are **registered by the language core** (architecture section) — they have no Caternary body; the schemes above are their authored Tier 0 contracts.

---

## 3. Type representation (Rust sketch — adapt to the real AST)

Wire this to the existing parser/AST; do not duplicate node types that already exist.

```rust
type TyVar  = u32;
type RowVar = u32;

#[derive(Clone)]
enum Ty {
    Var(TyVar),
    Con(String),                 // nullary base type: Num, Bool, ...
    App(String, Vec<Ty>),        // parameterized type, e.g. List a — add only when first needed
    Quote(Box<WordTy>),          // a quotation value carries an arrow (Tier 0: SHAPE ONLY)
}

#[derive(Clone)]
struct StackTy { elems: Vec<Ty>, row: RowVar }   // top of stack is the LAST elem
#[derive(Clone)]
struct WordTy  { input: StackTy, output: StackTy }
struct Scheme  { tyvars: Vec<TyVar>, rowvars: Vec<RowVar>, ty: WordTy }
```

Three representation invariants, all present **from the first inference commit** (they cannot be reconstructed later — same lesson as spans and the SCC pass):

1. **Flat substitution map + trailed undo log (NOT a persistent HAMT, NOT destructive union-find).** The Tier 0 type-variable substitution is a flat vector/arena mapping `TyVar -> Ty` and `RowVar -> StackTy`, with an **undo log** for any *inference-time backtracking* you need: `bind_ty`/`bind_row` push `(slot, old_value)`; a backtrack point records the trail length and unwinds to it. **O(1), zero-allocation mutation** — the systems-correct choice over an O(log n)-per-bind HAMT that thrashes the allocator in recursive inference. (If straight HM + SCC generalization never speculatively backtracks, this trail can be trivial or absent — it justifies itself by *inference backtracking alone*.)
   - **Critical seam separation — do NOT conflate two trails.** This Tier 0 substitution trail is **not** the Tier 1 logical-scope mechanism. Tier 1 is gated behind a green Tier 0: by the time the shadow evaluator and VC generator run, **inference is finished and the typed AST is frozen.** The `push_scope`/`pop_scope` of path conditions (§10.4/§10.8) belong **entirely to the Z3 context trait and the shadow evaluator's own state** — they never touch the Tier 0 substitution. **Tier 1 treats the Tier 0 substitution map as read-only.** Enforce this as an **immutability barrier** at the tier boundary: a Tier 1 branch mutating a Tier 0 binding must be *impossible by construction*, not merely avoided.
   - **Consequence to accept:** a trail gives *undo*, not *queryable history*. After an unwind the bindings are gone, so an error whose two conflicting origins **straddle a scope boundary** cannot recover the exact unwound binding value. Acceptable because the durable provenance spine is the **frame stack (invariant 3)**, not the substitution: such an error degrades to *"bound inside a branch at L_n"* from the frame. **Trail = inference speed; frames = error history.** Errors raised *inside* a scope lose nothing.

2. **Origin span on every node.** Every `Ty`/`StackTy` carries the source span where it was *born* (a `Spanned<T>` wrapper or a parallel id→span table), propagated through unification. This is what lets §7 report a **provenance pair** instead of a failure site. **Not an M5 deliverable — a representation invariant.**

3. **Typing-frame stack — the durable provenance spine.** Descending into a quotation `[ … ]` pushes a frame recording its span and expected effect at entry. The frame stack is the compile-time backtrace (§7) **and** the fallback for any provenance the trail has unwound (invariant 1). Frames persist across `pop_scope`; they exist from the first inference commit.

**Tier 1 payload (built later, but reserve the shape now):** a `Quote` additionally carries optional `pre`/`post` **refinement** payloads (predicate ASTs over named binders, SMT-targeted). **Tier 0 never reads these** — it forwards them untouched and unifies on shape alone. They wake up only at the VC boundary (§10).

---

## 4. The unifier (head-directed; this is the whole engine)

### `unify_stack(a, b)`
Peel matching elements from the **top**, then let a row variable absorb the remainder.

```
unify_stack(a, b):
    while a.elems nonempty and b.elems nonempty:
        unify_ty(a.elems.pop_last(), b.elems.pop_last())
        a, b = apply_subst(a), apply_subst(b)
    if a.elems empty and b.elems empty:  unify_row(a.row, b.row)
    elif a.elems empty:  bind_row(a.row, StackTy { elems: b.elems, row: b.row })
    else:                bind_row(b.row, StackTy { elems: a.elems, row: a.row })
```

### `unify_ty(x, y)`
Standard: `Var` binds (via `bind_ty`); `Con(n)`/`Con(m)` iff `n==m`; `App` iff same head, same arity, unify pairwise; `Quote(p)`/`Quote(q)` unifies `p.input~q.input` **and** `p.output~q.output` component-wise (do not generalize here); else type-mismatch diagnostic (§7).

### Occurs check — **canonicalize, then check, uniformly through arrows**
Both `bind_ty` and `bind_row` must: **(1) canonicalize both sides to their representatives via the substitution map** (chase any existing bindings — a row var may already point at another row or a concrete stack), **(2) run the occurs check**, **(3) on a positive occurrence emit a typed cyclic-type diagnostic — never recurse into a stack overflow**, **(4) otherwise write the binding.**

The occurs check is **uniform** — it recurses into both stack types of every `Quote`. Do not skip arrow interiors.

```
occurs(v, term):
    term = canonicalize(term)
    match term:
        Var(u)     => u == v
        Con(_)     => false
        App(_, xs) => any occurs(v, x)
        Quote(w)   => occurs_stack(v, w.input) or occurs_stack(v, w.output)
occurs_stack(v, s):
    s = canonicalize(s)
    (s.row is v) or any occurs(v, e) for e in s.elems
```

Implement unification and `occurs` with an explicit **depth bound or worklist** so a malicious/accidental cyclic program (e.g. `[ DUP CALL ] DUP CALL`, which forces `a ~ ( 'X a -- 'Y )`) is **rejected with a cyclic-type error, not a compiler stack overflow.** That robustness is part of correctness here.

That is the entire unification machinery. No labels, no field reordering, no record solving.

---

## 5. Inference over a sequence

Accumulated effect is built by composition; identity is `( 'a -- 'a )`.

```
infer_seq(words, env):
    acc = WordTy { input: ('a), output: ('a) }   # fresh shared row 'a
    for w in words:
        (c, d) = instantiate(lookup(w, env))      # fresh vars for the word's scheme
        unify_stack(acc.output, c)
        acc = apply_subst(WordTy { input: acc.input, output: d })
    return acc
```

`lookup` resolves a word to its scheme — whether a Caternary definition or a **registered operator** (both contribute schemes to `env`; operators' schemes are authored at registration).

- **Numeric literal** `n`: `( 'a -- 'a Num )`. One numeric type (§1).
- **Quotation literal** `[ q ]`: infer `q` to get `(P -- Q)`; literal’s word type is `( 'a -- 'a Quote(P -- Q) )`. Quotations are syntactic values, so the `Quote(...)` **may be generalized** at a binding site.
- **`IF`** is a **primitive combinator** with the hand-written scheme in §2 — `( 'S Bool ('S -- 'T) ('S -- 'T) -- 'T )`. Branch agreement falls out of unifying both branch quotations against the shared `( 'S -- 'T )`. Do **not** special-case `IF` beyond supplying its scheme.

### Named locals (`>name`)
`>name` pops the top and binds it in a **local environment** for the remainder of the scope.
- `>name : ( 'a t -- 'a )`, introducing `name : t` (fresh `t`, unified at the pop site).
- Each use `name : ( 'a -- 'a t )` with the *same* `t`.
- **Locals are monomorphic** within their scope (lambda-like, not generalized).

---

## 6. Generalization via SCC analysis (top-level definitions)

The global pre-pass already collects all `[ body ] :name` definitions **order-independently** and has the symbol table. Use it. (This same flat namespace is the whole-program compilation unit — architecture section.)

1. Build the **call graph** (edge `A → B` when `A` references `B`).
2. **Tarjan SCC** to find mutually-recursive groups.
3. Process SCCs in **reverse-topological order** (dependencies first).
4. Within an SCC: assign each member a fresh **monomorphic** arrow type variable, infer all bodies under those assumptions, unify. **Recursive calls inside the SCC use the un-generalized type** (this is the no-polymorphic-recursion rule).
5. Moving to dependents: **generalize** each member over variables **not free in the enclosing environment** (context-aware generalization) → each member's `Scheme`.

**Polymorphic recursion:** if a body requires calling itself at a type that does not unify with its monomorphic assumption, do **not** silently widen. Emit *"`name` appears to use polymorphic recursion, which cannot be inferred; add a type annotation."* (Clean rejection is acceptable for Tier 0; the annotation-checking path may follow later.)

---

## 7. Error reporting (representation-backed; the real risk)

Composition means a mistake on line 2 can surface as a unification failure "near" line 40. Untreated, this makes the checker unusable. Three layered requirements, in payoff order, **all backed by the §3 data structures that exist from commit one:**

1. **Provenance pair, not failure site.** On `unify_ty` mismatch you hold two origin spans: where each conflicting type was born. Report both — *"this is `Num` because of `+` at L3; it must be `Bool` because `IF` at L7 demands it"* — not merely the enclosing word where unification blew up.
2. **Quotation boundaries are frames.** Render the typing-frame stack as a breadcrumb: *"in the quotation at L7 ▸ which `IF` expects effect `( 'S -- 'S )` ▸ `foo` breaks it by producing `Bool`."* Nesting becomes a backtrace, not one opaque site.
3. **Localize inward.** Anchor at the **innermost** frame where both conflicting types are already concrete — that is almost always the user's actual mistake; the outer row-unification failure is the consequence. Walk inward to the deepest frame still carrying the contradiction.

**Never** leak internal variable names (`'t7`, `'r3`) to the user; debug-only, behind a flag. The judge rejects any user-facing diagnostic that leaks them or omits a span.

---

## 8. Combinators and the rank boundary

- **Primitive combinators are language-core operator registrations** (architecture section). `DIP`, `CALL`, `IF` have no Caternary body — the language core registers them, each carrying its hand-written **Tier 0 scheme** (§2) *and*, at Tier 1, a hand-written **refinement axiom** (§10.6). They live in the operator-contract (axiom) bucket, authored once, invisible to the user — **not** in the `assume` ledger. Combinators written *in* Caternary (`KEEP`, `BI`, derived shuffles) are **transparent definitions**, inferred and verified through their bodies, with no authored contract.
- **`DIP`/combinator rule — relay, never expand.** A combinator's contract is written using only the **declared** arrow of its quotation argument; never expand the quotation's body into the caller's contract. Categorically, `DIP q = q ⊗ id_a`. The only fact `DIP` itself contributes is that the set-aside value returns unchanged. (Its Tier 1 refinement axiom in §10.6 is exactly this rule lifted to predicates.)
- **Rank-1 is inferred. Rank-2 requires annotation.** The one non-inferable case: a user-defined word that applies its quotation argument at **two genuinely distinct types** in one definition. Detect this and emit *"this word applies its quotation at more than one type; add a type annotation."* With an annotation, check against it.

---

## 9. (reserved — Tier 1 begins at §10)

Tier 1 is large enough to own its own top-level section; see §10. This placeholder keeps the Tier 0 sections (§2–§8) and Tier 1 (§10) clearly separated.

---

## 10. Tier 1 — refinements, shadow stack, VCs, subsumption (PHASE 2; do not start until Tier 0 is green)

### 10.1 Refinement syntax
Names + predicates hang on the shape; drop the predicates and you are back to Tier 0.

```
sqrt : ( n: Num where n >= 0  --  r: Num where r >= 0 and r * r = n )
push : ( xs: List  n: Num  --  ys: List where length ys = length xs + 1 )
```

The **`where` language is an ordinary infix predicate language** over the named binders (arithmetic, comparison, boolean connectives, uninterpreted functions like `length`) targeting SMT. It is **not** Caternary's postfix surface syntax — keep the two parsers separate. Input predicates are **demands** (obligations on the caller); output predicates are **guarantees** (gifts to the next word).

### 10.2 Positional → named binding (the zip)
Tier 0 is positional and anonymous; SMT needs names. At a call boundary, zip the callee's named parameters against the inferred `StackTy.elems` **right-to-left from the top of stack** (the only pinned end). For `push : ( xs n -- … )` against inferred `'S Num Num`: `n ← top`, `xs ← second`. **The row tail `'S` stays anonymous and is never named into a VC** — the refinement language has no name for the unobserved tail. Naming it produces unbound SMT variables.

### 10.3 Symbolic shadow stack (compile-time only) — a second evaluator
The shadow stack is an **abstract interpreter over the *same* operational semantics as the runtime**, whose values are **symbolic terms** in the refinement logic instead of runtime values. Every word that moves data at runtime moves terms here with **byte-identical data flow.** This single mandate subsumes three requirements people get wrong:

- **Aliasing under `DUP`/shuffles.** `DUP` pushes the **exact same term** (same identity), not a second fresh literal; `SWAP` exchanges terms; `DROP` discards one. If `DUP` minted two fresh terms, the solver would lose the aliasing fact and trivial proofs like `x x - = 0` would fail to discharge. "`DUP` copies the term" is just the `n=1` case of "shadow execution mirrors real execution."
- **Combinator data flow.** Tier 0 *types* `DIP` via its declared signature, but the shadow stack must **execute `DIP`'s shuffle at compile time**: pop the set-aside term, run the quotation's shadow effects on the rest, restore the term. The shadow stack must mirror the exact data flow of every core combinator, or the positional binding (§10.2) zips the wrong SMT names to the wrong slots after a non-trivial shuffle.
- **Arity comes from the Tier 0 arrow — the shadow evaluator owns NO independent notion of it.** For *every* word it executes, including opaque ones, the shadow evaluator reads that word's **already-inferred, already-generalized Tier 0 type** to know exactly how many terms to pop and push. An opaque `lib : ( Num Num -- Num )` pops two terms and pushes **one** fresh literal — the shape is known (Tier 0 proved it) even though the *meaning* is not. Opacity is only about whether the pushed term carries a predicate, **never** about how many terms move. **Tier 0's shape closure is the structural rail the shadow stack runs on** — which is *why* Tier 1 is gated behind a green Tier 0: the shadow evaluator's soundness is parasitic on Tier 0 having already balanced every arity.

Each slot carries the term describing its value; after `x 0 >` the top slot's term is the proposition `x > 0` (types alone say only `Bool`). When a value's producing term is opaque (returned from an uninterpreted word), its slot carries a **fresh literal**: soundness holds, precision degrades gracefully — but see §10.7 for what happens when that opaque value flows into an obligation.

**Conformance is a soundness requirement, not hygiene.** A mis-shuffled shadow stack does not crash — it emits a *well-formed VC about the wrong variables*, which Z3 discharges or refutes, producing a **green checkmark that proves nothing (vacuous verification)** — the worst failure class in the system. Therefore: **shadow data flow for the core shuffles (`DUP SWAP DROP DIP` and friends) is property-tested against the interpreter's data flow, not eyeballed.** The `DUP`-aliasing test (`x x -` discharges to `0`) and a post-shuffle zip test (§10.2 binds the right names after `DIP`) are the guards.

**The shadow stack does not exist at runtime** — a compile-time analysis artifact discarded before the binary ships, like a dataflow lattice.

### 10.4 Path conditions
VC generation under control flow must assert branch facts. For `cond [ then ] [ else ] IF`: recover `cond`'s proposition `P` from the shadow stack; verify `[ then ]` with `P` asserted into the solver scope and `[ else ]` with `¬P`. This requires the solver seam to support **scoped** assertions — `push_scope` before a branch, `pop_scope` after (§10.8). The shadow stack (§10.3) and positional binding (§10.2) are the substrate; this is two scoped assertions on top. (These scopes live in the Z3 trait + shadow-evaluator state only — never the Tier 0 substitution; §3 invariant 1.)

### 10.5 VC = verification condition
A VC is a pure logical formula whose validity means an obligation holds — no program left, just math. Generate one at **each call site** for each `where` clause. At `x sqrt`, the VC is `(facts known about x) ⟹ (x >= 0)`. **Encoding (the thing people get wrong):** to check validity, assert the hypotheses and the **negation** of the goal, then `check()`. `Unsat` ⇒ no counterexample ⇒ **valid**. `Sat` ⇒ pull the model; it **is** your counterexample, surface it. `Unknown` ⇒ degrade (and fail closed for subsumption, §10.6).

### 10.6 Higher-order refinement subsumption (the centerpiece)
Refinements are part of the Tier 1 type, attached to `Quote` as a **forwarded payload Tier 0 never reads** (§3). When a refined quotation crosses a higher-order boundary (passed to a combinator expecting a refined signature), the contract must be checked for **preservation**. This is **subsumption**, and it is directional:

- **Guarantees (postconditions) covariant:** `provided_post ⟹ expected_post`. A stronger guarantee substitutes for a weaker one.
- **Demands (preconditions) contravariant:** `expected_pre ⟹ provided_pre`. A weaker demand substitutes for a stronger one. (Note the **flipped** direction — that is the contravariance.)

This is subtyping on the arrow — the thing banned for *inference* in the value language. Resolution: **subsumption is emitted as these two SMT implications and discharged by Z3, never inferred by the type engine.** The checker only *generates* the implications; the solver settles them. Subtyping-the-inference stays banned (principal typing intact, §1); subtyping-the-obligation lives entirely in the SMT seam. Control plane (inference) stays unification-only and decidable; the directional reasoning is pushed to the data plane (the solver you already pay for).

**Operator refinement axioms.** The primitive combinators (`DIP`, `CALL`, `IF`) are language-core registrations (§8) and carry **authored refinement axioms** here — the Tier 1 counterpart to their §2 schemes. `DIP`'s axiom is the relay rule made predicate-level: the set-aside value's predicate is preserved unchanged **and** the quotation's `post` holds on the rest (`DIP q = q ⊗ id_a`, lifted to predicates). Without these axioms, a refined quotation passing through `DIP` would have no axiom connecting its guarantee to the post-`DIP` stack, and the contract would evaporate exactly as at an unaxiomatized foreign call. Embedder operators carry their own `pre`/`post` from registration; the user's proofs discharge against those, on faith (the operator table is the modulo — architecture section).

**Undecidable subsumption fails closed.** If Z3 returns `Unknown` on a subsumption VC, **reject**: *"could not prove this contract is preserved across the combinator; annotate or simplify the predicate."* Never a silent pass. A refinement that fails open is worse than no refinement.

### 10.7 Gradual adoption + the opaque-dependency boundary
Refined and unrefined quotations interoperate. **An absent refinement means `where true`** — maximally weak guarantee and demand — so unrefined code slots into the lattice automatically and is adoptable incrementally. One sharp edge: an unrefined quotation passed where a *guarantee* is required fails its subsumption VC (`true ⟹ r > 0` is invalid). Detect the absent-payload case and emit the targeted diagnostic *"this quotation carries no contract and one is required here,"* **not** a bare SMT counterexample.

**Where opaque words come from (read with the architecture section).** In the whole-program/embedded model, every foreign word the user can call is a **registered operator carrying an embedder-attested contract** — so its `pre`/`post` are already present and the user inherits them automatically. The user therefore **rarely** writes `assume`: the foreign frontier is the operator table, contracted at registration, **not** a per-call user burden. `assume` survives as a **near-vestigial** residual for the rare case a user must locally assert something about a *Caternary* construct the solver cannot reach. It remains strict. The mechanics below define it precisely.

**The case it addresses: a refined obligation depends on an uncontracted thing.** A word `foo` guaranteeing `r > 0` depends on a value the solver knows only `true` about, and that value flows into `foo`'s guarantee. The VC becomes `true ⟹ r > 0`, invalid, and `foo` fails to verify. Two situations must be distinguished:

- **Situation A — the unknown value never reaches an obligation** (it is dropped, logged, or passed to another unrefined sink). `where true` is **correct and silent**: no VC depends on it, the user writes nothing. The common case; it costs zero.
- **Situation B — the unknown value flows into a demand or guarantee.** The default is **fail-closed** (verification failure). Fail-*open* (silently treating `true` as "trust me") is **categorically rejected** — vacuous verification, the §10.3 green-checkmark-proving-nothing failure.

The resolution for Situation B — an **assumed-contract boundary** (`assume`):

> The user attaches an **`assume`** clause at the boundary — a refinement **taken as true without proof**, **recorded as an explicit trust assumption in the verification ledger**, and discharged-on-faith. `assume( result > 0 )` lets `foo`'s guarantee go through; `foo`'s VC is now *conditional on a named assumption*, and the assumption is **surfaced**, not hidden. The proof chain is not poisoned and not broken — it is **explicitly rooted in a stated axiom**, and `grep assume` enumerates the entire user trusted base.

Three properties keep it sound rather than a disguised fail-open:

1. **Assumptions are first-class and enumerable.** Each `assume` lands in the ledger; the honest status is *"`foo` verified modulo { … }."* When the dependency later gets a real contract, the `assume` becomes a **checked subsumption** and the user deletes it.
2. **Assumptions propagate upward, but visibly.** `foo`'s callers inherit "proven modulo these assumptions" — the trust root propagates as **named ledger entries**, so a guarantee never silently becomes weaker than it reads. (The monotonic-ledger property from the ralph judge, reused.)
3. **The default stays fail-closed; `assume` is the deliberate opt-in.** Without `assume`, the obligation still fails (soundness preserved). The user reaches for it exactly when the trust is warranted — one local annotation that *documents a real trust decision*. **Wrapping hides the trust in a shell; `assume` hoists it into the audit log.**

**STRICT legality (ratified — anti-rot):** `assume` is legal **only** where a genuinely opaque/uncontracted dependency sits in the obligation's chain. It is a **hard error** to `assume` a goal the solver could discharge, or one with no opaque dependency. The ledger must mean *"things we honestly cannot prove,"* never *"things we couldn't be bothered to."* Enforcement: an `assume` whose obligation is dischargeable without it (drop the assumption, re-run the VC, solver returns `Unsat`) is rejected as *"this obligation is provable; remove the unnecessary `assume`."*

**Double-check cost — and why it stays cheap.** Strict legality requires solving **twice** at an `assume` boundary: once *without* the assumption (the exploratory check — is the obligation dischargeable alone?) and once *with* it (the real obligation). Two mitigations keep this from hanging a contract-dense compile:
- **The exploratory verdict is cacheable.** It is a property of the obligation, stable while the obligation and its in-scope facts are unchanged — memoize on the obligation's canonical content key (the same per-obligation cache as the whole-program reverify story, architecture section). A warm compile pays it zero times.
- **The exploratory solve is abortable and fails *lenient*.** Give it a tight timeout. Strictness only *rejects* when it can **positively** show the assumption was unnecessary (`Unsat` without it); a timeout (`Unknown`) is **not** a positive showing, so it **accepts** the `assume`. Thus the expensive-and-must-be-correct solve is the *second* one (the obligation under the assumption); the first runs on a short leash and degrades to allow. The exploratory check is never on the program's critical proof path.

### 10.8 Z3 integration + scoped seam
Use the **`z3` crate**, bundled feature (avoid system-library path fights); raw `z3-sys` only if the wrapper is missing something (it will not be). The seam is a **trait with four methods**: `assert`, `check`, **`push_scope`, `pop_scope`** — scoping is required by path conditions (§10.4) and **must exist before M8, not retrofitted.** One long-lived `Context` per checking session; keep facts ground where possible (`Unknown` comes from quantifiers/nonlinear; lists via datatypes/arrays only when unavoidable).

### 10.9 Solver-agnostic seam
The VC generator emits **SMT-LIB2 text** through the trait; the Z3 call sits behind it; **the generator core never calls the `z3` crate directly.** SMT-LIB emission mode must emit `(push 1)` / `(pop 1)` matching `push_scope`/`pop_scope` to stay at parity. This is the seam where CVC5 or Why3 slots in later. Provide the text-emission mode from day one of Phase 2 — it is your debugging window.

### 10.10 Everything in Tier 1 is compile-time — and the CI gate
**Z3 is not linked into the Caternary runtime and is never called during execution.** By the time a program runs, every contract is already proven and there is nothing left to check — this is build-time verification, like a borrow-check, not a runtime assert. The shadow stack (§10.3) is compile-time only. **No refinement machinery has any runtime cost.** This is what makes the CI-gate model work: **`caternary check`** pays the entire verification cost — Tier 0 + Tier 1 + operator-axiom discharge — at build time, so a *checked* program ships with **no solver, no shadow stack, no contract residue.** Check in CI; run lean.

### 10.11 Annotation burden (what the user actually writes)
Tier 0 infers all shapes; **zero annotations** for shape safety. Refinements are **opt-in per word** — write a `where` clause only on the words whose contracts you want proven. **Refinement is viral downward through demands but inferred upward through composition:** annotate a constraint at its *source* and the obligation propagates to call sites automatically; you never restate it on intermediate words. The total annotation surface is exactly: (a) words you deliberately contract, (b) the rank-2 case (§8), (c) the boundary where an unrefined quotation meets a refined expectation (§10.7), and (d) the near-vestigial `assume`. Foreign words are pre-contracted by the embedder at registration, so they cost the user **nothing**. There is **no** regime requiring annotation on ordinary `DUP`/`SWAP`/straight-line code.

---

## 11. Tier 2 (deferred — note only)

For a future crown-jewel routine, shallow-embed words as relations on stacks (concatenation = relational composition `∘`). **Evaluate Why3 first**: it dispatches VCs to multiple SMT backends and drops to Coq/Lean only for the residue — structurally the Tier 1 → Tier 2 escalation, already built. Emitting WhyML instead of raw SMT-LIB may mean never hand-writing a Rust↔Lean bridge. Note for later, not a task.

---

## 12. Milestones and acceptance tests

Each milestone is independently verifiable. Green = complete. Caternary snippets use runtime builtin names such as `DUP DROP SWAP + DIP CALL IF`. Initial stack for a top-level program is **empty** unless stated.

### Tier 0

**M0 — Whole-program driver + operator registration (shape).** Definitions load into one flat global namespace (the pre-pass); a `main` entry is type-checked against the empty stack. A registered operator with a Tier 0 scheme is usable in inference. **Registration requires explicit attestation** (the API is visibly trusted). **The user cannot register operators** (no Caternary surface reaches the registration path) — assert this is structurally impossible, not merely undocumented.

**M1 — Unifier**
- Peeling: `( 'a Num -- )` vs `( 'b Num -- )` unifies (`'a ~ 'b`).
- Row absorption: `( 'a -- )` unifies with `( 'b Num Bool -- )` by `'a := 'b Num Bool`.
- **Transitive canonicalization:** if `'r1` is already bound to `'r2` and `'r2` to a concrete stack, a further unification still canonicalizes correctly before the occurs check — no false cyclic rejection.
- Occurs check rejects `'a := 'a Num`.
- **Cyclic via arrows, no overflow:** `[ DUP CALL ] DUP CALL` is **rejected with a typed cyclic-type error, without stack-overflowing the compiler** (assert a clean rejection, not a crash). Occurs recurses into `Quote` interiors.

**M2 — Sequence inference**
- `1 2 +` infers `( 'a -- 'a Num )`.
- `DUP` infers `( 'a b -- 'a b b )`.
- `[ 1 + ]` infers `( 'a -- 'a (`'b Num -- 'b Num`) )`.
- `[ 1 + ] CALL` infers `( 'a Num -- 'a Num )`.
- `true [ 1 ] [ 2 ] IF` type-checks; both branches unify to `( 'a -- 'a Num )`.
- **Underflow:** top-level `DROP` (empty initial stack) is **rejected** with an arity error naming `DROP`.

**M3 — Definitions / SCC**
- Order independence: `[ bar 1 + ] :foo` before `[ 2 ] :bar` type-checks.
- Mutual recursion `[ … odd … ] :even` / `[ … even … ] :odd` type-checks (one SCC, monomorphic).
- A non-recursive helper used at two distinct types from two callers type-checks at both.
- Polymorphic recursion (self-call at a different type) yields the §6 diagnostic, not a raw unification error.

**M4 — Combinators**
- `xs total [ 99 push ] DIP` type-checks; result shape `… List Num`; `DIP` contributes only that `total` is carried through.
- Rank-2 without annotation → §8 diagnostic; with a correct annotation → checks.

**M5 — Error quality**
- A type mismatch reports the **provenance pair** (both origin spans) and the correct innermost frame; no internal variable names.
- An arity/underflow message names the offending word and gives counts.
- A mismatch inside a nested quotation renders the **frame breadcrumb** (§7.2) and anchors at the innermost concrete-contradiction frame (§7.3).

**Tier 0 done when:** M0–M5 green, `cargo test` clean, **no new clippy warnings**, public entry point wired into the existing pipeline.

### Tier 1 (Phase 2; gated)

**M6 — Refinement parsing.** `sqrt`/`push` (§10.1, `Num`) parse into demand/guarantee predicates over named binders; malformed `where` → located parse error.

**M7 — Positional binding + shadow stack (conformance is the point).** `push : ( xs n -- … )` against inferred `'S Num Num` binds `n←top`, `xs←second`; `'S` never appears as an SMT variable. After `x 0 >`, the shadow stack reports the top term as `x > 0`; an opaque producer yields a fresh literal (sound, imprecise). **Arity-from-Tier-0:** an opaque `lib : ( Num Num -- Num )` pops two terms and pushes exactly one fresh literal (the shadow evaluator reads the inferred arrow, owns no independent arity). **Aliasing:** `x DUP -` discharges to `0` (two slots after `DUP` carry the *same* term by identity). **Shuffle conformance:** after a non-trivial `DIP` shuffle, §10.2 binds the right SMT names to the right slots — property-tested against the interpreter's data flow, so a mis-shuffle is caught as a divergence, not silently shipped as a vacuous proof.

**M8 — Path conditions.** `x 0 > [ x sqrt ] [ 0 ] IF`: inside the true branch `x > 0` is in scope, so `x sqrt`'s demand `x >= 0` discharges (`Unsat`); **without** the path condition it would fail. The solver receives `push_scope`/`pop_scope` (and `(push 1)`/`(pop 1)` in text mode) around the branches. **Seam:** assert the scope state lives in the Z3 trait, not the Tier 0 substitution (the frozen AST is untouched).

**M9 — First-order VC generation + Z3.** `x sqrt` where facts do not imply `x >= 0` → VC `Sat`, counterexample surfaced; where they do → `Unsat`, accepted. Negated-goal encoding verified at unit level (known-valid ⇒ `Unsat`; known-invalid ⇒ `Sat` + model).

**M10 — Higher-order subsumption + operator axioms.** A quotation with `post: r > 5` passed where `post: r > 0` is expected → covariant VC `r>5 ⟹ r>0` valid, accepted. With `post: r > 0` where `r > 5` expected → invalid, rejected. Contravariant demand case symmetric. A subsumption VC returning **`Unknown` is rejected (fails closed)**, not accepted. **Operator axiom:** a refined quotation (`post: r > 0`) passed through `DIP` preserves its guarantee on the far side via `DIP`'s authored refinement axiom (§10.6); an embedder operator's registered `post` is usable to discharge a downstream user obligation.

**M11 — Gradual interop.** An unrefined quotation passed where a guarantee is required → rejected with the **"carries no contract"** message (§10.7), not a raw counterexample. An unrefined quotation passed to a contract-agnostic combinator → accepted (`true ⟹ true`). Situation A: a refined word that calls an opaque word whose result is **dropped** verifies silently (no annotation).

**M12 — Assumed-contract boundary (`assume`), strict + cheap.** A refined `foo` (guarantee `r > 0`) depending on an opaque value flowing into the guarantee: **without** `assume`, fails closed; **with** `assume( result > 0 )`, verifies and the assumption appears in the ledger as *"`foo` modulo { result > 0 }."* Callers of `foo` inherit the modulo-assumption status. **STRICT enforcement (positive rejection test required):** `assume` on a goal the solver can discharge without it → **rejected** as *"this obligation is provable; remove the unnecessary `assume`"* (drop assumption, re-run VC, `Unsat` ⇒ reject — assert this path actually runs, not a no-op). `assume` with no opaque dependency → rejected. **Cheap:** the exploratory check is cached on the obligation's canonical content key and uses a fail-lenient timeout (`Unknown` ⇒ accept the `assume`, never reject).

**M13 — SMT-LIB emission parity.** Text mode (including `(push 1)`/`(pop 1)`) agrees with the native `z3`-crate result across M8–M12; the VC generator core contains **no** direct `z3`-crate calls.

**M14 — Whole-program ledger + attestation hash.** `grep assume` over a program enumerates its complete user trusted base (one global ledger; no per-module reconciliation). The operator table (language-core + embedder registrations) is the enumerable modulo. A **content-hash of the whole-program contract set** is produced and is stable across rebuilds of unchanged source; changing one obligation invalidates only its exact discharge-cache key (warm-compile reuse). **CI gate / free runtime:** assert a *checked* program links **no** Z3 symbols and carries **no** shadow-stack machinery — verification is build-time only.

---

## 13. Invariants the judge holds across all iterations (do not violate)

1. **No *inferred* subtyping** in the value language; inference is unification-only, no inequality solving. The sole directional check (refinement subsumption, §10.6) is **SMT-discharged only, never inferred** — the engine emits the two directional implications and nothing more.
2. **One numeric type `Num`**; no operator overloading. Any int/float specialization is a **post-inference monomorphization lowering**, never branching in `unify_ty`.
3. The unifier is **head-directed** with row-variable tails; **no** field/label record machinery.
4. The **occurs check canonicalizes both sides first, is uniform through `Quote` interiors, and never stack-overflows** (depth-bounded or worklist); cyclic ⇒ typed error.
5. **Flat substitution + trailed undo log** for the **Tier 0** type substitution (inference backtracking only); **no** persistent HAMT, **no** destructive union-find; O(1)/zero-alloc. This trail is **Tier-0-only and is NOT the Tier 1 logical-scope mechanism** (invariant 18). Provenance the trail has unwound degrades to the **frame stack**, not the substitution.
6. **Origin spans and typing frames exist from the first inference commit.** Frames are the durable provenance spine and persist across `pop_scope`. User-facing diagnostics use provenance pairs and frame breadcrumbs, anchor at the innermost concrete contradiction, and never leak internal variable names.
7. **The shadow stack is an abstract evaluator mirroring runtime data flow exactly** (§10.3): `DUP` aliases the same term, combinators execute their real shuffle, **arity is read from the Tier 0 arrow** (the evaluator owns no independent arity). Core-shuffle conformance is **property-tested against the interpreter**, because a mis-shuffle yields a vacuous proof (a green checkmark proving nothing), not a crash.
8. Word/combinator contracts are typed from **declared signatures only**; a quotation argument's **body is never expanded** into a caller's contract.
9. **No polymorphic recursion** is silently admitted.
10. **Tier separation:** Tier 0 passes with **zero dependence on Z3**. Refinements are a **forwarded payload Tier 0 never reads.** The VC generator targets an **interchange format (SMT-LIB)** behind a **solver trait**, never calling a solver in its core.
11. The solver seam exposes **scoped assertions** (`push_scope`/`pop_scope`); SMT-LIB emission maintains push/pop parity.
12. **Undecidable subsumption fails closed.** Absent refinement = `where true`; the unrefined-meets-required-guarantee case emits the "carries no contract" diagnostic, not a bare counterexample.
13. **`assume` is STRICT and near-vestigial.** Legal only where a genuinely opaque/uncontracted dependency sits in the obligation's chain; **rejected** on any goal the solver could discharge without it (the positive-rejection check must actually run). The strict exploratory check is **cached and fails lenient** (timeout ⇒ accept). Every `assume` is a first-class, enumerable ledger entry; the modulo-assumption status propagates to callers visibly. The ledger means *"cannot honestly prove,"* never *"didn't bother."*
14. **All Tier 1 machinery is compile-time**; Z3 and the shadow stack have **zero runtime presence**.
15. **Whole-program, no modules.** One compilation unit, one global ledger, definitions in a flat global namespace, program = entry against empty stack. No imports, no serialized contracts, no version skew, no module `Cid`s. `grep assume` over source = the complete user trusted base.
16. **Two-registrant operator model; the user registers nothing.** Trusted contracts come **only** from operator registration — by the **language core** (primitives `CALL`/`DIP`/`IF`, with Tier 0 schemes + Tier 1 refinement axioms) and the **embedder** (host operators). The user cannot register; the capability wall is the embedding API, structural. Operator contracts (axioms) and user `assume` are **separate buckets**; the strict drop-and-re-run check applies to `assume` only.
17. **Registration is explicit attestation.** The embedder visibly attests each operator contract; Caternary takes it on faith (the FFI floor). The **operator table is the modulo** of every proof; a verified program's status is *"sound modulo the attested operator contracts."*
18. **Tier seam is an immutability barrier.** Tier 1 (shadow evaluator, VC generator) treats the Tier 0 substitution as **read-only**; `push_scope`/`pop_scope` live in the Z3 trait + shadow-evaluator state, never the inference substitution. A Tier 1 branch mutating a Tier 0 binding is **impossible by construction.**
19. **The shadow evaluator reads the Tier 0 arrow for arity** (folded into 7, restated for emphasis): opaque words pop/push per their inferred Tier 0 type. Tier 0 shape closure is the structural rail — hence Tier 1's gating behind green Tier 0.
20. **CI gate; free runtime.** `caternary check` discharges everything (incl. operator axioms) at build time; a checked program links no solver and carries no shadow stack — zero runtime verification machinery.

---

## 14. Suggested build order

1. **Whole-program driver + operator registration + attestation API.** Flat definition namespace (reuse the pre-pass); `main`-against-empty-stack; `register_operator_with_contract` as a visibly-trusted call; language-core primitives (`CALL`/`DIP`/`IF`) registered with their Tier 0 schemes; the user has no registration surface (capability wall). → M0
2. Type representation + **flat substitution + trailed undo (Tier-0-only)** + **origin spans + typing frames** (§3).
3. Unifier with **canonicalize-then-occurs, uniform through arrows, depth-guarded** (§4) → M1.
4. Sequence inference, literals, quotations, locals, `IF`-as-combinator (§5) → M2.
5. SCC pass + generalization (§6) → M3.
6. Combinator schemes + rank-2 detection (§8) → M4.
7. Provenance/frame/localize-inward error reframing (§7) → M5. **Tier 0 complete.**
8. *(Phase 2, gated)* refinement parser (§10.1) → M6; positional binding + **shadow evaluator (arity from Tier 0 arrow) with conformance tests** (§10.2–10.3) → M7; **path conditions + scoped seam (Z3-trait-only)** (§10.4, §10.8) → M8; first-order VCs + Z3 (§10.5, §10.8) → M9; **higher-order subsumption + operator refinement axioms, fail-closed** (§10.6) → M10; **gradual interop** (§10.7) → M11; **`assume` boundary, strict + cached/lenient** (§10.7) → M12; **SMT-LIB parity** (§10.9) → M13; **whole-program ledger + attestation hash + CI-gate/free-runtime checks** (architecture section, §10.10) → M14.

Build Tier 0 end-to-end on `abs` / `sqrt` / `push` / `DIP` before anything fancy. Never build value-language subtyping. Subsumption is two implications handed to Z3, not a feature of the type engine. The shadow stack mirrors runtime data flow exactly — including reading arity from the Tier 0 arrow — or your proofs are vacuous. Trusted contracts come only from attested operator registration; `assume` is strict and names only what you genuinely cannot prove. Whole-program, one ledger, no modules. Check in CI; run lean. When in doubt, prefer rejecting cleanly with a good message over widening silently — and fail closed.
