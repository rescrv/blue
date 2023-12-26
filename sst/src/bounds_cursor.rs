use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::Bound;

use keyvalint::{Cursor, KeyRef};

use super::{compare_bytes, Error};

////////////////////////////////////////////// Bounds //////////////////////////////////////////////

#[derive(Eq, PartialEq)]
enum Bounds {
    BeforeStart,
    Positioned,
    AfterEnd,
}

/////////////////////////////////////////// BoundsCursor ///////////////////////////////////////////

pub struct BoundsCursor<C: Cursor, E: Debug + From<Error>> {
    cursor: C,
    bounds: Bounds,
    // TODO(rescrv): I don't like that I have to allocate here.
    start_bound: Bound<Vec<u8>>,
    end_bound: Bound<Vec<u8>>,
    _phantom_e: std::marker::PhantomData<E>,
}

impl<E: Debug + From<Error>, C: Cursor<Error = E>> BoundsCursor<C, E> {
    pub fn new<T: AsRef<[u8]>>(
        cursor: C,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
    ) -> Result<Self, E> {
        fn as_ref_to_vec<U: AsRef<[u8]>>(b: &Bound<U>) -> Bound<Vec<u8>> {
            match b {
                Bound::Included(key) => Bound::Included(key.as_ref().to_vec()),
                Bound::Excluded(key) => Bound::Excluded(key.as_ref().to_vec()),
                Bound::Unbounded => Bound::Unbounded,
            }
        }
        let start_bound = as_ref_to_vec(start_bound);
        let end_bound = as_ref_to_vec(end_bound);
        let mut cursor = Self {
            cursor,
            bounds: Bounds::BeforeStart,
            start_bound,
            end_bound,
            _phantom_e: std::marker::PhantomData,
        };
        cursor.seek_to_first()?;
        Ok(cursor)
    }
}

impl<E: Debug + From<Error>, C: Cursor<Error = E>> BoundsCursor<C, E> {
    fn check_for_start_bound_exceeded(&mut self) {
        match (self.key(), &self.start_bound) {
            (Some(_), Bound::Unbounded) => {}
            (Some(kr), Bound::Included(key)) => {
                if compare_bytes(kr.key, key).is_lt() {
                    self.bounds = Bounds::BeforeStart;
                }
            }
            (Some(kr), Bound::Excluded(key)) => {
                if compare_bytes(kr.key, key).is_le() {
                    self.bounds = Bounds::BeforeStart;
                }
            }
            (None, _) => {}
        }
    }

    fn check_for_end_bound_exceeded(&mut self) {
        match (self.key(), &self.end_bound) {
            (Some(_), Bound::Unbounded) => {}
            (Some(kr), Bound::Included(key)) => {
                if compare_bytes(kr.key, key).is_gt() {
                    self.bounds = Bounds::AfterEnd;
                }
            }
            (Some(kr), Bound::Excluded(key)) => {
                if compare_bytes(kr.key, key).is_ge() {
                    self.bounds = Bounds::AfterEnd;
                }
            }
            (None, _) => {}
        }
    }
}

impl<E: Debug + From<Error>, C: Cursor<Error = E>> Cursor for BoundsCursor<C, E> {
    type Error = E;

    fn seek_to_first(&mut self) -> Result<(), E> {
        match &self.start_bound {
            Bound::Unbounded => {
                self.bounds = Bounds::BeforeStart;
                self.cursor.seek_to_first()?;
            }
            Bound::Included(start_bound) => {
                self.bounds = Bounds::BeforeStart;
                self.cursor.seek(start_bound)?;
                if self.cursor.key().is_some() {
                    self.cursor.prev()?;
                }
            }
            Bound::Excluded(start_bound) => {
                self.bounds = Bounds::BeforeStart;
                self.cursor.seek(start_bound)?;
            }
        }
        self.check_for_end_bound_exceeded();
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), E> {
        match &self.end_bound {
            Bound::Unbounded => {
                self.bounds = Bounds::AfterEnd;
                self.cursor.seek_to_last()?;
            }
            Bound::Included(end_bound) => {
                self.bounds = Bounds::AfterEnd;
                self.cursor.seek(end_bound)?;
                while let Some(key) = self.cursor.key() {
                    if compare_bytes(key.key, end_bound) == Ordering::Equal {
                        self.cursor.next()?;
                    } else {
                        break;
                    }
                }
            }
            Bound::Excluded(end_bound) => {
                self.bounds = Bounds::AfterEnd;
                self.cursor.seek(end_bound)?;
            }
        }
        self.check_for_start_bound_exceeded();
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), E> {
        self.bounds = Bounds::Positioned;
        self.cursor.seek(key)?;
        self.check_for_end_bound_exceeded();
        self.check_for_start_bound_exceeded();
        if self.bounds == Bounds::BeforeStart {
            self.seek_to_first()?;
            self.next()?;
        }
        Ok(())
    }

    fn prev(&mut self) -> Result<(), E> {
        if self.bounds != Bounds::BeforeStart {
            self.cursor.prev()?;
            self.bounds = Bounds::Positioned;
        }
        self.check_for_start_bound_exceeded();
        Ok(())
    }

    fn next(&mut self) -> Result<(), E> {
        if self.bounds != Bounds::AfterEnd {
            self.cursor.next()?;
            self.bounds = Bounds::Positioned;
        }
        self.check_for_end_bound_exceeded();
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        if self.bounds == Bounds::Positioned {
            self.cursor.key()
        } else {
            None
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if self.bounds == Bounds::Positioned {
            self.cursor.value()
        } else {
            None
        }
    }
}
