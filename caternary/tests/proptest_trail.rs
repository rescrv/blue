//! Harness 2 — Trail-Undo Interleaving (proptest).
//!
//! The Tier-0 substitution arena ([`Subst`]) is a *flat mutable map with a
//! trailed undo log* — exactly the structure where an aliasing or pop-order bug
//! hides. Such a bug corrupts inference *silently* rather than crashing, so a
//! reference-free differential check against an obviously-correct snapshot oracle
//! is the only thing that catches it.
//!
//! The properties below are stated against the type system's own claimed
//! invariants, exercised through these seams:
//!
//! * `mark`   → [`Subst::checkpoint`]  (returns the trail length).
//! * `undo`   → [`Subst::rewind`]      (pops the trail back to a length).
//! * `unify`  → [`InferCtx::unify_ty`] / [`InferCtx::unify_stack`], which mutate
//!   `ctx.subst` and push trail entries. The trail thus lives on `ctx.subst`, and
//!   checkpoint/rewind operate on it.
//! * The map is `Vec<Option<Ty>>` indexed by var (not a `HashMap`), so the
//!   "byte-exact" comparison is over a *canonicalized* form — the sorted list of
//!   `(var, value)` pairs for bound slots — so capacity-/length-driven `Vec`
//!   differences are not mistaken for state differences.
//!
//! caternary's [`TrailEntry`] records the **old value** of the slot, not just the
//! var, so a binding *can* be overwritten and undo restores the prior value
//! (path-compression-style rebinds are representable). That makes the
//! overwrite-and-restore property (P9) essential rather than optional, and it is
//! written here as a first-class property.

use caternary::*;
use proptest::prelude::*;

/// The byte-of-state we assert over: the bound slots of a [`Subst`], canonical
/// (sorted by var). All types in these tests are drawn from a small var pool, so
/// every binding `unify` can make stays inside `0..POOL`; iterating that range
/// captures the whole map. `Ty`/`StackTy` derive `PartialEq` (spans included),
/// so this is the semantic-identity ("byte-exact") form the comparison needs.
const POOL: u32 = 6;

/// The canonical, comparable form of a substitution's bound slots: sorted
/// `(var, value)` pairs for the type slots and the row slots.
type CanonMap = (Vec<(u32, Ty)>, Vec<(u32, StackTy)>);

/// Canonicalize the substitution's bound slots into a comparable snapshot.
fn canonical_map(s: &Subst) -> CanonMap {
    let mut tys = Vec::new();
    let mut rows = Vec::new();
    for v in 0..POOL {
        if let Some(t) = s.get_ty(v) {
            tys.push((v, t.clone()));
        }
        if let Some(r) = s.get_row(v) {
            rows.push((v, r.clone()));
        }
    }
    // get_ty/get_row already visit in var order, so the vectors are sorted.
    (tys, rows)
}

fn sp() -> Span {
    Span { start: 0, end: 1 }
}

// ---------------------------------------------------------------------------
// Generators.
// ---------------------------------------------------------------------------

/// A small element type drawn from a tiny universe: `Num`, `Bool`, or a variable
/// from the pool. Deliberately *narrow* so unifications collide frequently —
/// uniform-random types rarely interact, and collisions are the entire point.
fn any_ty() -> impl Strategy<Value = Ty> {
    prop_oneof![
        Just(Ty::num(sp())),
        Just(Ty::bool(sp())),
        (0..POOL).prop_map(|v| Ty::var(v, sp())),
    ]
}

/// A short stack type: 0..3 element types over a pooled row variable.
fn any_stack() -> impl Strategy<Value = StackTy> {
    (prop::collection::vec(any_ty(), 0..3), 0..POOL)
        .prop_map(|(elems, row)| StackTy::new(elems, row, sp()))
}

/// A pair of stacks to attempt to unify. Biased toward `Num`/`Bool`/`Quote`
/// collisions by reusing the same narrow `any_ty` universe on both sides.
fn any_unify_pair() -> impl Strategy<Value = (StackTy, StackTy)> {
    (any_stack(), any_stack())
}

/// The abstract operation model for the interleaving property.
#[derive(Clone, Debug)]
enum Op {
    /// Attempt a unification; may extend the map (or fail, leaving the trail valid).
    Unify(StackTy, StackTy),
    /// Push a savepoint.
    Mark,
    /// Pop to the most recent un-consumed mark.
    UndoToLastMark,
}

/// A raw op before validity repair: a unify pair (fully generated), or a request
/// to mark, or a request to undo.
#[derive(Clone, Debug)]
enum RawOp {
    Unify(StackTy, StackTy),
    Mark,
    Undo,
}

