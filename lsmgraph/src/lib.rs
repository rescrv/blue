use std::cmp::{max, min, Ordering};
use std::fmt::{Debug, Display, Formatter};
use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file, rename};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arrrg_derive::CommandLine;

use biometrics::Counter;

use buffertk::{stack_pack, Buffer};

use one_two_eight::{generate_id, generate_id_prototk};

use setsum::Setsum;

use sst::file_manager::FileManager;
use sst::merging_cursor::MergingCursor;
use sst::{compare_bytes, Builder, Cursor, Sst, SstBuilder, SstMetadata, SstMultiBuilder, SstOptions};

use tatl::{Stationary, HeyListen};

use tuple_key::TupleKey;

use utilz::lockfile::Lockfile;
use utilz::time::now;

use zerror::Z;

use zerror_core::ErrorCore;

mod graph;

use graph::Graph;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn LOCKFILE<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("LOCKFILE")
}

#[allow(non_snake_case)]
fn SST_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("sst")
}

#[allow(non_snake_case)]
fn SST_FILE<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    SST_ROOT(root).join(setsum + ".sst")
}

#[allow(non_snake_case)]
fn META_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("meta")
}

#[allow(non_snake_case)]
fn META_FILE<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    META_ROOT(root).join("meta.".to_owned() + &setsum + ".sst")
}

#[allow(non_snake_case)]
fn COMPACTION_ROOT<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    root.as_ref().to_path_buf().join(setsum + ".sst")
}

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!(MetaID, "meta:");
generate_id_prototk!(MetaID);

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_OBTAINED: Counter = Counter::new("lsmtk.lock_obtained");

