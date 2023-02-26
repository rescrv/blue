use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file, File};
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use util::lockfile::Lockfile;
use util::time::now;

use buffertk::{stack_pack, Unpacker};

use prototk::field_types::*;

use biometrics::Counter;

use hey_listen::{HeyListen, Stationary};

use clue::Trace;

use zerror::{ErrorCore, Z};

use super::file_manager::FileManager;
use super::merging_cursor::MergingCursor;
use super::options::CompactionOptions;
use super::pruning_cursor::PruningCursor;
use super::setsum::Setsum;
use super::sst::{SSTBuilder, SSTMetadata, SST};
use super::{compare_bytes, Builder, Cursor, Error, FromIO};

pub mod compaction;
pub use compaction::Compaction;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_NOT_OBTAINED: Counter = Counter::new("lp.db.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lp.db.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

///////////////////////////////////////////// constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn LOCKFILE<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("LOCKFILE")
}

/////////////////////////////////////////// ErrorHandler ///////////////////////////////////////////

trait ErrorHandler {
    fn error(&mut self, err: Error) -> Result<(), Error>;
    fn warning(&mut self, err: Error) -> Result<(), Error>;
}

#[derive(Default)]
struct NormalErrorHandler {}

impl ErrorHandler for NormalErrorHandler {
    fn error(&mut self, err: Error) -> Result<(), Error> {
        Err(err)
    }

    fn warning(&mut self, _: Error) -> Result<(), Error> {
        Ok(())
    }
}

#[derive(Default)]
struct ParanoidErrorHandler {}

impl ErrorHandler for ParanoidErrorHandler {
    fn error(&mut self, err: Error) -> Result<(), Error> {
        Err(err)
    }

    fn warning(&mut self, err: Error) -> Result<(), Error> {
        Err(err)
    }
}

#[derive(Default)]
struct FsckErrorHandler {
    errors: Vec<Error>,
}

impl ErrorHandler for FsckErrorHandler {
    fn error(&mut self, err: Error) -> Result<(), Error> {
        self.errors.push(err);
        Ok(())
    }

    fn warning(&mut self, err: Error) -> Result<(), Error> {
        self.errors.push(err);
        Ok(())
    }
}

/////////////////////////////////////////// get_lockfile ///////////////////////////////////////////

pub fn get_lockfile(options: &DBOptions, root: &PathBuf) -> Result<Lockfile, Error> {
    // Deal with making the root directory.
    if root.is_dir() && options.error_if_exists {
        return Err(Error::DBExists { core: ErrorCore::default(), path: root.clone() });
    }
    if !root.is_dir() && !options.create_if_missing {
        return Err(Error::DBNotExist { core: ErrorCore::default(), path: root.clone() });
    } else if !root.is_dir() {
        Trace::new("lp.db.create")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        create_dir(&root)
            .from_io()
            .with_variable("root", root.to_string_lossy())?;
    }
    // Deal with the lockfile first.
    let lockfile = if options.wait_for_lock {
        Trace::new("lp.db.wait_lockfile")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        Lockfile::wait(LOCKFILE(&root))
            .from_io()
            .with_variable("root", root.to_string_lossy())?
    } else {
        Trace::new("lp.db.nowait_lockfile")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        Lockfile::lock(LOCKFILE(&root))
            .from_io()
            .with_variable("root", root.to_string_lossy())?
    };
    if lockfile.is_none() {
        LOCK_NOT_OBTAINED.click();
        let err = Error::LockNotObtained {
            core: ErrorCore::default(),
            path: LOCKFILE(root),
        };
        return Err(err);
    }
    Trace::new("lp.db.lock_obtained")
        .with_context::<string, 1>("root", &root.to_string_lossy())
        .finish();
    Ok(lockfile.unwrap())
}

///////////////////////////////////////////// DBOptions ////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct DBOptions {
    // TODO(rescrv): Unify options feel and make these public.
    create_if_missing: bool,
    error_if_exists: bool,
    paranoid_checks: bool,
    wait_for_lock: bool,
    max_open_files: usize,
    pub compaction: CompactionOptions,
}

impl Default for DBOptions {
    fn default() -> Self {
        Self {
            create_if_missing: true,
            error_if_exists: false,
            paranoid_checks: true,
            wait_for_lock: true,
            max_open_files: 1 << 20,
            compaction: CompactionOptions::default(),
        }
    }
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

#[derive(Default)]
struct State {
    sst_metadata: Vec<SSTMetadata>,
}

impl State {
    fn get_metadata_by_setsum(&self, setsum: &str) -> Option<SSTMetadata> {
        for sst in self.sst_metadata.iter() {
            if sst.setsum() == setsum {
                return Some(sst.clone())
            }
        }
        None
    }
}

//////////////////////////////////////////////// DB ////////////////////////////////////////////////

pub struct DB {
    root: PathBuf,
    options: DBOptions,
    file_manager: FileManager,
    state: Mutex<Arc<State>>,
    _lockfile: Lockfile,
}

impl DB {
    #[allow(non_snake_case)]
    fn SST<P: AsRef<Path>>(root: P) -> PathBuf {
        root.as_ref().to_path_buf().join("sst")
    }

    #[allow(non_snake_case)]
    fn META<P: AsRef<Path>>(root: P) -> PathBuf {
        root.as_ref().to_path_buf().join("meta")
    }