fn any_raw_op() -> impl Strategy<Value = RawOp> {
    prop_oneof![
        any_unify_pair().prop_map(|(a, b)| RawOp::Unify(a, b)),
        Just(RawOp::Mark),
        Just(RawOp::Undo),
    ]
}

/// Generate a *valid* op sequence: never emit `UndoToLastMark` without an
/// outstanding `Mark`. A balance counter tracks outstanding marks; an orphaned
/// `Undo` is rewritten to a `Mark` so every candidate the shrinker produces
/// stays valid.
fn valid_op_sequence(max: usize) -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(any_raw_op(), 0..max).prop_map(|raws| {
        let mut ops = Vec::with_capacity(raws.len());
        let mut outstanding = 0usize;
        for raw in raws {
            match raw {
                RawOp::Unify(a, b) => ops.push(Op::Unify(a, b)),
                RawOp::Mark => {
                    outstanding += 1;
                    ops.push(Op::Mark);
                }
                RawOp::Undo => {
                    if outstanding > 0 {
                        outstanding -= 1;
                        ops.push(Op::UndoToLastMark);
                    } else {
                        outstanding += 1;
                        ops.push(Op::Mark);
                    }
                }
            }
        }
        ops
    })
}

/// The snapshot oracle: a stack of whole-map snapshots.
/// `Mark` pushes a clone of the current map; `UndoToLastMark` pops and restores
/// it wholesale. Obviously correct, obviously too slow for production — exactly
/// what a test oracle should be. `Unify` mirrors the real map's post-unify state
/// into the oracle's current snapshot (the oracle does not re-implement
/// unification; it only models the mark/undo stack discipline), so any
/// divergence localizes to a *bad undo*.
struct SnapshotStack {
    current: CanonMap,
    saved: Vec<CanonMap>,
}

impl SnapshotStack {
    fn new() -> Self {
        SnapshotStack {
            current: (Vec::new(), Vec::new()),
            saved: Vec::new(),
        }
    }
    fn mark(&mut self) {
        self.saved.push(self.current.clone());
    }
    fn undo(&mut self) {
        if let Some(prev) = self.saved.pop() {
            self.current = prev;
        }
    }
    /// Mirror the real map after a (possibly failed) unify.
    fn observe(&mut self, real: CanonMap) {
        self.current = real;
    }
}

// ---------------------------------------------------------------------------
// P7 — Undo is a byte-exact inverse.
// ---------------------------------------------------------------------------

proptest! {
    /// Snapshot, mark, mutate, undo, compare. After rewinding to the mark the
    /// canonical map must be byte-identical to the pre-mark snapshot, and the
    /// trail must be empty back to the mark (checkpoint round-trips).
    #[test]
    fn p7_undo_restores_exactly(unifs in prop::collection::vec(any_unify_pair(), 0..32)) {
        let mut ctx = InferCtx::new();
        let before = canonical_map(&ctx.subst);
        let m = ctx.subst.checkpoint();
        for (a, b) in &unifs {
            // Ignore unify failures; the trail must remain valid either way.
            let _ = ctx.unify_stack(a, b);
        }
        ctx.subst.rewind(m);
        prop_assert_eq!(canonical_map(&ctx.subst), before);
        // The trail is empty back to the mark: a second rewind is a no-op.
        prop_assert_eq!(ctx.subst.checkpoint(), m);
    }
}

// ---------------------------------------------------------------------------
// P8 — Interleaved marks/undos match the snapshot oracle.
// ---------------------------------------------------------------------------

