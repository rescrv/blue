//! Lazy cursoring, so we can limit the number of files open at once.

use super::{Cursor, Error, KeyRef, SstCursor};

///////////////////////////////////////////// Position /////////////////////////////////////////////

#[allow(clippy::large_enum_variant)]
enum Position {
    First,
    Last,
    Instantiated { cursor: SstCursor },
}

//////////////////////////////////////////// LazyCursor ////////////////////////////////////////////

/// A LazyCursor instantiates its contents lazily, one file at a time.
pub struct LazyCursor<F: FnMut() -> Result<SstCursor, Error>> {
    position: Position,
    instantiate: F,
}

impl<F: FnMut() -> Result<SstCursor, Error>> LazyCursor<F> {
    /// Create a new LazyCursor.
    pub fn new(instantiate: F) -> Self {
        Self {
            position: Position::First,
            instantiate,
        }
    }

    fn establish_cursor(&mut self) -> Result<&mut SstCursor, Error> {
        let cursor = (self.instantiate)()?;
        self.position = Position::Instantiated { cursor };
        if let Position::Instantiated { ref mut cursor } = &mut self.position {
            Ok(cursor)
        } else {
            panic!("this should never happen");
        }
    }
}

impl<F: FnMut() -> Result<SstCursor, Error>> Cursor for LazyCursor<F> {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.position = Position::First;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.position = Position::Last;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        let cursor = match &mut self.position {
            Position::First => self.establish_cursor()?,
            Position::Last => self.establish_cursor()?,
            Position::Instantiated { cursor } => cursor,
        };
        cursor.seek(key)?;
        if cursor.key().is_none() {
            self.position = Position::Last;
        }
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        match &mut self.position {
            Position::First => {}
            Position::Last => {
                let cursor = self.establish_cursor()?;
                cursor.seek_to_last()?;
                cursor.prev()?;
                if cursor.key().is_none() {
                    self.position = Position::First;
                }
            }
            Position::Instantiated { cursor } => {
                cursor.prev()?;
                if cursor.key().is_none() {
                    self.position = Position::First;
                }
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), Error> {
        match &mut self.position {
            Position::First => {
                let cursor = self.establish_cursor()?;
                cursor.seek_to_first()?;
                cursor.next()?;
                if cursor.key().is_none() {
                    self.position = Position::Last;
                }
            }
            Position::Last => {}
            Position::Instantiated { cursor } => {
                cursor.next()?;
                if cursor.key().is_none() {
                    self.position = Position::Last;
                }
            }
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        if let Position::Instantiated { cursor } = &self.position {
            cursor.key()
        } else {
            None
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if let Position::Instantiated { cursor } = &self.position {
            cursor.value()
        } else {
            None
        }
    }
}
