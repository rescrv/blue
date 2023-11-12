use std::path::{Path, PathBuf};

use super::file_manager::FileManager;
use super::{Cursor, Error, KeyRef, KeyValueRef, Sst, SstCursor};

///////////////////////////////////////////// Position /////////////////////////////////////////////

#[allow(clippy::large_enum_variant)]
enum Position {
    First,
    Last,
    Instantiated { cursor: SstCursor },
}

//////////////////////////////////////////// LazyCursor ////////////////////////////////////////////

pub struct LazyCursor<FM: AsRef<FileManager>> {
    file_manager: FM,
    path: PathBuf,
    position: Position,
}

impl<FM: AsRef<FileManager>> LazyCursor<FM> {
    pub fn new<P: AsRef<Path>>(file_manager: FM, path: P) -> Self {
        Self {
            file_manager,
            path: path.as_ref().to_path_buf(),
            position: Position::First,
        }
    }

    fn establish_cursor(&mut self) -> Result<&mut SstCursor, Error> {
        let path = self.path.clone();
        let handle = self.file_manager.as_ref().open(path)?;
        let sst = Sst::from_file_handle(handle)?;
        let cursor = sst.cursor();
        self.position = Position::Instantiated { cursor };
        if let Position::Instantiated { ref mut cursor } = &mut self.position {
            Ok(cursor)
        } else {
            panic!("this should never happen");
        }
    }
}

impl<FM: AsRef<FileManager>> Cursor for LazyCursor<FM> {
    fn reset(&mut self) -> Result<(), Error> {
        self.position = Position::First;
        Ok(())
    }

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
                cursor.prev()?;
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

    fn value(&self) -> Option<KeyValueRef> {
        if let Position::Instantiated { cursor } = &self.position {
            cursor.value()
        } else {
            None
        }
    }
}