proptest! {
    /// A random *valid* op sequence run against the real trailed `Subst` and the
    /// snapshot oracle, compared at **every** step so the shrinker can localize
    /// the exact op that diverges. This is the property that catches pop-order
    /// bugs: a wrong-order `rewind` diverges the moment the bad undo lands.
    #[test]
    fn p8_trail_matches_snapshot_oracle(ops in valid_op_sequence(64)) {
        let mut ctx = InferCtx::new();
        let mut shadow = SnapshotStack::new();
        // Real marks are trail lengths; we keep our own stack of them, mirroring
        // the shadow's `saved` stack one-for-one.
        let mut marks: Vec<usize> = Vec::new();

        for (i, op) in ops.iter().enumerate() {
            match op {
                Op::Unify(a, b) => {
                    let _ = ctx.unify_stack(a, b);
                    shadow.observe(canonical_map(&ctx.subst));
                }
                Op::Mark => {
                    marks.push(ctx.subst.checkpoint());
                    shadow.mark();
                }
                Op::UndoToLastMark => {
                    // Valid sequences never under-run; assert that invariant.
                    let m = marks.pop().expect("valid sequence: a mark is outstanding");
                    ctx.subst.rewind(m);
                    shadow.undo();
                }
            }
            prop_assert_eq!(
                canonical_map(&ctx.subst),
                shadow.current.clone(),
                "divergence at op #{}: {:?}",
                i,
                op
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P9 — Overwrite-and-restore (essential here: TrailEntry records the old value).
// ---------------------------------------------------------------------------

proptest! {
    /// caternary's trail can *overwrite* an already-bound slot and undo must
    /// restore the prior value, not delete the key. We drive overwrites directly
    /// via `bind_ty`/`bind_row` (public) over a deliberately tiny var pool so
    /// rebinds of the same slot are frequent — collisions are where this bug
    /// lives, and a large var space hides it. After rewinding to a mark taken
    /// before a batch of (re)binds, the map must equal the pre-mark snapshot.
    #[test]
    fn p9_overwrite_and_restore(
        binds in prop::collection::vec((0u32..3, any_ty()), 0..40),
        // A handful of pre-existing bindings the batch will overwrite.
        seed in prop::collection::vec((0u32..3, any_ty()), 0..3),
    ) {
        let mut ctx = InferCtx::new();
        // Establish some prior bindings, then snapshot: rewind must restore THESE
        // exact values even though the batch rebinds the same slots.
        for (v, t) in &seed {
            ctx.subst.bind_ty(*v, t.clone());
        }
        let before = canonical_map(&ctx.subst);
        let m = ctx.subst.checkpoint();
        for (v, t) in &binds {
            ctx.subst.bind_ty(*v, t.clone());
        }
        ctx.subst.rewind(m);
        prop_assert_eq!(canonical_map(&ctx.subst), before);
    }

    /// The same property targeting row slots, which carry their own old-value
    /// trail entries.
    #[test]
    fn p9_overwrite_and_restore_rows(
        binds in prop::collection::vec((0u32..3, any_stack()), 0..40),
        seed in prop::collection::vec((0u32..3, any_stack()), 0..3),
    ) {
        let mut ctx = InferCtx::new();
        for (v, st) in &seed {
            ctx.subst.bind_row(*v, st.clone());
        }
        let before = canonical_map(&ctx.subst);
        let m = ctx.subst.checkpoint();
        for (v, st) in &binds {
            ctx.subst.bind_row(*v, st.clone());
        }
        ctx.subst.rewind(m);
        prop_assert_eq!(canonical_map(&ctx.subst), before);
    }
}

// ---------------------------------------------------------------------------
// P10 — Failed unification leaves a clean trail.
// ---------------------------------------------------------------------------

proptest! {
    /// A unification that fails partway (mismatch or occurs-check trip) may push
    /// *some* bindings before detecting the conflict. The contract caternary
    /// commits to: whatever partial state a failed `unify` leaves, it is on the
    /// trail, so undoing to the pre-unify mark makes the state pristine again.
    /// This property pins that contract so a refactor cannot silently change it.
    #[test]
    fn p10_failed_unify_leaves_clean_trail(pairs in prop::collection::vec(any_unify_pair(), 1..16)) {
        let mut ctx = InferCtx::new();
        for (a, b) in &pairs {
            let before = canonical_map(&ctx.subst);
            let m = ctx.subst.checkpoint();
            let result = ctx.unify_stack(a, b);
            if result.is_err() {
                ctx.subst.rewind(m);
                prop_assert_eq!(
                    canonical_map(&ctx.subst),
                    before,
                    "failed unify + undo must restore pristine state"
                );
            } else {
                // Successful unifications accumulate; keep going from here.
            }
        }
    }

    /// A directed P10: construct a guaranteed conflict (`Num` vs `Bool` on top of
    /// otherwise-unifiable stacks), confirm it errors, and confirm the enclosing
    /// mark reverts any partial bindings made before the conflict was detected.
    #[test]
    fn p10_directed_conflict_reverts(prefix in prop::collection::vec(any_ty(), 0..3)) {
        let s = sp();
        let mut ctx = InferCtx::new();
        // ( 'r0 prefix.. Num ) vs ( 'r1 prefix.. Bool ): the prefix may bind
        // vars/rows as it peels, then the Num/Bool top conflicts.
        let mut left = prefix.clone();
        left.push(Ty::num(s));
        let mut right = prefix.clone();
        right.push(Ty::bool(s));
        let a = StackTy::new(left, 0, s);
        let b = StackTy::new(right, 1, s);

        let before = canonical_map(&ctx.subst);
        let m = ctx.subst.checkpoint();
        let result = ctx.unify_stack(&a, &b);
        prop_assert!(result.is_err(), "Num-vs-Bool top must be a typed mismatch");
        ctx.subst.rewind(m);
        prop_assert_eq!(canonical_map(&ctx.subst), before);
    }
}
