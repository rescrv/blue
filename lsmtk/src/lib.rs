use std::cmp::{max, min, Ordering};
use std::fmt::Debug;
use std::fs::{create_dir, hard_link, rename, remove_dir, remove_file};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, RwLock};

use arrrg_derive::CommandLine;

use mani::{Edit, Manifest, ManifestOptions};

use setsum::Setsum;

use sst::file_manager::FileManager;
use sst::merging_cursor::MergingCursor;
use sst::{compare_bytes, Builder, Cursor, Sst, SstMetadata, SstMultiBuilder, SstOptions};

use zerror::{iotoz, Z};
use zerror_core::ErrorCore;
use zerror_derive::ZerrorCore;

mod graph;
mod in_flight;

use graph::Graph;
use in_flight::CompactionsInFlight;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn MANI_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("mani")
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
fn COMPACTION_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("compaction")
}

#[allow(non_snake_case)]
fn COMPACTION_DIR<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    COMPACTION_ROOT(root).join(setsum)
}

#[allow(non_snake_case)]
fn RM_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("trash")
}

#[allow(non_snake_case)]
fn RM_FILE<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    RM_ROOT(root).join(setsum + ".sst")
}

#[allow(non_snake_case)]
fn INGEST_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("ingest")
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug, ZerrorCore)]
pub enum Error {
    Success {
        core: ErrorCore,
    },
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
    DuplicateSst {
        core: ErrorCore,
        what: String,
    },
    SstNotFound {
        core: ErrorCore,
        setsum: String,
    },
    PathError {
        core: ErrorCore,
        path: PathBuf,
        what: String,
    },
    ManifestError {
        core: ErrorCore,
        what: mani::Error,
    },
    ConcurrentCompaction {
        core: ErrorCore,
        setsum: String,
    },
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError { core: ErrorCore::default(), what: what.to_string() }
    }
}

impl From<mani::Error> for Error {
    fn from(what: mani::Error) -> Error {
        Error::ManifestError { core: ErrorCore::default(), what }
    }
}