static LOCK_NOT_OBTAINED: Counter = Counter::new("lsmtk.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lsmtk.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

/////////////////////////////////////////// get_lockfile ///////////////////////////////////////////

pub fn get_lockfile(options: &LsmOptions, root: &PathBuf) -> Result<Lockfile, Error> {
    // Deal with making the root directory.
    if root.is_dir() && options.fail_if_exists {
        return Err(Error::DbExists { core: ErrorCore::default(), path: root.clone() });
    }
    if !root.is_dir() && options.fail_if_not_exist {
        return Err(Error::DbNotExist { core: ErrorCore::default(), path: root.clone() });
    } else if !root.is_dir() {
        create_dir(root)
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?;
    }
    // Deal with the lockfile first.
    let lockfile = if options.fail_if_locked {
        Lockfile::lock(LOCKFILE(root))
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?
    } else {
        Lockfile::wait(LOCKFILE(root))
            .map_io_err()
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
    LOCK_OBTAINED.click();
    Ok(lockfile.unwrap())
}


/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Error {
    KeyTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    ValueTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    SortOrder {
        core: ErrorCore,
        last_key: Vec<u8>,
        last_timestamp: u64,
        new_key: Vec<u8>,
        new_timestamp: u64,
    },
    TableFull {
        core: ErrorCore,
        size: usize,
        limit: usize,
    },
    BlockTooSmall {
        core: ErrorCore,
        length: usize,
        required: usize,
    },
    UnpackError {
        core: ErrorCore,
        error: prototk::Error,
        context: String,
    },
    Crc32cFailure {
        core: ErrorCore,
        start: u64,
        limit: u64,
        crc32c: u32,
    },
    Corruption {
        core: ErrorCore,
        context: String,
    },
    LogicError {
        core: ErrorCore,
        context: String,
    },
    SystemError {
        core: ErrorCore,
        what: String,
    },
    TooManyOpenFiles {
        core: ErrorCore,
        limit: usize,
    },
    LockNotObtained {
        core: ErrorCore,
        path: PathBuf,
    },
    DuplicateSst {
        core: ErrorCore,
        what: String,
    },
    SstNotFound {
        core: ErrorCore,
        setsum: String,
    },
    DbExists {
        core: ErrorCore,
        path: PathBuf,
    },
    DbNotExist {
        core: ErrorCore,
        path: PathBuf,
    },
    PathError {
        core: ErrorCore,
        path: PathBuf,
        what: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSst { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SstNotFound { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSst { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SstNotFound { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.core_mut().set_token(identifier, value);
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.core_mut().set_url(identifier, url);
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.core_mut().set_variable(variable, x);
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv):  Make sure this isn't infinitely co-recursive with long_form
        write!(fmt, "{}", self.long_form())
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError { core: ErrorCore::default(), what: what.to_string() }
    }
}

impl From<sst::Error> for Error {
    fn from(what: sst::Error) -> Error {
        match what {
            sst::Error::KeyTooLarge { core, length, limit } => Error::KeyTooLarge { core, length, limit },
            sst::Error::ValueTooLarge { core, length, limit } => Error::ValueTooLarge { core, length, limit },
            sst::Error::SortOrder { core, last_key, last_timestamp, new_key, new_timestamp } => Error::SortOrder { core, last_key, last_timestamp, new_key, new_timestamp },
            sst::Error::TableFull { core, size, limit } => Error::TableFull { core, size, limit },
            sst::Error::BlockTooSmall { core, length, required } => Error::BlockTooSmall { core, length, required },
            sst::Error::UnpackError { core, error, context } => Error::UnpackError { core, error, context },
            sst::Error::Crc32cFailure { core, start, limit, crc32c } => Error::Crc32cFailure { core, start, limit, crc32c },
            sst::Error::Corruption { core, context } => Error::Corruption { core, context },
            sst::Error::LogicError { core, context } => Error::LogicError { core, context },
            sst::Error::SystemError { core, what } => Error::SystemError { core, what },
            sst::Error::TooManyOpenFiles { core, limit } => Error::TooManyOpenFiles { core, limit },
        }
    }
}

//////////////////////////////////////////// MapIoError ////////////////////////////////////////////

pub trait MapIoError {
    type Result;

    fn map_io_err(self) -> Self::Result;
}

impl<T> MapIoError for Result<T, std::io::Error> {
    type Result = Result<T, Error>;

    fn map_io_err(self) -> Self::Result {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }
}

//////////////////////////////////////////// LsmOptions ////////////////////////////////////////////

#[derive(CommandLine, Clone, Debug, Eq, PartialEq)]
pub struct LsmOptions {
    #[arrrg(flag, "Create the graph if it does not exist.")]
    fail_if_not_exist: bool,
    #[arrrg(flag, "Exit with an error if the graph exists.")]
    fail_if_exists: bool,
    #[arrrg(flag, "Block waiting for the lock.")]
    fail_if_locked: bool,
    #[arrrg(optional, "Maximum number of files to open", "FILES")]
    max_open_files: usize,
    #[arrrg(nested)]
    sst: SstOptions,
    #[arrrg(required, "Root path for the lsmgraph", "PATH")]
    path: String,
    #[arrrg(optional, "Root Table's 16B identifier", "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX")]
    meta_id: MetaID,
    #[arrrg(optional, "Maximum number of bytes permitted in a compaction", "BYTES")]
    max_compaction_bytes: usize,
}

impl LsmOptions {
    pub fn open(&self) -> Result<DB, Error> {
        let root: PathBuf = PathBuf::from(&self.path);
        let lockfile = get_lockfile(self, &root)?;
        let root: PathBuf = root
            .canonicalize()
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?;
        let file_manager = Arc::new(FileManager::new(self.max_open_files));
        let file_manager_p = Arc::clone(&file_manager);
        // Create the correct directories, or at least make sure they exist.
        if !META_ROOT(&self.path).is_dir() {
            create_dir(META_ROOT(&self.path))
                .map_io_err()
                .with_variable("meta", META_ROOT(&self.path))?;
        }
        if !SST_ROOT(&self.path).is_dir() {
            create_dir(SST_ROOT(&self.path))
                .map_io_err()
                .with_variable("sst", SST_ROOT(&self.path))?;
        }
        if !SST_ROOT(&self.path).is_dir() {
            create_dir(SST_ROOT(&self.path))
                .map_io_err()
                .with_variable("sst", SST_ROOT(&self.path))?;
        }
        let metadata = Mutex::new(Metadata::new(&self.path, file_manager_p)?);
        let lsm_graph = DB {
            root,
            options: self.clone(),
            file_manager,
            metadata,
            _lockfile: lockfile,
        };
        lsm_graph.reload()?;
        Ok(lsm_graph)
    }
}

impl Default for LsmOptions {
    fn default() -> Self {
        Self {
            fail_if_not_exist: false,
            fail_if_exists: false,
            fail_if_locked: false,
            max_open_files: 1 << 20,
            sst: SstOptions::default(),
            path: "db".to_owned(),
            meta_id: MetaID::from_human_readable("meta:2482d311-f68a-4da6-bfc1-f65b2db7ca99").unwrap(),
            max_compaction_bytes: usize::max_value(),
        }
    }
}

///////////////////////////////////////////// Metadata /////////////////////////////////////////////

struct Metadata {
    root: PathBuf,
    file_manager: Arc<FileManager>,

    meta: Vec<SstMetadata>,
    data: Vec<SstMetadata>,
}

impl Metadata {
    fn new<P: AsRef<Path>>(root: P, file_manager: Arc<FileManager>) -> Result<Arc<Self>, Error> {
        let md = Self {
            root: root.as_ref().to_path_buf(),
            file_manager,
            meta: Vec::new(),
            data: Vec::new(),
        };
        md.reload()
    }

    fn reload(&self) -> Result<Arc<Self>, Error> {
        let mut md = Self {
            root: self.root.clone(),
            file_manager: Arc::clone(&self.file_manager),
            meta: Vec::new(),
            data: Vec::new(),
        };
        for file in read_dir(META_ROOT(&self.root))? {
            let file = self.file_manager.open(file?.path())?;
            let sst = Sst::from_file_handle(file)?;
            md.meta.push(sst.metadata()?);
        }
        for file in read_dir(SST_ROOT(&self.root))? {
            let file = self.file_manager.open(file?.path())?;
            let sst = Sst::from_file_handle(file)?;
            md.data.push(sst.metadata()?);
        }
        Ok(Arc::new(md))
    }
}

//////////////////////////////////////////// MetadataKey ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MetadataKey (
    [u8; 16],
    [u8; 32],
);

///////////////////////////////////////// key_range_overlap ////////////////////////////////////////

fn key_range_overlap(lhs: &SstMetadata, rhs: &SstMetadata) -> bool {
    compare_bytes(lhs.first_key.as_bytes(), rhs.last_key.as_bytes()) != Ordering::Greater
        && compare_bytes(rhs.first_key.as_bytes(), lhs.last_key.as_bytes()) != Ordering::Greater
}

//////////////////////////////////////////// Compaction ////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Compaction {
    pub options: LsmOptions,
    pub inputs: Vec<SstMetadata>,
}

impl Compaction {
    pub fn stats(&self) -> CompactionStats {
        let graph = Graph::new(self.options.clone(), &self.inputs).unwrap();
        let mut stats = CompactionStats::default();
        stats.num_inputs += self.inputs.len();
        if stats.num_inputs > 0 {
            stats.lower_level = usize::max_value();
        }
        for input in self.inputs.iter() {
            stats.bytes_input += input.file_size as usize;
        }
        let mut lower = 0;
        let mut upper = 0;
        for idx in 0..self.inputs.len() {
            let level = graph.level_for_vertex(idx);
            stats.lower_level = min(stats.lower_level, level);
            stats.upper_level = max(stats.upper_level, level);
            upper += self.inputs[idx].file_size;
            if level < stats.upper_level {
                lower += self.inputs[idx].file_size;
            }
        }
        if upper > 0 {
            stats.ratio = (lower as f64) / (upper as f64);
        }
        stats
    }

    pub fn perform(&self, file_manager: &FileManager) -> Result<(), Error> {
        let mut digests: Vec<(Setsum, Option<Buffer>)> = Vec::new();
        let mut cursors: Vec<Box<dyn Cursor + 'static>> = Vec::new();
        let mut acc_setsum = Setsum::default();
        for sst_metadata in &self.inputs {
            let sst_setsum = Setsum::from_digest(sst_metadata.setsum);
            acc_setsum = acc_setsum + sst_setsum.clone();
            let file = file_manager.open(SST_FILE(&self.options.path, sst_setsum.hexdigest()))?;
            digests.push((sst_setsum, None));
            let sst = Sst::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let mut cursor = MergingCursor::new(cursors)?;
        cursor.seek_to_first()?;
        let prefix = COMPACTION_ROOT(&self.options.path, acc_setsum.hexdigest());
        create_dir(prefix.clone())?;
        let mut sstmb = SstMultiBuilder::new(prefix.clone(), ".sst".to_string(), self.options.sst.clone());
        loop {
            cursor.next()?;
            let kvr = match cursor.value() {
                Some(v) => { v },
                None => { break; },
            };
            match kvr.value {
                Some(v) => { sstmb.put(kvr.key, kvr.timestamp, v)?; }
                None => { sstmb.del(kvr.key, kvr.timestamp)?; }
            }
        }
        let paths = sstmb.seal()?;
        for path in paths.iter() {
            let file = file_manager.open(path.clone())?;
            let sst = Sst::from_file_handle(file)?;
            let sst_setsum = sst.setsum();
            digests.push((Setsum::from_digest(sst_setsum.digest()), Some(stack_pack(sst.metadata()?).to_buffer())));
            let new_path = SST_FILE(&self.options.path, sst_setsum.hexdigest());
            hard_link(path, new_path)?;
        }
        digests.sort_by_key(|x| x.0.hexdigest());
        let meta_now = now::millis();
        let meta_file_final = META_FILE(&self.options.path, acc_setsum.hexdigest());
        let meta_file = format!("tmp-{}-{}.sst", acc_setsum.hexdigest(), meta_now);
        let mut meta = SstBuilder::new(&meta_file, self.options.sst.clone())?;
        for (digest, buf) in digests.into_iter() {
            let key = MetadataKey(self.options.meta_id.id, digest.digest());
            todo!();
            /*
            let tuple_key = key.into_tuple_key();
            match buf {
                Some(value) => {
                    meta.put(tuple_key.as_bytes(), meta_now, value.as_bytes())?;
                },
                None => {
                    meta.del(tuple_key.as_bytes(), meta_now)?;
                },
            }
            */
        }
        meta.seal()?;
        rename(meta_file, meta_file_final)?;
        for path in paths.into_iter() {
            remove_file(path)?;
        }
        remove_dir(prefix)?;
        Ok(())
    }
}

////////////////////////////////////////// CompactionStats /////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct CompactionStats {
    pub num_inputs: usize,
    pub bytes_input: usize,
    pub lower_level: usize,
    pub upper_level: usize,
    pub ratio: f64,
}

//////////////////////////////////////////////// DB ////////////////////////////////////////////////

pub struct DB {
    root: PathBuf,
    options: LsmOptions,
    file_manager: Arc<FileManager>,
    metadata: Mutex<Arc<Metadata>>,
    _lockfile: Lockfile,
}

impl DB {
    pub fn ingest(&self, sst_paths: &[PathBuf]) -> Result<(), Error> {
        // For each SST, hardlink it into the ingest root.
        let mut ssts = Vec::new();
        let mut acc = Setsum::default();
        for sst_path in sst_paths {
            let file = self.file_manager.open(sst_path.clone())?;
            let sst = Sst::from_file_handle(file)?;
            // Update the setsum for the ingest.
            let setsum = sst.setsum();
            acc = acc + Setsum::from_digest(setsum.digest());
            // Hard-link the file into place.
            let target = SST_FILE(&self.root, setsum.hexdigest());
            if target.is_file() {
                return Err(Error::DuplicateSst {
                    core: ErrorCore::default(),
                    what: target.to_string_lossy().to_string(),
                });
            }
            hard_link(sst_path, target).map_io_err()?;
            // Extract the metadata.
            let metadata = sst.metadata()?;
            ssts.push(metadata);
        }
        ssts.sort_by(|lhs, rhs| compare_bytes(&lhs.setsum, &rhs.setsum));
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        let meta_file_final = META_FILE(&self.root, acc.hexdigest());
        let meta_file = format!("tmp-{}-{}.sst", acc.hexdigest(), now::millis());
        let mut meta = SstBuilder::new(&meta_file, self.options.sst.clone())?;
        for metadata in ssts.iter() {
            let key = MetadataKey(self.options.meta_id.id, metadata.setsum);
            /*
            let tuple_key = key.into_tuple_key();
            let ts = std::fs::metadata(SST_FILE(&self.root, metadata.setsum()))?.modified()?.duration_since(std::time::UNIX_EPOCH).map_err(|err| {
                Error::SystemError {
                    core: ErrorCore::default(),
                    what: err.to_string(),
                }
            })?.as_secs();
            let value = stack_pack(metadata).to_buffer();
            meta.put(tuple_key.as_bytes(), ts, value.as_bytes())?;
            */
            todo!();
        }
        meta.seal()?;
        rename(meta_file, meta_file_final)?;
        Ok(())
    }

    pub fn compactions(&self) -> Result<Vec<Compaction>, Error> {
        let state = self.get_state();
        let graph = Graph::new(self.options.clone(), &state.data)?;
        let mut compactions = graph.compactions();
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        Ok(compactions)
    }

    pub fn compact(&self, ssts: &[String]) -> Result<(), Error> {
        let mut compaction = Compaction {
            options: self.options.clone(),
            inputs: Vec::new(),
        };
        for sst_setsum in ssts {
            let file = self.file_manager.open(SST_FILE(self.options.path.clone(), sst_setsum.to_string()))?;
            let sst = Sst::from_file_handle(file)?;
            compaction.inputs.push(sst.metadata()?);
        }
        compaction.perform(&self.file_manager)?;
        self.reload()?;
        Ok(())
    }

    fn reload(&self) -> Result<(), Error> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut guard = self.metadata.lock().unwrap();
        *guard = guard.reload()?;
        Ok(())
    }

    fn get_state(&self) -> Arc<Metadata> {
        Arc::clone(&self.metadata.lock().unwrap())
    }
}
