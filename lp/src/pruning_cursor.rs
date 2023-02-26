use std::cmp::Ordering;

use zerror::{ErrorCore, ZError};

use super::{compare_bytes, Cursor, Error, KeyRef, KeyValueRef};

/////////////////////////////////////////// PruningCursor //////////////////////////////////////////

pub struct PruningCursor<C: Cursor> {
    cursor: C,
    timestamp: u64,
    skip_key: Option<Vec<u8>>,
}

impl<C: Cursor> PruningCursor<C> {
    pub fn new(mut cursor: C, timestamp: u64) -> Result<Self, ZError<Error>> {
        cursor.seek_to_first()?;
        Ok(Self {
            cursor,
            timestamp,
            skip_key: None,
        })
    }

    fn set_skip_key(&mut self) {
        match self.key() {
            Some(v) => {
                self.skip_key = Some(v.key.to_vec());
            },
            None => {
                self.skip_key = None;
            },
        }
    }
}

impl<C: Cursor> Cursor for PruningCursor<C> {
    fn seek_to_first(&mut self) -> Result<(), ZError<Error>> {
        self.skip_key = None;
        self.cursor.seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), ZError<Error>> {
        self.skip_key = None;
        self.cursor.seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), ZError<Error>> {
        self.skip_key = None;
        self.cursor.seek(key)
    }

    fn prev(&mut self) -> Result<(), ZError<Error>> {
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
                    },
                };
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
                },
            };
            let target_key = kr.key.to_vec();
            // Loop until we overrun and then reverse by one.
            loop {
                // We will step prev, and call next() when we overrun.
                // Unfortunately it's the only way that I see.
                self.cursor.prev()?;
                let kr = match self.key() {
                    Some(kvr) => kvr,
                    None => {
                        self.cursor.next()?;
                        break;
                    }
                };
                if kr.timestamp > self.timestamp
                    || compare_bytes(kr.key, &target_key) != Ordering::Equal
                {
                    self.cursor.next()?;
                    break;
                }
            }
            // Operate on the considered value.
            // The largest of target_key less than or equal to the timestamp.
            let kvr = match self.value() {
                Some(kvr) => kvr,
                None => {
                    let zerr = ZError::new(Error::LogicError {
                        core: ErrorCore::default(),
                        context: "should be positioned at some key with a value".to_string(),
                    });
                    return Err(zerr);
                },
            };
            assert!(kvr.timestamp <= self.timestamp);
            assert!(compare_bytes(kvr.key, &target_key) == Ordering::Equal);
            // If it's not a tombstone, return the value (and skip it next time)
            // Otherwise, just skip it.
            if kvr.value.is_some() {
                self.set_skip_key();
                return Ok(());
            } else {
                self.set_skip_key();
            }
        }
    }

    fn next(&mut self) -> Result<(), ZError<Error>> {
        loop {
            self.cursor.next()?;
            let kvr = match self.value() {
                Some(kvr) => kvr,
                None => {
                    return Ok(());
                },
            };
            if kvr.timestamp < self.timestamp && kvr.value.is_none() {
                self.set_skip_key();
            } else if kvr.timestamp < self.timestamp
                && (self.skip_key.is_none()
                    || compare_bytes(self.skip_key.as_ref().unwrap(), kvr.key) != Ordering::Equal) {
                self.set_skip_key();
                return Ok(());
            }
        }
    }

    fn key(&self) -> Option<KeyRef> {
        self.cursor.key()
    }

    fn value(&self) -> Option<KeyValueRef> {
        self.cursor.value()
    }
}
