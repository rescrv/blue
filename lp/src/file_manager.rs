use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_int;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use prototk::field_types::*;

use biometrics::{click, Counter};

use hey_listen::{HeyListen, Stationary};

use clue::Trace;

use zerror::{FromIOError, ZError, ZErrorTrait};

use super::{LOGIC_ERROR, Error};

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
    pub fn path(&self) -> Result<PathBuf, ZError<Error>> {
        let fd = check_fd(self.file.as_raw_fd())?;
        let state = self.state.lock().unwrap();
        if let Some((path, _)) = &state.files[fd] {
            Trace::new("lp.open_file")
                .with_context::<stringref>("path", 1, &path.to_string_lossy())
                .with_context::<int32>("fd", 2, self.file.as_raw_fd())
                .finish();
            return Ok(path.clone());
        } else {
            LOGIC_ERROR.click();
            let zerr = ZError::new(Error::LogicError {
                context: "FileManager has broken names->files pointer".to_string(),
            })
            .with_context::<int32>("fd", 2, self.file.as_raw_fd());
            return Err(zerr);
        }
    }

    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), ZError<Error>> {
        self.file.read_exact_at(buf, offset).from_io()
           .with_context::<int32>("fd", 2, self.file.as_raw_fd())
           .with_context::<uint64>("offset", 3, offset)
           .with_context::<uint64>("amount", 4, buf.len() as u64)
    }

    pub fn size(&self) -> Result<u64, ZError<Error>> {
        Ok(self.file.metadata().from_io()?.len())
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
            Trace::new("unmanaged fd")
                .with_context::<int32>("fd", 2, fd)
                .with_context::<uint64>("self.files.len()", 3, self.files.len() as u64)
                .panic(format!("self.files.len() <= fd"));
        }
        let (path, file) = match self.files[fd as usize].take() {
            Some((path, file)) => (path, file),
            None => {
                Trace::new("unmanaged fd")
                    .with_context::<int32>("fd", 2, fd)
                    .panic(format!("self.file[{}] is None", fd));
            }
        };
        if file.as_raw_fd() != fd {
            Trace::new("mismanaged fd")
                .with_context::<int32>("fd", 2, fd)
                .with_context::<int32>("file.as_raw_fd()", 3, file.as_raw_fd())
                .panic(format!("file.as_raw_fd() != fd"));
        }
        match self.names.remove(&path) {
            Some(_) => {},
            None => {
                Trace::new("missing fd")
                    .with_context::<stringref>("path", 1, &path.to_string_lossy())
                    .panic("path missing from names map".to_string());
            }
        };
        click!("lp.file_manager.close");
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

    pub fn open(&self, path: PathBuf) -> Result<FileHandle, ZError<Error>> {
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
                    let zerr = ZError::new(Error::LogicError {
                        context: "FileManager has fd that exists outside open_files".to_string(),
                    })
                    .with_context::<uint64>("fd", 2, *fd as u64)
                    .with_context::<uint64>("state.files.len()", 3, state.files.len() as u64);
                    return Err(zerr);
                };
                // Check that we haven't violated internal invariants.
                if let Some((_, file)) = &state.files[*fd] {
                    return Ok(FileHandle {
                        file: Arc::clone(file),
                        state: Arc::clone(&self.state),
                    });
                } else {
                    LOGIC_ERROR.click();
                    let zerr = ZError::new(Error::LogicError {
                        context: "FileManager has broken names->files pointer".to_string(),
                    })
                    .with_context::<uint64>("fd", 2, *fd as u64);
                    return Err(zerr);
                };
            };
            // We're going to be opening a file, so make sure we won't exceed the max number of
            // files.
            if state.opening.len() + state.names.len() >= self.max_open_files {
                TOO_MANY_OPEN_FILES.click();
                let zerr = ZError::new(Error::TooManyOpenFiles {
                    limit: self.max_open_files,
                })
                .with_context::<uint64>("max_open_files", 1, self.max_open_files as u64)
                .with_context::<uint64>("open_files", 2, (state.opening.len() + state.names.len()) as u64);
                return Err(zerr);
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
fn check_fd(fd: c_int) -> Result<usize, ZError<Error>> {
    if fd < 0 {
        LOGIC_ERROR.click();
        let zerr = ZError::new(Error::LogicError {
            context: "valid file's file descriptor is negative".to_string(),
        })
        .with_context::<int32>("fd", 2, fd);
        return Err(zerr);
    }
    Ok(fd as usize)
}

/////////////////////////////////////////////// open ///////////////////////////////////////////////

fn open(path: PathBuf) -> Result<File, ZError<Error>> {
    // Open the file
    click!("lp.file_manager.open");
    let file = match File::open(path.clone()) {
        Ok(file) => file,
        Err(e) => {
            let zerr = ZError::new(Error::IOError { what: e }).with_context::<stringref>(
                "path",
                1,
                &path.to_string_lossy(),
            );
            return Err(zerr);
        }
    };
    check_fd(file.as_raw_fd())?;
    Ok(file)
}

/////////////////////////////////////// open_without_manager ///////////////////////////////////////

pub fn open_without_manager(path: PathBuf) -> Result<FileHandle, ZError<Error>> {
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
