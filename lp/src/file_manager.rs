use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_int;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use prototk::field_types::*;

use biometrics::Counter;

use hey_listen::{HeyListen, Stationary};

use zerror::Z;
use zerror_core::ErrorCore;

use super::{LOGIC_ERROR, Error, FromIO};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static TOO_MANY_OPEN_FILES: Counter = Counter::new("lp.file_manager.too_many_open_files");
static TOO_MANY_OPEN_FILES_MONITOR: Stationary = Stationary::new("lp.file_manager.too_many_open_files", &TOO_MANY_OPEN_FILES);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&TOO_MANY_OPEN_FILES_MONITOR);
}

//////////////////////////////////////////// FileHandle ////////////////////////////////////////////

#[derive(Clone)]
pub struct FileHandle {
    file: Arc<File>,
    state: Arc<Mutex<State>>,
}

impl FileHandle {
    pub fn path(&self) -> Result<PathBuf, Error> {
        let fd = check_fd(self.file.as_raw_fd())?;
        let state = self.state.lock().unwrap();
        if let Some((path, _)) = &state.files[fd] {
            return Ok(path.clone());
        } else {
            LOGIC_ERROR.click();
            let err = Error::LogicError {
                core: ErrorCore::default(),
                context: "FileManager has broken names->files pointer".to_string(),
            }
            .with_variable("fd", self.file.as_raw_fd());
            return Err(err);
        }
    }

    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        self.file.read_exact_at(buf, offset).from_io()
            .with_variable("fd", self.file.as_raw_fd())
            .with_variable("offset", offset)
            .with_variable("amount", buf.len())
    }

    pub fn size(&self) -> Result<u64, Error> {
        Ok(self.file.metadata()?.len())
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        // This FileHandle and the State object.
        if Arc::strong_count(&self.file) == 2 {
            let mut state = self.state.lock().unwrap();
            if Arc::strong_count(&self.file) == 2 {
                state.close_file(self.file.as_raw_fd());
            }
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
        if self.files.len() <= fd as usize {
            panic!("self.files.len() <= fd");
        }
        let (path, file) = match self.files[fd as usize].take() {
            Some((path, file)) => (path, file),
            None => {
                panic!("self.file[{}] is None", fd);
            }
        };
        if file.as_raw_fd() != fd {
            panic!("file.as_raw_fd() != fd");
        }
        match self.names.remove(&path) {
            Some(_) => {},
            None => {
                panic!("path missing from names map");
            }
        };
        // TODO(rescrv) click!("lp.file_manager.close");
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
                    LOGIC_ERROR.click();
                    let err = Error::LogicError {
                        core: ErrorCore::default(),
                        context: "FileManager has fd that exists outside open_files".to_string(),
                    }
                    .with_variable("fd", *fd)
                    .with_variable("state.files.len()", state.files.len());
                    return Err(err);
                };
                // Check that we haven't violated internal invariants.
                if let Some((_, file)) = &state.files[*fd] {
                    return Ok(FileHandle {
                        file: Arc::clone(file),
                        state: Arc::clone(&self.state),
                    });
                } else {
                    LOGIC_ERROR.click();
                    let err = Error::LogicError {
                        core: ErrorCore::default(),
                        context: "FileManager has broken names->files pointer".to_string(),
                    }
                    .with_variable("fd", *fd);
                    return Err(err);
                };
            };
            // We're going to be opening a file, so make sure we won't exceed the max number of
            // files.
            if state.opening.len() + state.names.len() >= self.max_open_files {
                TOO_MANY_OPEN_FILES.click();
                let err = Error::TooManyOpenFiles {
                    core: ErrorCore::default(),
                    limit: self.max_open_files,
                }
                .with_variable("max_open_files", self.max_open_files)
                .with_variable("open_files", state.opening.len() + state.names.len());
                return Err(err);
            }
            state.opening.insert(path.clone());
        }
        // Open the file
        let file = match open(path.clone()) {
            Ok(file) => file,
            Err(e) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.opening.remove(&path);
                }
                self.wake_opening.notify_all();
                return Err(e);
            }
        };
        let fd = file.as_raw_fd() as usize;
        // Setup the file as a managed file.
        let file = Arc::new(file);
        let file2 = Arc::clone(&file);
        let path2 = path.clone();
        {
            let mut state = self.state.lock().unwrap();
            state.opening.remove(&path);
            state.names.insert(path, fd);
            if state.files.len() <= fd {
                state.files.resize(fd + 1, None);
            }
            state.files[fd] = Some((path2, file2));
        }
        self.wake_opening.notify_all();
        Ok(FileHandle {
            file,
            state: Arc::clone(&self.state),
        })
    }
}

///////////////////////////////////////////// check_fd /////////////////////////////////////////////

// Check that the file descriptor is [0, usize::max_value).
fn check_fd(fd: c_int) -> Result<usize, Error> {
    if fd < 0 {
        LOGIC_ERROR.click();
        let err = Error::LogicError {
            core: ErrorCore::default(),
            context: "valid file's file descriptor is negative".to_string(),
        }
        .with_variable("fd", fd);
        return Err(err);
    }
    Ok(fd as usize)
}

/////////////////////////////////////////////// open ///////////////////////////////////////////////

fn open(path: PathBuf) -> Result<File, Error> {
    // Open the file
    // TODO(rescrv) click!("lp.file_manager.open");
    let file = match File::open(path.clone()) {
        Ok(file) => file,
        Err(e) => {
            let err = Error::IOError {
                core: ErrorCore::default(),
                what: e
            }
            .with_variable("path", path.to_string_lossy());
            return Err(err);
        }
    };
    check_fd(file.as_raw_fd())?;
    Ok(file)
}

/////////////////////////////////////// open_without_manager ///////////////////////////////////////

pub fn open_without_manager(path: PathBuf) -> Result<FileHandle, Error> {
    let file = Arc::new(open(path.clone())?);
    let fd = file.as_raw_fd() as usize;
    assert!(fd < usize::max_value());
    let mut state = State {
        opening: HashSet::new(),
        files: Vec::with_capacity(fd + 1),
        names: HashMap::new(),
    };
    state.names.insert(path.clone(), fd);
    state.files.resize(fd + 1, None);
    state.files[fd] = Some((path, Arc::clone(&file)));
    let state = Arc::new(Mutex::new(state));
    Ok(FileHandle { file, state })
}