impl From<sst::Error> for Error {
    fn from(what: sst::Error) -> Error {
        match what {
            sst::Error::Success { core } => Error::Success { core },
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

iotoz!{Error}

//////////////////////////////////////////// LsmOptions ////////////////////////////////////////////

#[derive(CommandLine, Clone, Debug, Eq, PartialEq)]
pub struct LsmOptions {
    #[arrrg(nested)]
    mani: ManifestOptions,
    #[arrrg(nested)]
    sst: SstOptions,
    #[arrrg(required, "Root path for the lsmtk", "PATH")]
    path: String,
    #[arrrg(optional, "Maximum number of files to open", "FILES")]
    max_open_files: usize,
    #[arrrg(optional, "Maximum number of bytes permitted in a compaction", "BYTES")]
    max_compaction_bytes: usize,
}

impl LsmOptions {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn open(self) -> Result<DB, Error> {
        let root: PathBuf = PathBuf::from(&self.path);
        if !root.is_dir() {
            create_dir(&root)
                .as_z()
                .with_variable("sst", SST_ROOT(&root))?;
        }
        let mani = RwLock::new(Manifest::open(self.mani.clone(), MANI_ROOT(&root))?);
        if !SST_ROOT(&root).is_dir() {
            create_dir(SST_ROOT(&root))
                .as_z()
                .with_variable("sst", SST_ROOT(&root))?;
        }
        if !COMPACTION_ROOT(&root).is_dir() {
            create_dir(COMPACTION_ROOT(&root))
                .as_z()
                .with_variable("compaction", COMPACTION_ROOT(&root))?;
        }
        if !RM_ROOT(&root).is_dir() {
            create_dir(RM_ROOT(&root))
                .as_z()
                .with_variable("rm", RM_ROOT(&root))?;
        }
        if !INGEST_ROOT(&root).is_dir() {
            create_dir(INGEST_ROOT(&root))
                .as_z()
                .with_variable("ingest", INGEST_ROOT(&root))?;
        }
        let file_manager = Arc::new(FileManager::new(self.max_open_files));
        let db = DB {
            root,
            mani,
            options: self,
            file_manager,
            in_flight: CompactionsInFlight::default(),
            mtx: Mutex::default(),
            cnd: Condvar::default(),
        };
        Ok(db)
    }
}

impl Default for LsmOptions {
    fn default() -> Self {
        Self {
            mani: ManifestOptions::default(),
            sst: SstOptions::default(),
            path: "db".to_owned(),
            max_open_files: 1 << 20,
            max_compaction_bytes: usize::max_value(),
        }
    }
}

///////////////////////////////////////// key_range_overlap ////////////////////////////////////////

fn key_range_overlap(lhs: &SstMetadata, rhs: &SstMetadata) -> bool {
    compare_bytes(&lhs.first_key, &rhs.last_key) != Ordering::Greater
        && compare_bytes(&rhs.first_key, &lhs.last_key) != Ordering::Greater
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

    pub fn perform(&self, db: &DB) -> Result<(), Error> {
        let mut mani_edit = Edit::default();
        let mut to_remove = Vec::new();
        let mut cursors: Vec<Box<dyn Cursor + 'static>> = Vec::new();
        let mut acc_setsum = Setsum::default();
        // Create the cursors.
        for sst_metadata in &self.inputs {
            let sst_setsum = Setsum::from_digest(sst_metadata.setsum);
            acc_setsum = acc_setsum + sst_setsum.clone();
            let path_sst = SST_FILE(&self.options.path, sst_setsum.hexdigest());
            let path_rm = RM_FILE(&self.options.path, sst_setsum.hexdigest());
            to_remove.push((path_sst.clone(), path_rm));
            let file = db.file_manager.open(path_sst)?;
            mani_edit.rm(&sst_setsum.hexdigest())?;
            let sst = Sst::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let mut cursor = MergingCursor::new(cursors)?;
        cursor.seek_to_first()?;
        // Setup the compaction outputs
        let prefix = COMPACTION_DIR(&self.options.path, acc_setsum.hexdigest());
        create_dir(prefix.clone())?;
        let mut sstmb = SstMultiBuilder::new(prefix.clone(), ".sst".to_string(), self.options.sst.clone());
        'looping:
        loop {
            cursor.next()?;
            let kvr = match cursor.value() {
                Some(v) => { v },
                None => { break 'looping; },
            };
            match kvr.value {
                Some(v) => { sstmb.put(kvr.key, kvr.timestamp, v)?; }
                None => { sstmb.del(kvr.key, kvr.timestamp)?; }
            }
        }
        let paths = sstmb.seal()?;
        for path in paths.iter() {
            let file = db.file_manager.open(path.clone())?;
            let sst = Sst::from_file_handle(file)?;
            let sst_setsum = sst.setsum().hexdigest();
            mani_edit.add(&sst_setsum)?;
            let new_path = SST_FILE(&self.options.path, sst_setsum);
            hard_link(path, new_path)?;
        }
        db.mani.write().unwrap().apply(mani_edit)?;
        for (path, trash) in to_remove.into_iter() {
            rename(path, trash)?;
        }
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
    mani: RwLock<Manifest>,
    file_manager: Arc<FileManager>,
    in_flight: CompactionsInFlight,
    mtx: Mutex<usize>,
    cnd: Condvar,
}

impl DB {
    pub fn ingest(&self, sst_paths: &[PathBuf]) -> Result<(), Error> {
        // For each SST, hardlink it into the ingest root.
        let mut edit = Edit::default();
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
            hard_link(sst_path, target).as_z()?;
            edit.add(&setsum.hexdigest())?;
        }
        self.mani.write().unwrap().apply(edit)?;
        *self.mtx.lock().unwrap() += 1;
        self.cnd.notify_one();
        Ok(())
    }

    pub fn compactions(&self) -> Result<Vec<Compaction>, Error> {
        let setsums: Vec<String> = self.mani.read().unwrap().strs().cloned().collect();
        let mut sst_metadata = Vec::new();
        for sst_setsum in setsums.iter() {
            let file = self.file_manager.open(SST_FILE(&self.options.path, sst_setsum.clone()))?;
            let sst = Sst::from_file_handle(file)?;
            sst_metadata.push(sst.metadata()?);
        }
        let graph = Graph::new(self.options.clone(), &sst_metadata)?;
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
        let mut in_flight = self.in_flight.start();
        for sst_setsum in ssts {
            let file = self.file_manager.open(SST_FILE(self.options.path.clone(), sst_setsum.to_string()))?;
            let sst = Sst::from_file_handle(file)?;
            let meta = sst.metadata()?;
            if !in_flight.add(meta.setsum) {
                return Err(Error::ConcurrentCompaction {
                    core: ErrorCore::default(),
                    setsum: meta.setsum(),
                });
            }
            compaction.inputs.push(meta);
        }
        compaction.perform(self)?;
        Ok(())
    }

    pub fn compaction_background_thread(&self) -> Result<(), Error> {
        loop {
            {
                {
                    let mut mtx = self.mtx.lock().unwrap();
                    while *mtx == 0 {
                        mtx = self.cnd.wait(mtx).unwrap();
                    }
                    *mtx = 0;
                }
                let compactions = self.compactions()?;
                if !compactions.is_empty() {
                    compactions[0].perform(self)?;
                }
            }
        }
    }
}
