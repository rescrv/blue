//! Management of open files.  Intended to re-use open files and fail when too many files are open.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::c_int;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use biometrics::Counter;

use tatl::{HeyListen, Stationary};

use super::{
    error_with_path, io_result, logic_error_file_descriptor_negative,
    logic_error_file_manager_broken_pointer, system_error_with_context,
    system_error_with_path_and_context, too_many_open_files, Error, Sst, SstMetadata, LOGIC_ERROR,
};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static FILE_MANAGER_OPEN: Counter = Counter::new("sst.file_manager.open");
static FILE_MANAGER_OPEN_WITHOUT_MANAGER: Counter =
    Counter::new("sst.file_manager.open_without_manager");
static FILE_MANAGER_CLOSE: Counter = Counter::new("sst.file_manager.close");

static TOO_MANY_OPEN_FILES: Counter = Counter::new("sst.file_manager.too_many_open_files");
static TOO_MANY_OPEN_FILES_MONITOR: Stationary =
    Stationary::new("sst.file_manager.too_many_open_files", &TOO_MANY_OPEN_FILES);

/// Register the biometrics for this module.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&FILE_MANAGER_OPEN);
    collector.register_counter(&FILE_MANAGER_OPEN_WITHOUT_MANAGER);
    collector.register_counter(&FILE_MANAGER_CLOSE);
    collector.register_counter(&TOO_MANY_OPEN_FILES);
}

/// Register the monitors for this module.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&TOO_MANY_OPEN_FILES_MONITOR);
}

//////////////////////////////////////////// FileHandle ////////////////////////////////////////////

/// A FileHandle represents an open, materialized, non-preemptable file.
#[derive(Clone, Debug)]
pub struct FileHandle {
    file: Arc<File>,
    state: Arc<Mutex<State>>,
}

impl FileHandle {
    /// Return the path associated with the file handle.
    pub fn path(&self) -> Result<PathBuf, Error> {
        let fd = check_fd(self.file.as_raw_fd())?;
        let state = self.state.lock().unwrap();
        if let Some((path, _)) = &state.files[fd] {
            Ok(path.clone())
        } else {
            LOGIC_ERROR.click();
            Err(logic_error_file_manager_broken_pointer(fd))
        }
    }

    /// Perform a read_exact_at on the file.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        self.file.read_exact_at(buf, offset).map_err(|e| {
            let context = format!(
                "read_exact_at failed: fd={}, offset={}, amount={}",
                self.file.as_raw_fd(),
                offset,
                buf.len()
            );
            if let Ok(path) = self.path() {
                system_error_with_path_and_context(e, path.to_string_lossy(), context)
            } else {
                system_error_with_context(e, context)
            }
        })
    }

    /// return the size of the file.
    pub fn size(&self) -> Result<u64, Error> {
        Ok(io_result(self.file.metadata())?.len())
    }
}

impl Seek for FileHandle {
    fn seek(&mut self, from: SeekFrom) -> Result<u64, std::io::Error> {
        self.file.seek(from)
    }
}

impl Write for FileHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.file.flush()
    }
}

impl FileExt for FileHandle {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, std::io::Error> {
        self.file.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> Result<usize, std::io::Error> {
        self.file.write_at(buf, offset)
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        // This FileHandle and the State object.
        if Arc::strong_count(&self.file) == 2 {
            state.close_file(self.file.as_raw_fd());
        }
    }
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

#[derive(Debug, Default)]
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
                panic!("self.file[{fd}] is None");
            }
        };
        if file.as_raw_fd() != fd {
            panic!("file.as_raw_fd() != fd");
        }
        match self.names.remove(&path) {
            Some(_) => {}
            None => {
                panic!("path missing from names map");
            }
        };
        FILE_MANAGER_CLOSE.click();
    }
}

//////////////////////////////////////////// FileManager ///////////////////////////////////////////

/// FileManager manages open files.
pub struct FileManager {
    max_open_files: usize,
    state: Arc<Mutex<State>>,
    wake_opening: Condvar,
}

impl FileManager {
    /// Create a new file manager that will not open more than `max_open_files`.
    pub fn new(max_open_files: usize) -> Self {
        Self {
            max_open_files,
            state: Arc::new(Mutex::new(State::default())),
            wake_opening: Condvar::new(),
        }
    }

