//! A MergingCursor merges several cursors together.

use keyvalint::{Cursor, KeyRef};

//////////////////////////////////////////// Comparator ////////////////////////////////////////////

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum Comparator {
    Forward,
    Reverse,
}

impl Comparator {
    #[allow(clippy::borrowed_box)]
    fn is_less<C: Cursor>(&self, lhs: &C, rhs: &C) -> bool {
        let lhs_key = lhs.key();
        let rhs_key = rhs.key();
        match (self, lhs_key, rhs_key) {
            // We're comparing two positioned cursors.
            (Comparator::Forward, Some(lhs), Some(rhs)) => lhs < rhs,
            // lhs is not at the end.  rhs is at the end.
            (Comparator::Forward, Some(_), None) => true,
            // lhs is at the end.  rhs is not at the end.
            (Comparator::Forward, None, Some(_)) => false,
            // Both are at the end.  Neither strictly less than the other.
            (Comparator::Forward, None, None) => false,

            // We're comparing two positioned cursors.
            (Comparator::Reverse, Some(lhs), Some(rhs)) => lhs > rhs,
            // lhs is not at the beginning.  rhs is at the beginning.
            (Comparator::Reverse, Some(_), None) => true,
            // lhs is at the beginning.  rhs is not at the beginning.
            (Comparator::Reverse, None, Some(_)) => false,
            // Both are at the beginning.  Neither strictly greater than the other.
            (Comparator::Reverse, None, None) => false,
        }
    }
}

/////////////////////////////////////////// MergingCursor //////////////////////////////////////////

/// MergingCursor takes several cursors of type `C` and merges them into one logical cursor.
pub struct MergingCursor<C: Cursor> {
    comparator: Comparator,
    cursors: Vec<C>,
}

impl<C: Cursor + Clone> Clone for MergingCursor<C> {
    fn clone(&self) -> Self {
        let comparator = self.comparator.clone();
        let cursors = self.cursors.clone();
        Self {
            comparator,
            cursors,
        }
    }
}

impl<C: Cursor> MergingCursor<C> {
    /// Create a new MergingCursor that wraps `cursors`.
    pub fn new(cursors: Vec<C>) -> Result<Self, C::Error> {
        let mut cursor = Self {
            comparator: Comparator::Forward,
            cursors,
        };
        cursor.seek_to_first()?;
        Ok(cursor)
    }

    // A reminder on heap indexing:
    // parent: (idx - 1) / 2
    // child_left: idx * 2 + 1;
    // child_right: idx * 2 + 2;
    //
    //   |-----|
    //   |---| |
    // 0 1 2 3 4
    // |-| |
    // |---|

    fn heapify(&mut self) {
        for i in 0..self.cursors.len() {
            self.percolate_down(self.cursors.len() - i - 1);
        }
    }

    // Assumption: The heap is valid at indices >= index.
    fn percolate_down(&mut self, mut index: usize) {
        loop {
            let child_lhs = index * 2 + 1;
            let child_rhs = index * 2 + 2;
            // Find the child the comparator says is less.
            // Making the lesser child the parent preserves the heap invariant.
            // Proof:  The greater child is lesser than every one of its descendants, which means
            //      that a value lesser than it will also be lesser than its descendants.
            let child = if child_lhs >= self.cursors.len() {
                break;
            } else if child_rhs >= self.cursors.len()
                || self
                    .comparator
                    .is_less(&self.cursors[child_lhs], &self.cursors[child_rhs])
            {
                child_lhs
            } else {
                child_rhs
            };
            if self
                .comparator
                .is_less(&self.cursors[index], &self.cursors[child])
            {
                break;
            } else {
                self.cursors.swap(index, child);
                index = child;
            }
        }
    }
}

impl<C: Cursor> Cursor for MergingCursor<C> {
    type Error = C::Error;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        self.comparator = Comparator::Forward;
        for cursor in self.cursors.iter_mut() {
            cursor.seek_to_first()?;
            cursor.next()?;
        }
        self.heapify();
        if !self.cursors.is_empty() {
            self.cursors[0].seek_to_first()?;
        }
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        self.comparator = Comparator::Reverse;
        for cursor in self.cursors.iter_mut() {
            cursor.seek_to_last()?;
            cursor.prev()?;
        }
        self.heapify();
        if !self.cursors.is_empty() {
            self.cursors[0].seek_to_last()?;
        }
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        self.comparator = Comparator::Forward;
        for cursor in self.cursors.iter_mut() {
            cursor.seek(key)?;
        }
        self.heapify();
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        if self.comparator == Comparator::Forward {
            // We are positioned at a key K such that:
            // \forall C \in self.cursors: K <= C.value() && prev(C) < K
            for c in self.cursors.iter_mut() {
                c.prev()?;
            }
            self.comparator = Comparator::Reverse;
            self.heapify();
        } else if !self.cursors.is_empty() {
            self.cursors[0].prev()?;
            self.percolate_down(0);
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        if self.comparator == Comparator::Reverse {
            // We are positioned at a key K such that:
            // \forall C \in self.cursors: K >= C.value() && next(C) > K
            for c in self.cursors.iter_mut() {
                c.next()?;
            }
            self.comparator = Comparator::Forward;
            self.heapify();
        } else if !self.cursors.is_empty() {
            self.cursors[0].next()?;
            self.percolate_down(0);
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        if !self.cursors.is_empty() {
            self.cursors[0].key()
        } else {
            None
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if !self.cursors.is_empty() {
            self.cursors[0].value()
        } else {
            None
        }
    }
}
