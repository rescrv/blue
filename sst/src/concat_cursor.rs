#![allow(missing_docs)]

use keyvalint::{Cursor, KeyRef};

//////////////////////////////////////// ConcatenatingCursor ///////////////////////////////////////

// TODO(rescrv):  I don't like this cursor's structure.  Rethink it, then document.
pub struct ConcatenatingCursor<C: Cursor> {
    cursors: Vec<C>,
    position: usize,
}

impl<C: Cursor> ConcatenatingCursor<C> {
    pub fn new(mut cursors: Vec<C>) -> Result<Self, C::Error> {
        assert!(!cursors.is_empty());
        let position = 0;
        cursors[0].seek_to_first()?;
        Ok(Self { cursors, position })
    }

    fn reposition(&mut self, idx: usize) -> Result<(), C::Error> {
        assert!(!self.cursors.is_empty());
        if self.position != idx {
            // Seek to first on the current cursor to allow e.g. the lazy cursor to clean up state.
            if self.position < self.cursors.len() {
                self.cursors[self.position].seek_to_first()?;
            }
            self.position = idx;
        }
        Ok(())
    }
}

impl<C: Cursor> Cursor for ConcatenatingCursor<C> {
    type Error = C::Error;

    fn seek_to_first(&mut self) -> Result<(), C::Error> {
        self.reposition(0)?;
        self.cursors[self.position].seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), C::Error> {
        self.reposition(self.cursors.len() - 1)?;
        self.cursors[self.position].seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), C::Error> {
        let kref = KeyRef {
            key,
            timestamp: u64::max_value(),
        };
        let mut left = 0usize;
        let mut right = self.cursors.len() - 1;

        while left < right {
            let mut mid = (left + right) / 2;
            self.reposition(mid)?;
            self.cursors[self.position].seek_to_last()?;
            self.cursors[self.position].prev()?;
            while mid > left && self.cursors[self.position].key().is_none() {
                mid -= 1;
                self.reposition(mid)?;
                self.cursors[self.position].seek_to_last()?;
                self.cursors[self.position].prev()?;
            }
            if mid == left {
                break;
            }
            // SAFETY(rescrv):  We have a loop invariant above that goes until is_some or the
            // conditional right above us.
            if self.cursors[self.position].key().unwrap() >= kref {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        self.reposition(left)?;
        self.cursors[self.position].seek(key)
    }

    fn prev(&mut self) -> Result<(), C::Error> {
        loop {
            self.cursors[self.position].prev()?;
            if self.cursors[self.position].key().is_none() && self.position > 0 {
                self.reposition(self.position - 1)?;
                self.cursors[self.position].seek_to_last()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), C::Error> {
        loop {
            self.cursors[self.position].next()?;
            if self.cursors[self.position].value().is_none()
                && self.position + 1 < self.cursors.len()
            {
                self.reposition(self.position + 1)?;
                self.cursors[self.position].seek_to_first()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        if self.position < self.cursors.len() {
            self.cursors[self.position].key()
        } else {
            None
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if self.position < self.cursors.len() {
            self.cursors[self.position].value()
        } else {
            None
        }
    }
}