    pub fn open<P: AsRef<Path>>(options: DBOptions, root: P) -> Result<DB, Error> {
        let root: PathBuf = root
            .as_ref()
            .canonicalize()
            .from_io()
            .with_variable("root", root.as_ref().to_string_lossy())?;
        let lockfile = get_lockfile(&options, &root)?;
        let file_manager = FileManager::new(options.max_open_files);
        // Create the correct directories, or at least make sure they exist.
        if !Self::SST(&root).is_dir() {
            create_dir(Self::SST(&root))
                .from_io()
                .with_variable("sst", Self::SST(&root))?;
        }
        if !root.join("meta").is_dir() {
            create_dir(Self::META(&root))
                .from_io()
                .with_variable("meta", Self::META(&root))?;
        }
        // DB.
        Trace::new("lp.db.open")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        let db = Self {
            root: root,
            options,
            file_manager,
            state: Mutex::new(Arc::new(State::default())),
            _lockfile: lockfile,
        };
        db.reload()?;
        Ok(db)
    }

    pub fn fsck<P: AsRef<Path>>(options: DBOptions, root: P) -> Vec<Error> {
        todo!();
    }

    pub fn ingest(&self, paths: &[PathBuf]) -> Result<(), Error> {
        // Make a directory into which all files will be linked.
        let ingest_time = now::millis();
        let ingest_root = self.root.join(format!("ingest:{}", ingest_time));
        create_dir(&ingest_root).from_io()?;
        let mut ssts = Vec::new();
        // For each SST, hardlink it into the ingest root.
        for path in paths {
            let file = self.file_manager.open(path.clone())?;
            let sst = SST::from_file_handle(file)?;
            // Extract the metadata.
            let setsum_str = sst.setsum();
            let metadata = sst.metadata()?;
            // Hard-link the file into place.
            let filename = setsum_str.clone() + ".sst";
            let target = ingest_root.join(&filename);
            hard_link(path, target).from_io()?;
            ssts.push((setsum_str, filename, metadata));
        }
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        ssts.sort_by(|lhs, rhs| compare_bytes(lhs.0.as_bytes(), rhs.0.as_bytes()));
        let meta_basename = format!("meta.{}.sst", ingest_time);
        let mut meta = SSTBuilder::new(
            ingest_root.join(&meta_basename),
            self.options.compaction.sst_options.clone(),
        )?;
        for (setsum, _, metadata) in ssts.iter() {
            let pa = stack_pack(metadata);
            meta.put(setsum.as_bytes(), ingest_time as u64, &pa.to_vec())?;
        }
        let meta = meta.seal()?;
        // Now that everything's been put in the ingest root, link into the sst dir.
        for (_, sst, _) in ssts.iter() {
            let ingested = ingest_root.join(&sst);
            let target = self.root.join("sst").join(&sst);
            // Intentionally error if the hard_link already exists.
            let err = hard_link(ingested, target);
            match err {
                Ok(_) => {},
                Err(err) => {
                    if err.kind() == ErrorKind::AlreadyExists {
                        return Err(Error::DuplicateSST { core: ErrorCore::default(), what: sst.clone() });
                    } else {
                        return Err(err.into());
                    }
                },
            }
        }
        // Move the metadata
        let meta_path =
            self.root
                .join("meta")
                .join(&format!("{}.{}.sst", meta.setsum(), ingest_time));
        hard_link(ingest_root.join(&meta_basename), meta_path).from_io()?;
        // Unlink the ingest directory last.
        for (_, sst, _) in ssts.iter() {
            remove_file(ingest_root.join(&sst)).from_io()?;
        }
        remove_file(ingest_root.join(&meta_basename)).from_io()?;
        remove_dir(ingest_root).from_io()?;
        self.reload()
    }

    pub fn reload(&self) -> Result<(), Error> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut state = self.state.lock().unwrap();
        let mut cursors: Vec<Box<dyn Cursor>> = Vec::new();
        for meta in read_dir(self.root.join("meta")).from_io()? {
            let meta = meta.from_io()?;
            let file = self.file_manager.open(meta.path())?;
            let sst = SST::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let cursor = MergingCursor::new(cursors)?;
        let mut cursor = PruningCursor::new(cursor, u64::max_value())?;
        cursor.seek_to_first()?;
        let mut sst_metadata = Vec::new();
        loop {
            cursor.next()?;
            let value = match cursor.value() {
                Some(v) => v,
                None => {
                    break;
                }
            };
            let mut up = Unpacker::new(value.value.unwrap_or(&[]));
            let metadata: SSTMetadata = up.unpack().map_err(|_| {
                Error::Corruption {
                    core: ErrorCore::default(),
                    context: "key is corrupted in metadata".to_string(),
                }
                .with_variable("key", value.key)
            })?;
            sst_metadata.push(metadata);
        }
        *state = Arc::new(State {
            sst_metadata,
        });
        Ok(())
    }

    pub fn suggest_compactions(&self) -> Result<Vec<Compaction>, Error> {
        let state = self.get_state();
        let graph = compaction::Graph::new(self.options.compaction.clone(), &state.sst_metadata)?;
        let mut compactions = graph.compactions();
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        Ok(compactions)
    }

    pub fn compaction_setup(&self, options: CompactionOptions, inputs: &[&str], smallest_snapshot: u64) -> Result<Compaction, Error> {
        let state = self.get_state();
        let mut setsums = Vec::new();
        for input in inputs.into_iter() {
            if let Some(metadata) = state.get_metadata_by_setsum(input) {
                setsums.push(metadata);
            } else {
                return Err(Error::SSTNotFound {
                    core: ErrorCore::default(),
                    setsum: input.to_string(),
                });
            }
        }
        Ok(Compaction::from_inputs(options, setsums, smallest_snapshot))
    }

    fn get_state(&self) -> Arc<State> {
        let state = self.state.lock().unwrap();
        Arc::clone(&state)
    }
}