    /// Open the given path, if allocation limits allow.
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<FileHandle, Error> {
        // TODO(rescrv): Use utf8path to avoid lossy path conversions.
        self.open_inner(path.as_ref())
            .map_err(|err| error_with_path(err, path.as_ref().to_string_lossy()))
    }

    fn open_inner(&self, path: &Path) -> Result<FileHandle, Error> {
        // Check if the file is opened or opening.
        {
            let mut state = self.state.lock().unwrap();
            while state.opening.contains(path) {
                state = self.wake_opening.wait(state).unwrap();
            }
            // We have it by name
            if let Some(fd) = state.names.get(path) {
                // Check that we won't exceed the vector's bounds.
                if state.files.len() <= *fd {
                    LOGIC_ERROR.click();
                    return Err(logic_error_file_manager_broken_pointer(*fd));
                };
                // Check that we haven't violated internal invariants.
                if let Some((_, file)) = &state.files[*fd] {
                    return Ok(FileHandle {
                        file: Arc::clone(file),
                        state: Arc::clone(&self.state),
                    });
                } else {
                    LOGIC_ERROR.click();
                    return Err(logic_error_file_manager_broken_pointer(*fd));
                };
            };
            // We're going to be opening a file, so make sure we won't exceed the max number of
            // files.
            if state.opening.len() + state.names.len() >= self.max_open_files {
                TOO_MANY_OPEN_FILES.click();
                return Err(too_many_open_files(
                    self.max_open_files,
                    state.opening.len() + state.names.len(),
                ));
            }
            state.opening.insert(path.to_path_buf());
        }
        // Open the file
        let file = match open(path.to_path_buf()) {
            Ok(file) => file,
            Err(e) => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.opening.remove(path);
                }
                self.wake_opening.notify_all();
                return Err(e);
            }
        };
        let fd = file.as_raw_fd() as usize;
        // Setup the file as a managed file.
        let file = Arc::new(file);
        let file2 = Arc::clone(&file);
        let path2 = path.to_path_buf();
        {
            let mut state = self.state.lock().unwrap();
            state.opening.remove(&path.to_path_buf());
            state.names.insert(path.to_path_buf(), fd);
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

    /// Stat the provided path, if allocation limits allow.
    pub fn stat<P: AsRef<Path>>(&self, path: P) -> Result<SstMetadata, Error> {
        // TODO(rescrv): Use utf8path to avoid lossy path conversions.
        let handle = self
            .open(path.as_ref())
            .map_err(|err| error_with_path(err, path.as_ref().to_string_lossy()))?;
        let sst = Sst::<FileHandle>::from_file_handle(handle)
            .map_err(|err| error_with_path(err, path.as_ref().to_string_lossy()))?;
        sst.metadata()
            .map_err(|err| error_with_path(err, path.as_ref().to_string_lossy()))
    }
}

///////////////////////////////////////////// check_fd /////////////////////////////////////////////

// Check that the file descriptor is [0, usize::max_value).
fn check_fd(fd: c_int) -> Result<usize, Error> {
    if fd < 0 {
        LOGIC_ERROR.click();
        return Err(logic_error_file_descriptor_negative(fd));
    }
    Ok(fd as usize)
}

/////////////////////////////////////////////// open ///////////////////////////////////////////////

fn open(path: PathBuf) -> Result<File, Error> {
    // Open the file
    FILE_MANAGER_OPEN.click();
    let file = match File::open(path.clone()) {
        Ok(file) => file,
        Err(e) => {
            return Err(system_error_with_path_and_context(
                e,
                path.to_string_lossy(),
                "file open failed",
            ));
        }
    };
    check_fd(file.as_raw_fd())?;
    Ok(file)
}

/////////////////////////////////////// open_without_manager ///////////////////////////////////////

/// Open a file handle without caring about the number of open files.
pub fn open_without_manager<P: AsRef<Path>>(path: P) -> Result<FileHandle, Error> {
    let path = path.as_ref().to_path_buf();
    FILE_MANAGER_OPEN_WITHOUT_MANAGER.click();
    // TODO(rescrv): Use utf8path to avoid lossy path conversions.
    let file =
        Arc::new(open(path.clone()).map_err(|err| error_with_path(err, path.to_string_lossy()))?);
    let fd = file.as_raw_fd() as usize;
    assert!(fd < usize::MAX);
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
