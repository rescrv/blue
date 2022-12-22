use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_int;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use super::Error;

//////////////////////////////////////////// FileHandle ////////////////////////////////////////////

pub struct FileHandle {
    file: Arc<File>,
    state: Arc<Mutex<State>>,
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        // This FileHandle and the State object.
        if Arc::strong_count(&self.file) == 2 {
            let mut state = self.state.lock().unwrap();
            state.close_file(self.file.as_raw_fd());
        }
    }
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

#[derive(Default)]
struct State {
    opening: HashSet<PathBuf>,
    files: Vec<Option<(PathBuf, Arc<File>)>>,
    names: HashMap<PathBuf, usize>,
}

impl State {
    fn close_file(&mut self, fd: c_int) {
        assert!(fd >= 0);
        let fd = fd as usize;
        if self.files.len() <= fd {
            return;
        }
        let (path, file) = match self.files[fd].take() {
            Some((path, file)) => { (path, file) },
            None => { panic!("fd={} not managed by FileManager", fd); },
        };
        assert_eq!(file.as_raw_fd(), fd as c_int);
        let names_fd = match self.names.remove(&path) {
            Some(names_fd) => { names_fd },
            None => { panic!("path={} not managed by FileManager", path.to_string_lossy()); }
        };
        assert_eq!(fd, names_fd);
    }
}

//////////////////////////////////////////// FileManager ///////////////////////////////////////////

pub struct FileManager {
    max_open_files: usize,
    state: Arc<Mutex<State>>,
    wake_opening: Condvar,
}

impl FileManager {
    pub fn new(max_open_files: usize) -> Self {
        Self {
            max_open_files,
            state: Arc::new(Mutex::new(State::default())),
            wake_opening: Condvar::new(),
        }
    }

    pub fn open(&self, path: PathBuf) -> Result<FileHandle, Error> {
        // Check if the file is opened or opening.
        {
            let mut state = self.state.lock().unwrap();
            while state.opening.contains(&path) {
                state = self.wake_opening.wait(state).unwrap();
            }
            // We have it by name
            if let Some(fd) = state.names.get(&path) {
                // Check that we won't exceed the vector's bounds.
                if state.files.len() <= *fd {
                    return Err(Error::LogicError {
                        context: "FileManager has fd that exists outside open_files".to_string(),
                    });
                };
                // Check that we haven't violated internal invariants.
                if let Some((_, file)) = &state.files[*fd] {
                    return Ok(FileHandle {
                        file: Arc::clone(file),
                        state: Arc::clone(&self.state),
                    });
                } else {
                    return Err(Error::LogicError {
                        context: "FileManager has broken names->files pointer".to_string(),
                    });
                };
            };
            // We're going to be opening a file, so make sure we won't exceed the max number of
            // files.
            if state.opening.len() + state.names.len() >= self.max_open_files {
                return Err(Error::TooManyOpenFiles {
                    limit: self.max_open_files,
                });
            }
            state.opening.insert(path.clone());
        }
        // Open the file
        let file = match File::open(path.clone()) {
            Ok(file) => { file },
            Err(e) => {
                self.cleanup(&path);
                return Err(e.into());
            }
        };
        // Check that the file descriptor is [0, usize::max_value).
        let fd: c_int = file.as_raw_fd();
        if fd < 0 {
            self.cleanup(&path);
            return Err(Error::LogicError {
                context: "valid file's file descriptor is negative".to_string(),
            });
        }
        let fd: usize = fd as usize;
        if fd >= usize::max_value() {
            self.cleanup(&path);
            return Err(Error::LogicError {
                context: "valid file's file descriptor meets or exceeds usize::max_value()".to_string(),
            });
        }
        // Setup the file as a managed file.
        let file = Arc::new(file);
        {
            let mut state = self.state.lock().expect("poisoned mutex");
            state.opening.remove(&path);
            state.names.insert(path.clone(), fd);
            if state.files.len() <= fd {
                state.files.resize(fd + 1, None);
            }
            state.files[fd] = Some((path, Arc::clone(&file)));
        }
        self.wake_opening.notify_all();
        Ok(FileHandle {
            file,
            state: Arc::clone(&self.state),
        })
    }

    fn cleanup(&self, path: &PathBuf) {
        {
            let mut state = self.state.lock().expect("poisoned mutex");
            state.opening.remove(path);
        }
        self.wake_opening.notify_all();
    }
}
