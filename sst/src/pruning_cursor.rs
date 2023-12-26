//! The primary point of the pruning cursor is to turn an in-memory skip-list into something that
//! looks like a consistent cut of the data.  Consequently, there nees to be more logic than just
//! working on a static sst.  Other cursors will assume a pruning cursor gets applied beneath them
//! to create a cursor over an immutable data set.
use std::cmp::Ordering;
use std::fmt::Debug;

use keyvalint::{compare_bytes, Cursor, KeyRef};
use zerror_core::ErrorCore;

use super::Error;

/////////////////////////////////////////// PruningCursor //////////////////////////////////////////

/// A PruningCursor returns the latest value less than or equal to a timestmap.
pub struct PruningCursor<C: Cursor, E: Debug + From<Error>> {
    cursor: C,
    timestamp: u64,
    skip_key: Option<Vec<u8>>,
    _phantom_e: std::marker::PhantomData<E>,
}

impl<E: Debug + From<Error>, C: Cursor<Error = E>> PruningCursor<C, E> {
    /// Create a new pruning cursor.
    pub fn new(mut cursor: C, timestamp: u64) -> Result<Self, E> {
        cursor.seek_to_first()?;
        Ok(Self {
            cursor,
            timestamp,
            skip_key: None,
            _phantom_e: std::marker::PhantomData,
        })
    }

    fn set_skip_key(&mut self) {
        match self.key() {
            Some(v) => {
                self.skip_key = Some(v.key.to_vec());
            }
            None => {
                self.skip_key = None;
            }
        }
    }
}

impl<E: Debug + From<Error>, C: Cursor<Error = E>> Cursor for PruningCursor<C, E> {
    type Error = E;

    fn seek_to_first(&mut self) -> Result<(), E> {
        self.skip_key = None;
        self.cursor.seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), E> {
        self.skip_key = None;
        self.cursor.seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), E> {
        self.skip_key = None;
        self.cursor.seek(key)?;
        loop {
            let kr = match self.key() {
                Some(kr) => kr,
                None => {
                    return Ok(());
                }
            };
            if kr.timestamp <= self.timestamp && self.value().is_none() {
                self.set_skip_key();
            } else if kr.timestamp <= self.timestamp
                && (self.skip_key.is_none()
                    || compare_bytes(self.skip_key.as_ref().unwrap(), kr.key) != Ordering::Equal)
            {
                self.set_skip_key();
                return Ok(());
            }
            self.cursor.next()?;
        }
    }

    fn prev(&mut self) -> Result<(), E> {
        if self.key().is_none() {
            self.skip_key = None;
        }
        loop {
            // Skip the set skip key.
            self.cursor.prev()?;
            while self.skip_key.is_some() {
                let kr = match self.key() {
                    Some(kr) => kr,
                    None => {
                        self.skip_key = None;
                        return Ok(());
                    }
                };
                // SAFETY(rescrv):  We check is_some() as while invariant.
                if compare_bytes(self.skip_key.as_ref().unwrap(), kr.key) != Ordering::Equal {
                    self.skip_key = None;
                } else {
                    self.cursor.prev()?;
                }
            }
            // This is the key we want to investigate.
            // Find the largest timestamp less than self.timestamp for this key.
            let kr = match self.key() {
                Some(kr) => kr,
                None => {
                    self.skip_key = None;
                    return Ok(());
                }
            };
            // If the smallest timestamp of this key (the first we'll hit after a series of prevs)
            // is too large, set skip key and continue at the top.
            if kr.timestamp > self.timestamp {
                self.set_skip_key();
                continue;
            }
            // We know there exists a target key with timestamp less than or equal to our threshold
            // timestamp.
            let target_key = kr.key.to_vec();
            // Loop until we overrun and then reverse by at least one.
            //
            // Note that reversing by one is not sufficient as late-arriving writes---which are
            // allowed to arrive out of order in the prescribed pattern---will possibly insert
            // earlier data.  The LSM tree relies upon the pruning cursor to give a consistent view
            // by screening out these writes, and then it sequences the writes so that they expose
            // their data in an order consistent with their timestamps.
            loop {
                self.cursor.prev()?;
                let kr = match self.key() {
                    Some(kr) => kr,
                    None => {
                        break;
                    }
                };
                if kr.timestamp > self.timestamp
                    || compare_bytes(kr.key, &target_key) != Ordering::Equal
                {
                    break;
                }
            }
            // Here's where we would be most likely to fail with concurrency present.
            if self.key().is_none() {
                self.cursor.next()?;
            }
            // SAFETY(rescrv)  self.key() cannot be None because we witnessed target_key above as a
            // valid value that *must* come after the None at the head of the list.  We know that
            // key must have a lesser timestamp by the continue under kr.timestamp > self.timestamp
            // above.  Thus we can simply seek until we have both fronts.
            while let Some(kr) = self.key() {
                if kr.timestamp <= self.timestamp && compare_bytes(kr.key, &target_key).is_eq() {
                    break;
                } else {
                    self.cursor.next()?;
                }
            }
            // Operate on the considered value.
            // The largest of target_key less than or equal to the timestamp.
            let kr = match self.key() {
                Some(kr) => kr,
                None => {
                    let err = Error::LogicError {
                        core: ErrorCore::default(),
                        context: "should be positioned at some key with a value".to_string(),
                    };
                    return Err(err.into());
                }
            };
            // SAFETY(rescrv): Ensured by the while loop above.
            assert!(kr.timestamp <= self.timestamp);
            assert!(compare_bytes(kr.key, &target_key) == Ordering::Equal);
            // If it's not a tombstone, return the value (and skip it next time)
            // Otherwise, just skip it.
            if self.value().is_some() {
                self.set_skip_key();
                return Ok(());
            } else {
                self.set_skip_key();
            }
        }
    }

    fn next(&mut self) -> Result<(), E> {
        loop {
            self.cursor.next()?;
            let kr = match self.key() {
                Some(kr) => kr,
                None => {
                    return Ok(());
                }
            };
            if kr.timestamp <= self.timestamp && self.value().is_none() {
                self.set_skip_key();
            } else if kr.timestamp <= self.timestamp
                && (self.skip_key.is_none()
                    // SAFETY(rescrv):  We check is_none() and short circuit.
                    || compare_bytes(self.skip_key.as_ref().unwrap(), kr.key) != Ordering::Equal)
            {
                self.set_skip_key();
                return Ok(());
            }
        }
    }

    fn key(&self) -> Option<KeyRef> {
        self.cursor.key()
    }

    fn value(&self) -> Option<&[u8]> {
        self.cursor.value()
    }
}
