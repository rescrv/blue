//! M14 — whole-program ledger + attestation hash + CI-gate / free-runtime model
//! (architecture section / §10.10 / §12 M14 / invariants 14–17, 20).
//!
//! This is the final milestone. It does not introduce a new verification
//! mechanism; it makes the *trust story* of the whole-program / embeddable /
//! operator-contracted design **enumerable and content-addressable**:
//!
//! 1. **One global ledger** ([`crate::Ledger`], M12): every `assume` in the
//!    program lands in one structure, so `grep assume` over the source
//!    enumerates the *complete* user trusted base — there are no modules to
//!    reconcile (invariant 15). [`grep_assume_tokens`] is the source-side
//!    `grep`; a test asserts it equals the ledger's accepted entries for a clean
//!    program.
//! 2. **The operator table is the modulo of every proof** (invariants 16/17):
//!    the language-core primitives ([`crate::core_scheme`]) plus the embedder's
//!    [`crate::Evaluator::register_operator_with_contract`] entries. [`OperatorTable`]
//!    enumerates exactly that set — the enumerable statement of *"proven sound
//!    modulo these attested contracts."*
//! 3. **One artifact attestation hash** ([`ContractSet::attestation_hash`]):
//!    a content-hash of the whole-program contract set — every generalized
//!    definition signature (M3 schemes) plus the operator table — **stable across
//!    rebuilds of unchanged source**. The discharge cache uses exact
//!    per-obligation content keys; [`crate::obligation_sub_hash`] exposes the
//!    corresponding M14 fingerprint, so changing one obligation invalidates only
//!    its own cache entry (warm-compile reuse).
//! 4. **CI gate; free runtime** (invariants 14/20): a *checked* program links no
//!    solver (the native `z3` backend is a build-time-only check backend behind
//!    an optional, off-by-default feature) and carries no
//!    shadow-stack/solver machinery at runtime (the shadow stack is compile-time
//!    only; it is never a field of [`crate::Evaluator`]). [`caternary check`] —
//!    the public [`crate::check`] / [`crate::Evaluator::check_refinements`] path
//!    — pays the entire verification cost at build time; these properties are
//!    asserted structurally in the M14 tests below.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use crate::Token;
use crate::check::TypeError;
use crate::combinators::Quotable;
use crate::evaluator::Evaluator;
use crate::refinement::ASSUME_PREFIX;
use crate::types::Scheme;
use crate::types::StackTy;
use crate::types::Ty;
use crate::types::TyKind;
use crate::types::WordTy;
use crate::types::core_scheme;

const CORE_PRIMITIVES: &[&str] = &[
    "DUP", "DROP", "SWAP", "OVER", "ROT", "-ROT", "NIP", "TUCK", "2DUP", "2DROP", "2SWAP", "2OVER",
    "2ROT", "CALL", "DIP", "2DIP", "3DIP", "IF", "KEEP", "2KEEP", "3KEEP", "BI", "BI*", "BI@",
    "TRI", "TRI*", "TRI@", "COMPOSE", "CURRY", "2CURRY", "3CURRY", "WHEN", "UNLESS", "MAP",
    "FILTER", "FOLD", "EACH",
];

// ===========================================================================
// The operator table — the modulo of every proof (invariants 16/17)
// ===========================================================================

/// Which of the **two registrants** attested an operator contract (architecture
/// section / invariant 16). The user is neither — there is no third variant,
/// because the user has no registration surface (the capability wall is the
/// embedding API, structural).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperatorOrigin {
    /// The **language core** registered this irreducible primitive
    /// (fixed stack shuffles plus fixed-arity quotation combinators), baked into
    /// [`crate::core_scheme`] with its Tier-0 scheme (and Tier-1 axiom, §10.6).
    LanguageCore,
    /// The **embedder** registered this host operator at embed time via
    /// [`crate::Evaluator::register_operator_with_contract`] — an explicit
    /// attestation that the host's Rust satisfies the contract (the FFI floor).
    Embedder,
}

impl OperatorOrigin {
    /// A stable tag for hashing/display (`"core"` / `"embedder"`).
    pub fn tag(self) -> &'static str {
        match self {
            OperatorOrigin::LanguageCore => "core",
            OperatorOrigin::Embedder => "embedder",
        }
    }
}

/// One attested operator contract — a row of the operator table (invariant 17).
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorContract {
    /// The operator name as written in Caternary source (the runtime
    /// UPPER_SNAKE_CASE spelling for core primitives, the registered name for
    /// embedder ops).
    pub name: String,
    /// Who attested it (language core vs. embedder).
    pub origin: OperatorOrigin,
    /// The attested Tier-0 [`Scheme`] — the type Caternary takes on faith.
    pub scheme: Scheme,
}

/// The **operator table**: the complete, enumerable **modulo** of every
/// Caternary proof (architecture section / invariants 16/17). It is the exact
/// statement of *"this program is proven sound **modulo** these attested
/// contracts"*: language-core primitives plus embedder registrations, and
/// nothing else (the user cannot add to it).
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorTable {
    entries: Vec<OperatorContract>,
}

impl OperatorTable {
    /// Build the operator table of an [`Evaluator`]: the language-core primitives
    /// ([`crate::core_scheme`], in the fixed runtime-name order) followed
    /// by the embedder's registered contracts (sorted by name). The result is
    /// deterministic for a fixed embedding — the input to the stable attestation
    /// hash.
    pub fn of<T>(evaluator: &Evaluator<T>) -> Self {
        let mut entries = Vec::new();
        // Language core: every primitive that has a core scheme, in the fixed
        // runtime-name order (deterministic).
        for name in CORE_PRIMITIVES {
            if let Some(scheme) = core_scheme(name) {
                entries.push(OperatorContract {
                    name: (*name).to_string(),
                    origin: OperatorOrigin::LanguageCore,
                    scheme,
                });
            }
        }
        // Embedder: registered contracts, sorted by name for determinism.
        let mut names: Vec<String> = evaluator.contract_names().map(String::from).collect();
        names.sort();
        for name in names {
            if let Some(scheme) = evaluator.contract(&name) {
                entries.push(OperatorContract {
                    name,
                    origin: OperatorOrigin::Embedder,
                    scheme: scheme.clone(),
                });
            }
        }
        OperatorTable { entries }
    }

    /// Every attested contract, in deterministic order (core first, then embedder
    /// by name). This enumeration *is* the modulo of every proof.
    pub fn entries(&self) -> &[OperatorContract] {
        &self.entries
    }

    /// The names of every attested operator, in table order.
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }

    /// The language-core half of the table (the irreducible primitives).
    pub fn core(&self) -> impl Iterator<Item = &OperatorContract> {
        self.entries
            .iter()
            .filter(|e| e.origin == OperatorOrigin::LanguageCore)
    }

    /// The embedder half of the table (host operators attested at embed time).
    pub fn embedder(&self) -> impl Iterator<Item = &OperatorContract> {
        self.entries
            .iter()
            .filter(|e| e.origin == OperatorOrigin::Embedder)
    }

    /// The number of attested contracts in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty (no core primitives and no embedder ops).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ===========================================================================
// The whole-program contract set + its one artifact attestation hash
// ===========================================================================

/// The **whole-program contract set** (§12 M14): every generalized definition
/// signature (M3 [`Scheme`]) plus the [`OperatorTable`]. This is exactly the set
/// the architecture section says to content-hash into *one artifact attestation
/// hash* for build reproducibility and the CI gate — there is no per-module
/// `Cid` machinery, one program-level attestation.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractSet {
    /// Generalized definition signatures, keyed by name (sorted iteration).
    pub definitions: BTreeMap<String, Scheme>,
    /// The operator table (the modulo).
    pub operators: OperatorTable,
}

impl ContractSet {
    /// Derive the whole-program contract set from a loaded [`Evaluator`]. This
    /// runs the same SCC generalization pass [`crate::type_check`] runs, so a
    /// program that does not type-check returns the same [`TypeError`]; a checked
    /// program returns the full set. Reads the evaluator read-only (§3 barrier).
    pub fn of<T>(evaluator: &Evaluator<T>) -> Result<Self, TypeError>
    where
        T: Quotable,
    {
        let schemes = crate::check::definition_schemes(evaluator)?;
        Ok(ContractSet {
            definitions: schemes.into_iter().collect(),
            operators: OperatorTable::of(evaluator),
        })
    }

    /// The **one artifact attestation hash** (§12 M14): a content-hash of the
    /// whole contract set — every definition signature (canonicalized so a
    /// variable-id renaming cannot perturb it) plus every operator-table entry.
    /// **Stable across rebuilds of unchanged source**: the canonical rendering
    /// drops origin spans and renumbers type/row variables in first-appearance
    /// order, so the hash is a property of the *contracts*, not of inference's
    /// internal counters.
    pub fn attestation_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        // Definitions, in sorted name order (BTreeMap iterates sorted).
        h.write_usize(self.definitions.len());
        for (name, scheme) in &self.definitions {
            name.hash(&mut h);
            canonical_scheme(scheme).hash(&mut h);
        }
        // Operator table, in table order (deterministic).
        h.write_usize(self.operators.len());
        for entry in self.operators.entries() {
            entry.name.hash(&mut h);
            entry.origin.tag().hash(&mut h);
            canonical_scheme(&entry.scheme).hash(&mut h);
        }
        h.finish()
    }
}

/// The whole-program artifact attestation hash for a loaded [`Evaluator`]
/// (§12 M14): builds the [`ContractSet`] and hashes it. Convenience over
/// [`ContractSet::of`] + [`ContractSet::attestation_hash`].
pub fn attestation_hash<T>(evaluator: &Evaluator<T>) -> Result<u64, TypeError>
where
    T: Quotable,
{
    Ok(ContractSet::of(evaluator)?.attestation_hash())
}

// ---- canonical scheme rendering (stable across var-id renaming) ------------

/// Canonicalize a [`Scheme`] to a stable string: type/row variables are
/// renumbered in first-appearance order (input before output) and origin spans
/// are dropped, so two structurally identical schemes with different internal
/// variable ids — e.g. the same definition re-inferred on a rebuild — render
/// identically. This is what makes [`ContractSet::attestation_hash`] reproducible.
fn canonical_scheme(scheme: &Scheme) -> String {
    let mut tymap: BTreeMap<u32, usize> = BTreeMap::new();
    let mut rowmap: BTreeMap<u32, usize> = BTreeMap::new();
    render_word(&scheme.ty, &mut tymap, &mut rowmap)
}

fn canon_id(map: &mut BTreeMap<u32, usize>, v: u32) -> usize {
    let next = map.len();
    *map.entry(v).or_insert(next)
}

fn render_word(
    w: &WordTy,
    tymap: &mut BTreeMap<u32, usize>,
    rowmap: &mut BTreeMap<u32, usize>,
) -> String {
    format!(
        "({} -- {})",
        render_stack(&w.input, tymap, rowmap),
        render_stack(&w.output, tymap, rowmap)
    )
}

fn render_stack(
    s: &StackTy,
    tymap: &mut BTreeMap<u32, usize>,
    rowmap: &mut BTreeMap<u32, usize>,
) -> String {
    let elems: Vec<String> = s
        .elems
        .iter()
        .map(|e| render_ty(e, tymap, rowmap))
        .collect();
    format!("[{}|r{}]", elems.join(","), canon_id(rowmap, s.row))
}

fn render_ty(
    t: &Ty,
    tymap: &mut BTreeMap<u32, usize>,
    rowmap: &mut BTreeMap<u32, usize>,
) -> String {
    match &t.kind {
        TyKind::Var(v) => format!("t{}", canon_id(tymap, *v)),
        TyKind::Con(n) => n.clone(),
        TyKind::App(n, args) => {
            let inner: Vec<String> = args.iter().map(|a| render_ty(a, tymap, rowmap)).collect();
            format!("{n}({})", inner.join(","))
        }
        TyKind::Quote(w) => render_word(w, tymap, rowmap),
    }
}

// ===========================================================================
// grep assume — the source-side enumeration of the user trusted base
// ===========================================================================

/// Enumerate the `assume( … )` surfaces in a token body, recursing into
/// quotations — the source-side `grep assume` (architecture section / invariant
/// 15). Returns each surface verbatim (e.g. `assume(result>0)`), in source
/// order. For a clean (checked) program this equals the global [`crate::Ledger`]'s
/// accepted entries — there are no modules to reconcile, so one `grep` over the
/// whole program *is* the complete user trusted base.
pub fn grep_assume_tokens(tokens: &[Token]) -> Vec<String> {
    let mut out = Vec::new();
    walk_assume(tokens, &mut out);
    out
}

fn walk_assume(tokens: &[Token], out: &mut Vec<String>) {
    for t in tokens {
        match t {
            Token::Bracket(body) => walk_assume(body, out),
            Token::Word(w) if w.starts_with(ASSUME_PREFIX) => out.push(w.clone()),
            Token::Word(_) => {}
        }
    }
}

#[cfg(test)]
mod tests;
