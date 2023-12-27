use std::cmp::{max, Ordering};
use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::{
    create_dir, hard_link, read_dir, remove_dir, remove_dir_all, remove_file, rename, File,
};
use std::io::ErrorKind;
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};

use biometrics::{Collector, Counter};
use indicio::clue;
use keyvalint::{compare_bytes, Cursor, KeyRef, KeyValuePair, KeyValueRef};
use mani::{Edit, Manifest, ManifestOptions};
use setsum::Setsum;
use sst::bounds_cursor::BoundsCursor;
use sst::file_manager::FileManager;
use sst::gc::GarbageCollectionPolicy;
use sst::log::{ConcurrentLogBuilder, LogOptions};
use sst::merging_cursor::MergingCursor;
use sst::pruning_cursor::PruningCursor;
use sst::{
    check_key_len, check_value_len, Builder, Sst, SstBuilder, SstCursor, SstMetadata,
    SstMultiBuilder, SstOptions,
};
use sync42::wait_list::WaitList;
use utilz::fmt;
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;
use zerror_derive::ZerrorCore;

mod memtable;
mod reference_counter;
mod tree;
mod verifier;

use memtable::MemTable;
use reference_counter::ReferenceCounter;
use tree::{Compaction, Tree};

pub use tree::{CompactionID, NUM_LEVELS};
pub use verifier::{LsmVerifier, ManifestVerifier};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static OPEN_DB: Counter = Counter::new("lsmtk.open");

static BYTES_INGESTED: Counter = Counter::new("lsmtk.bytes_ingested");
static INGEST_LINK: Counter = Counter::new("lsmtk.ingest.link");
static INGEST_STALL: Counter = Counter::new("lsmtk.ingest.stall");

static COMPACTION_THREAD_NO_COMPACTION: Counter =
    Counter::new("lsmtk.compaction_thread.no_compaction");

static COMPACTION_PERFORM: Counter = Counter::new("lsmtk.compaction");
static COMPACTION_NEW_CURSOR: Counter = Counter::new("lsmtk.compaction.new_cursor");
static COMPACTION_KEYS_WRITTEN: Counter = Counter::new("lsmtk.compaction.keys_written");
static COMPACTION_BYTES_WRITTEN: Counter = Counter::new("lsmtk.compaction.bytes_written");
static COMPACTION_LINK: Counter = Counter::new("lsmtk.compaction.link");
static COMPACTION_REMOVE: Counter = Counter::new("lsmtk.compaction.remove");

static GARBAGE_COLLECTION_PERFORM: Counter = Counter::new("lsmtk.garbage_collection");
static GARBAGE_COLLECTION_KEYS_DROPPED: Counter =
    Counter::new("lsmtk.garbage_collection.keys_dropped");

pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&OPEN_DB);
    collector.register_counter(&BYTES_INGESTED);
    collector.register_counter(&INGEST_LINK);
    collector.register_counter(&INGEST_STALL);
    collector.register_counter(&COMPACTION_THREAD_NO_COMPACTION);
    collector.register_counter(&COMPACTION_PERFORM);
    collector.register_counter(&COMPACTION_NEW_CURSOR);
    collector.register_counter(&COMPACTION_KEYS_WRITTEN);
    collector.register_counter(&COMPACTION_BYTES_WRITTEN);
    collector.register_counter(&COMPACTION_LINK);
    collector.register_counter(&COMPACTION_REMOVE);
    collector.register_counter(&GARBAGE_COLLECTION_PERFORM);
    collector.register_counter(&GARBAGE_COLLECTION_KEYS_DROPPED);
    tree::register_biometrics(collector);
    verifier::register_biometrics(collector);
}

///////////////////////////////////////////// Constants ////////////////////////////////////////////

#[allow(non_snake_case)]
pub fn MANI_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("mani")
}

#[allow(non_snake_case)]
pub fn VERIFY_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("verify")
}

#[allow(non_snake_case)]
pub fn SST_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("sst")
}

#[allow(non_snake_case)]
pub fn SST_FILE<P: AsRef<Path>>(root: P, setsum: Setsum) -> PathBuf {
    SST_ROOT(root).join(setsum.hexdigest() + ".sst")
}

#[allow(non_snake_case)]
pub fn COMPACTION_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("compaction")
}

#[allow(non_snake_case)]
pub fn COMPACTION_DIR<P: AsRef<Path>>(root: P, setsum: Setsum) -> PathBuf {
    COMPACTION_ROOT(root).join(setsum.hexdigest())
}

#[allow(non_snake_case)]
pub fn TRASH_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("trash")
}

#[allow(non_snake_case)]
pub fn TRASH_SST<P: AsRef<Path>>(root: P, setsum: Setsum) -> PathBuf {
    TRASH_ROOT(root).join(setsum.hexdigest() + ".sst")
}

#[allow(non_snake_case)]
pub fn INGEST_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("ingest")
}

#[allow(non_snake_case)]
pub fn INGEST_FILE<P: AsRef<Path>>(root: P, setsum: Setsum) -> PathBuf {
    INGEST_ROOT(root).join(setsum.hexdigest() + ".sst")
}

#[allow(non_snake_case)]
pub fn TEMP_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("tmp")
}

#[allow(non_snake_case)]
pub fn TEMP_FILE<P: AsRef<Path>>(root: P, setsum: Setsum) -> PathBuf {
    TEMP_ROOT(root).join(setsum.hexdigest() + ".sst")
}

#[allow(non_snake_case)]
pub fn LOG_FILE<P: AsRef<Path>>(root: P, number: u64) -> PathBuf {
    root.as_ref().to_path_buf().join(format!("log.{number}"))
}

fn parse_log_file<P: AsRef<Path>>(path: P) -> Option<u64> {
    if let Some(file_name) = path.as_ref().file_name() {
        if file_name.to_string_lossy().as_ref() != file_name {
            return None;
        }
        let file_name = file_name.to_string_lossy().to_string();
        if !file_name.starts_with("log.") {
            return None;
        }
        let number: u64 = match file_name[4..].parse() {
            Ok(number) => number,
            Err(_) => {
                // SAFETY(rescrv):  The only valid logs are ones that match log.#.
                //
                // The verifier will catch excess files that get left around.
                return None;
            }
        };
        Some(number)
    } else {
        None
    }
}

fn ensure_dir(path: PathBuf, commentary: &str) -> Result<(), Error> {
    if !path.is_dir() {
        Ok(create_dir(&path).as_z().with_variable(commentary, path)?)
    } else {
        Ok(())
    }
}

fn make_all_dirs<P: AsRef<Path>>(root: P) -> Result<(), Error> {
    ensure_dir(VERIFY_ROOT(&root), "verify")?;
    ensure_dir(SST_ROOT(&root), "sst")?;
    ensure_dir(COMPACTION_ROOT(&root), "compaction")?;
    ensure_dir(TRASH_ROOT(&root), "trash")?;
    ensure_dir(INGEST_ROOT(&root), "ingest")?;
    ensure_dir(TEMP_ROOT(&root), "tmp")?;
    Ok(())
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, ZerrorCore)]
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
    EmptyBatch {
        core: ErrorCore,
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
    Backoff {
        core: ErrorCore,
        what: String,
    },
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError {
            core: ErrorCore::default(),
            what: what.to_string(),
        }
    }
}

impl From<mani::Error> for Error {
    fn from(what: mani::Error) -> Error {
        Error::ManifestError {
            core: ErrorCore::default(),
            what,
        }
    }
}

impl From<sst::Error> for Error {
    fn from(what: sst::Error) -> Error {
        match what {
            sst::Error::Success { core } => Error::Success { core },
            sst::Error::KeyTooLarge {
                core,
                length,
                limit,
            } => Error::KeyTooLarge {
                core,
                length,
                limit,
            },
            sst::Error::ValueTooLarge {
                core,
                length,
                limit,
            } => Error::ValueTooLarge {
                core,
                length,
                limit,
            },
            sst::Error::SortOrder {
                core,
                last_key,
                last_timestamp,
                new_key,
                new_timestamp,
            } => Error::SortOrder {
                core,
                last_key,
                last_timestamp,
                new_key,
                new_timestamp,
            },
            sst::Error::TableFull { core, size, limit } => Error::TableFull { core, size, limit },
            sst::Error::BlockTooSmall {
                core,
                length,
                required,
            } => Error::BlockTooSmall {
                core,
                length,
                required,
            },
            sst::Error::UnpackError {
                core,
                error,
                context,
            } => Error::UnpackError {
                core,
                error,
                context,
            },
            sst::Error::Crc32cFailure {
                core,
                start,
                limit,
                crc32c,
            } => Error::Crc32cFailure {
                core,
                start,
                limit,
                crc32c,
            },
            sst::Error::Corruption { core, context } => Error::Corruption { core, context },
            sst::Error::LogicError { core, context } => Error::LogicError { core, context },
            sst::Error::SystemError { core, what } => Error::SystemError { core, what },
            sst::Error::TooManyOpenFiles { core, limit } => Error::TooManyOpenFiles { core, limit },
            sst::Error::EmptyBatch { core } => Error::EmptyBatch { core },
        }
    }
}

iotoz! {Error}

/////////////////////////////////////////// LSM_TREE_LOG ///////////////////////////////////////////

#[derive(Clone, Default, Eq, PartialEq, prototk_derive::Message)]
pub enum TreeLogKey {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    BySetsum {
        #[prototk(1, bytes32)]
        setsum: [u8; 32],
    },
    #[prototk(3, message)]
    ByCompactionID {
        #[prototk(1, message)]
        compaction_id: CompactionID,
    },
}

impl std::fmt::Debug for TreeLogKey {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use TreeLogKey::*;
        match self {
            Nop => write!(fmt, "NOP"),
            BySetsum { setsum } => write!(fmt, "{:?}", Setsum::from_digest(*setsum)),
            ByCompactionID { compaction_id } => write!(fmt, "{}", compaction_id.human_readable()),
        }
    }
}

#[derive(Clone, Default, prototk_derive::Message)]
pub enum TreeLogValue {
    #[prototk(1, message)]
    #[default]
    Nop,
    #[prototk(2, message)]
    Ingest {
        #[prototk(2, uint64)]
        level: usize,
        #[prototk(3, uint64)]
        cardinality: usize,
    },
    #[prototk(3, message)]
    CandidateCompaction {
        #[prototk(1, sint64)]
        score: i64,
        #[prototk(2, uint64)]
        lower_level: usize,
        #[prototk(3, uint64)]
        upper_level: usize,
        #[prototk(4, bytes)]
        first_key: Vec<u8>,
        #[prototk(5, bytes)]
        last_key: Vec<u8>,
        #[prototk(6, bytes32)]
        inputs: Vec<[u8; 32]>,
    },
    #[prototk(4, message)]
    GatherInput {},
    #[prototk(5, message)]
    RemoveCompactionDir {
        #[prototk(1, string)]
        dir: String,
    },
    #[prototk(8, message)]
    ApplyCompaction {
        #[prototk(1, bytes32)]
        outputs: Vec<[u8; 32]>,
    },
    #[prototk(9, message)]
    CompactLevel {
        #[prototk(1, uint64)]
        level: usize,
        #[prototk(2, bytes32)]
        before: Vec<[u8; 32]>,
        #[prototk(3, bytes32)]
        after: Vec<[u8; 32]>,
    },
    #[prototk(10, message)]
    CompactUpperLevelBounds {
        #[prototk(1, uint64)]
        level: usize,
        #[prototk(2, uint64)]
        lower_bound: usize,
        #[prototk(3, uint64)]
        upper_bound: usize,
    },
}

fn setsum_list(setsums: &[[u8; 32]]) -> Vec<Setsum> {
    setsums
        .iter()
        .copied()
        .map(Setsum::from_digest)
        .collect::<Vec<_>>()
}

impl std::fmt::Debug for TreeLogValue {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use TreeLogValue::*;
        match self {
            Nop => fmt.debug_struct("Nop").finish(),
            Ingest { level, cardinality } => fmt
                .debug_struct("Ingest")
                .field("level", level)
                .field("cardinality", cardinality)
                .finish(),
            CandidateCompaction {
                score,
                lower_level,
                upper_level,
                first_key,
                last_key,
                inputs,
            } => fmt
                .debug_struct("CandidateCompaction")
                .field("score", score)
                .field("lower_level", lower_level)
                .field("upper_level", upper_level)
                .field("first_key", &fmt::escape_str(first_key))
                .field("last_key", &fmt::escape_str(last_key))
                .field("inputs", &setsum_list(inputs))
                .finish(),
            GatherInput {} => fmt.debug_struct("GatherInput").finish(),
            RemoveCompactionDir { dir } => fmt
                .debug_struct("RemoveCompactionDir")
                .field("dir", dir)
                .finish(),
            ApplyCompaction { outputs } => fmt
                .debug_struct("ApplyCompaction")
                .field("outputs", &setsum_list(outputs))
                .finish(),
            CompactLevel {
                level,
                before,
                after,
            } => fmt
                .debug_struct("CompactLevel")
                .field("level", level)
                .field("before", &setsum_list(before))
                .field("after", &setsum_list(after))
                .field(
                    "removed",
                    &setsum_list(
                        &before
                            .iter()
                            .filter(|x| !after.contains(x))
                            .copied()
                            .collect::<Vec<_>>(),
                    ),
                )
                .field(
                    "added",
                    &setsum_list(
                        &after
                            .iter()
                            .filter(|x| !before.contains(x))
                            .copied()
                            .collect::<Vec<_>>(),
                    ),
                )
                .finish(),
            CompactUpperLevelBounds {
                level,
                lower_bound,
                upper_bound,
            } => fmt
                .debug_struct("CompactUpperLevelBounds")
                .field("level", level)
                .field("lower_bound", lower_bound)
                .field("upper_bound", upper_bound)
                .finish(),
        }
    }
}

pub static LSM_TREE_LOG: indicio::Collector<TreeLogKey, TreeLogValue> = indicio::Collector::new();

////////////////////////////////////////// LsmtkOptions //////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct LsmtkOptions {
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    mani: ManifestOptions,
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    log: LogOptions,
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    sst: SstOptions,
    // TODO(rescrv):  Convert this to a PathBuf.
    #[cfg_attr(
        feature = "command_line",
        arrrg(required, "Root path for the lsmtk", "PATH")
    )]
    path: String,
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Maximum number of files to open", "FILES")
    )]
    max_open_files: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Maximum number of bytes permitted in a compaction", "BYTES")
    )]
    max_compaction_bytes: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Maximum number of files permitted in a compaction", "FILES")
    )]
    max_compaction_files: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Maximum number of files permitted in L0 before compaction becomes mandatory.",
            "FILES"
        )
    )]
    l0_mandatory_compaction_threshold_files: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Maximum number of bytes permitted in L0 before compaction becomes mandatory.",
            "BYTES"
        )
    )]
    l0_mandatory_compaction_threshold_bytes: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Maximum number of files permitted in L0 before writes begin to stall.",
            "FILES"
        )
    )]
    l0_write_stall_threshold_files: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Maximum number of bytes permitted in L0 before writes begin to stall.",
            "BYTES"
        )
    )]
    l0_write_stall_threshold_bytes: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Maximum number of bytes to grow a memtable to before compacting into L0.",
            "BYTES"
        )
    )]
    memtable_size_bytes: usize,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Garbage collection policy as a string; only versions=X will collect at the moment.",
            "POLICY"
        )
    )]
    gc_policy: GarbageCollectionPolicy,
}

impl LsmtkOptions {
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Default for LsmtkOptions {
    fn default() -> Self {
        Self {
            mani: ManifestOptions::default(),
            log: LogOptions::default(),
            sst: SstOptions::default(),
            path: "db".to_owned(),
            max_open_files: 1 << 9,
            max_compaction_bytes: 1 << 29,
            max_compaction_files: 1 << 6,
            l0_mandatory_compaction_threshold_files: 4,
            l0_mandatory_compaction_threshold_bytes: 1 << 26,
            l0_write_stall_threshold_files: 12,
            l0_write_stall_threshold_bytes: 1 << 28,
            memtable_size_bytes: 1 << 26,
            gc_policy: GarbageCollectionPolicy::try_from("versions = 1").unwrap(),
        }
    }
}

////////////////////////////////////////////// LsmTree /////////////////////////////////////////////

pub struct LsmTree {
    root: PathBuf,
    options: LsmtkOptions,
    file_manager: Arc<FileManager>,
    mani: RwLock<Manifest>,
    tree: Mutex<Arc<Tree>>,
    compaction: Mutex<()>,
    stall: Condvar,
    compact: Condvar,
    references: ReferenceCounter<Setsum>,
}

impl LsmTree {
    pub fn open(options: LsmtkOptions) -> Result<Self, Error> {
        let root: PathBuf = PathBuf::from(&options.path);
        ensure_dir(root.clone(), "root")?;
        make_all_dirs(&root)?;
        let mani = Manifest::open(options.mani.clone(), MANI_ROOT(&root))?;
        Self::from_manifest(options, mani)
    }

    fn from_manifest(options: LsmtkOptions, mani: Manifest) -> Result<Self, Error> {
        let root: PathBuf = PathBuf::from(&options.path);
        let mani = RwLock::new(mani);
        let file_manager = Arc::new(FileManager::new(options.max_open_files));
        let (metadata, _) = Self::list_ssts_present(&root, &mani.read().unwrap(), &file_manager)?;
        let tree = Mutex::new(Arc::new(Tree::open(options.clone(), metadata)?));
        let compaction = Mutex::new(());
        let tree_setsum = tree.lock().unwrap().compute_setsum().hexdigest();
        let mani_setsum = mani
            .read()
            .unwrap()
            .info('O')
            .map(|s| s.to_string())
            .unwrap_or(Setsum::default().hexdigest());
        if tree_setsum != mani_setsum {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "setsum of tree does not match setsum of manifest".to_string(),
            })
            .as_z()
            .with_variable("tree", tree_setsum)
            .with_variable("mani", mani_setsum);
        }
        let stall = Condvar::new();
        let compact = Condvar::new();
        let references = ReferenceCounter::default();
        Self::explicit_ref(&references, &tree.lock().unwrap());
        OPEN_DB.click();
        let db = Self {
            root,
            options,
            file_manager,
            mani,
            tree,
            compaction,
            stall,
            compact,
            references,
        };
        // TODO(rescrv): cleanup orphaned sst files
        Ok(db)
    }

    fn list_ssts_present<P: AsRef<Path>>(
        root: P,
        mani: &Manifest,
        file_manager: &FileManager,
    ) -> Result<(Vec<SstMetadata>, HashSet<Setsum>), Error> {
        let mut metadata = vec![];
        let mut setsums = HashSet::new();
        for hexdigest in mani.strs() {
            let setsum = Setsum::from_hexdigest(hexdigest).ok_or(Error::Corruption {
                core: ErrorCore::default(),
                context: "setsum invalid".to_string(),
            })?;
            let path = SST_FILE(&root, setsum);
            let file = file_manager.open(&path)?;
            let sst = Sst::from_file_handle(file)?;
            metadata.push(sst.metadata()?);
            setsums.insert(setsum);
        }
        Ok((metadata, setsums))
    }

    pub fn ingest<P: AsRef<Path>>(&self, sst_path: P) -> Result<(), Error> {
        // For each SST, hardlink it into the ingest root.
        let mut edit = Edit::default();
        let mut acc = Setsum::default();
        let metadata = self.file_manager.stat(&sst_path)?;
        BYTES_INGESTED.count(metadata.file_size);
        // Update the setsum for the ingest.
        // We are adding data, not removing it, so subtract to balance the added output.
        let setsum = Setsum::from_digest(metadata.setsum);
        acc -= setsum;
        // Hard-link the file into place.
        let target = SST_FILE(&self.root, setsum);
        if target.exists() {
            return Err(Error::DuplicateSst {
                core: ErrorCore::default(),
                what: target.to_string_lossy().to_string(),
            });
        }
        INGEST_LINK.click();
        hard_link(&sst_path, target).as_z()?;
        edit.add(&setsum.hexdigest())?;
        self.apply_manifest_ingest(acc, edit, metadata)?;
        let _mutex = self.compaction.lock().unwrap();
        self.compact.notify_all();
        Ok(())
    }

    pub fn compaction_thread(&self) -> Result<(), Error> {
        loop {
            let compaction = {
                let mut mutex = self.compaction.lock().unwrap();
                'inner: loop {
                    let tree = self.get_tree();
                    let compaction = tree.next_compaction();
                    self.explicit_unref(tree);
                    if let Some(compaction) = compaction {
                        break 'inner compaction;
                    } else {
                        COMPACTION_THREAD_NO_COMPACTION.click();
                        mutex = self.compact.wait(mutex).unwrap();
                    }
                }
            };
            if let Err(err) = self.perform_compaction(compaction.clone()) {
                let _mutex = self.compaction.lock().unwrap();
                let tree = self.get_tree();
                let _ = tree.release_compaction(compaction);
                self.explicit_unref(tree);
                return Err(err);
            }
        }
    }

    fn perform_compaction(&self, compaction: Compaction) -> Result<(), Error> {
        COMPACTION_PERFORM.click();
        if compaction.inputs().count() == 1 {
            // SAFETY(rescrv): This is ensured by count in a good implementation.
            let input = compaction.inputs().next().unwrap();
            return self.apply_moving_compaction(compaction, input);
        }
        if compaction.top_level() {
            return self.perform_garbage_collection(compaction);
        }
        let mut mani_edit = Edit::default();
        let (input_setsum, mut cursor, compaction_dir) =
            self.compaction_setup(&compaction, &mut mani_edit)?;
        cursor.seek_to_first()?;
        // Get a set of hints as to where to split the multi-builder.
        let tree = self.get_tree();
        let mut split_hint = tree::SplitHint::new(tree.clone());
        // Setup the compaction multi-builder.
        let mut sstmb = SstMultiBuilder::new(
            compaction_dir.clone(),
            ".sst".to_string(),
            self.options.sst.clone(),
        );
        'looping: loop {
            cursor.next()?;
            let kvr = match cursor.key_value() {
                Some(v) => v,
                None => {
                    break 'looping;
                }
            };
            if !compaction.top_level() && split_hint.witness(kvr.key) {
                sstmb.split_hint()?;
            }
            COMPACTION_KEYS_WRITTEN.click();
            match kvr.value {
                Some(v) => {
                    sstmb.put(kvr.key, kvr.timestamp, v)?;
                }
                None => {
                    sstmb.del(kvr.key, kvr.timestamp)?;
                }
            }
        }
        drop(cursor);
        // SAFETY(rescrv): Drop the split hint before calling explicit_unref on the tree.
        drop(split_hint);
        self.explicit_unref(tree);
        // Seal the multi-builder.
        let paths = sstmb.seal()?;
        // Finish the compaction
        self.compaction_finish(
            compaction,
            compaction_dir,
            paths,
            input_setsum,
            Setsum::default(),
            mani_edit,
        )
    }

    fn perform_garbage_collection(&self, compaction: Compaction) -> Result<(), Error> {
        GARBAGE_COLLECTION_PERFORM.click();
        let mut mani_edit = Edit::default();
        let (input_setsum, mut cursor, compaction_dir) =
            self.compaction_setup(&compaction, &mut mani_edit)?;
        cursor.seek_to_first()?;
        let mut gc_cursor = cursor.clone();
        gc_cursor.next()?;
        let mut gc = self.options.gc_policy.collector(gc_cursor, 0)?;
        // Setup the compaction multi-builder.
        let mut sstmb = SstMultiBuilder::new(
            compaction_dir.clone(),
            ".sst".to_string(),
            self.options.sst.clone(),
        );
        let mut gc_next = gc.next()?;
        let mut discard = Setsum::default();
        'looping: loop {
            cursor.next()?;
            let kvr = match cursor.key_value() {
                Some(v) => v,
                None => {
                    break 'looping;
                }
            };
            let retain = if let Some(gcn) = gc_next {
                match gcn.cmp(&KeyRef::from(&kvr)) {
                    Ordering::Less => {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            context: "gc iterator out of sync with inputs".to_string(),
                        });
                    }
                    Ordering::Equal => {
                        gc_next = gc.next()?;
                        true
                    }
                    Ordering::Greater => false,
                }
            } else {
                false
            };
            if retain {
                COMPACTION_KEYS_WRITTEN.click();
                match kvr.value {
                    Some(v) => {
                        sstmb.put(kvr.key, kvr.timestamp, v)?;
                    }
                    None => {
                        sstmb.del(kvr.key, kvr.timestamp)?;
                    }
                }
            } else {
                GARBAGE_COLLECTION_KEYS_DROPPED.click();
                let mut setsum = sst::Setsum::default();
                setsum.insert(kvr);
                discard += setsum.into_inner();
            }
        }
        drop(cursor);
        // Seal the multi-builder.
        let paths = sstmb.seal()?;
        // Finish the compaction
        self.compaction_finish(
            compaction,
            compaction_dir,
            paths,
            input_setsum,
            discard,
            mani_edit,
        )
    }

    fn compaction_setup(
        &self,
        compaction: &Compaction,
        mani_edit: &mut Edit,
    ) -> Result<(Setsum, MergingCursor<SstCursor>, PathBuf), Error> {
        let mut cursors: Vec<SstCursor> = vec![];
        let mut acc = Setsum::default();
        // Figure out the moves to make, update the mani_edit, compute setsum, and create a cursor.
        for input in compaction.inputs() {
            let sst_path = SST_FILE(&self.root, input);
            let file = self.file_manager.open(&sst_path)?;
            mani_edit.rm(&input.hexdigest())?;
            let sst = Sst::from_file_handle(file)?;
            cursors.push(sst.cursor());
            acc += input;
            COMPACTION_NEW_CURSOR.click();
            clue! { LSM_TREE_LOG, TreeLogKey::BySetsum {
                    setsum: input.digest(),
                } => TreeLogValue::GatherInput {
                }
            };
        }
        // Setup the compaction output directory.
        let compaction_dir = COMPACTION_DIR(&self.root, acc);
        if compaction_dir.exists() {
            clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                    compaction_id: compaction.compaction_id(),
                } => TreeLogValue::RemoveCompactionDir {
                    dir: compaction_dir.to_string_lossy().to_string(),
                }
            };
            remove_dir_all(&compaction_dir)
                .as_z()
                .with_variable("dir", &compaction_dir)?;
        }
        create_dir(&compaction_dir)?;
        Ok((acc, MergingCursor::new(cursors)?, compaction_dir))
    }

    fn compaction_finish(
        &self,
        compaction: Compaction,
        compaction_dir: PathBuf,
        paths: Vec<PathBuf>,
        input_setsum: Setsum,
        discard_setsum: Setsum,
        mut mani_edit: Edit,
    ) -> Result<(), Error> {
        let mut outputs = vec![];
        let mut output_setsum = Setsum::default();
        // NOTE(rescrv):  Sometimes compaction generates the same file as input and output.  We are
        // not to remove the file in that case.
        for path in paths.iter() {
            let metadata = self.file_manager.stat(path)?;
            let setsum = Setsum::from_digest(metadata.setsum);
            output_setsum += setsum;
            COMPACTION_BYTES_WRITTEN.count(metadata.file_size);
            mani_edit.add(&setsum.hexdigest())?;
            let new_path = SST_FILE(&self.root, setsum);
            COMPACTION_LINK.click();
            match hard_link(path, &new_path) {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
                err @ Err(_) => {
                    return err
                        .as_z()
                        .with_variable("src", path)
                        .with_variable("dst", &new_path);
                }
            };
            outputs.push(metadata);
        }
        if input_setsum != output_setsum + discard_setsum {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "setsum does not balance input = output + discard".to_string(),
            }
            .with_variable("input_setsum", input_setsum.hexdigest())
            .with_variable("output_setsum", output_setsum.hexdigest())
            .with_variable("discard_setsum", discard_setsum.hexdigest()));
        }
        let ret = self.apply_manifest_compaction(compaction, discard_setsum, mani_edit, outputs);
        for path in paths.into_iter() {
            COMPACTION_REMOVE.click();
            remove_file(&path).as_z().with_variable("path", &path)?;
        }
        remove_dir(&compaction_dir)
            .as_z()
            .with_variable("dir", &compaction_dir)?;
        ret
    }

    fn apply_manifest_ingest(
        &self,
        setsum: Setsum,
        mut mani_edit: Edit,
        new: SstMetadata,
    ) -> Result<(), Error> {
        let _mutex = self.compaction.lock().unwrap();
        let mut tree = self.tree.lock().unwrap();
        while tree.should_stall_ingest() {
            INGEST_STALL.click();
            tree = self.stall.wait(tree).unwrap();
        }
        let tree_setsum = tree.compute_setsum();
        // NOTE(rescrv):  We subtract because we are removing the discard setsum.
        // This has the happy effect of subtracting the inverse of what we added.
        let output_setsum = tree_setsum - setsum;
        // TODO(rescrv): poison here.
        mani_edit.info('I', &tree_setsum.hexdigest())?;
        mani_edit.info('O', &output_setsum.hexdigest())?;
        mani_edit.info('D', &setsum.hexdigest())?;
        // TODO(rescrv):  Do not hold tree lock across manifest edit.
        self.mani.write().unwrap().apply(mani_edit)?;
        let new_tree = Arc::new(tree.ingest(new)?);
        self.install_tree(&mut tree, new_tree);
        // TODO(rescrv): don't hold the lock for computing setsum.
        let tree_setsum = tree.compute_setsum();
        assert_eq!(tree_setsum, output_setsum);
        Ok(())
    }

    fn apply_manifest_compaction(
        &self,
        compaction: Compaction,
        discard_setsum: Setsum,
        mut mani_edit: Edit,
        outputs: Vec<SstMetadata>,
    ) -> Result<(), Error> {
        let _mutex = self.compaction.lock().unwrap();
        let mut tree = self.tree.lock().unwrap();
        let tree_setsum = tree.compute_setsum();
        let output_setsum = tree_setsum - discard_setsum;
        // TODO(rescrv): poison here.
        mani_edit.info('I', &tree_setsum.hexdigest())?;
        mani_edit.info('O', &output_setsum.hexdigest())?;
        mani_edit.info('D', &discard_setsum.hexdigest())?;
        self.mani.write().unwrap().apply(mani_edit)?;
        let new_tree = Arc::new(tree.apply_compaction(compaction, outputs)?);
        self.install_tree(&mut tree, new_tree);
        // TODO(rescrv): don't hold the lock for computing setsum.
        let tree_setsum = tree.compute_setsum();
        assert_eq!(tree_setsum, output_setsum);
        self.stall.notify_all();
        Ok(())
    }

    fn apply_moving_compaction(&self, compaction: Compaction, output: Setsum) -> Result<(), Error> {
        let _mutex = self.compaction.lock().unwrap();
        let sst_path = SST_FILE(&self.root, output);
        let file = self.file_manager.open(sst_path)?;
        let sst = Sst::from_file_handle(file)?;
        let meta = sst.metadata()?;
        let mut tree = self.tree.lock().unwrap();
        let tree_setsum1 = tree.compute_setsum();
        let new_tree = Arc::new(tree.apply_compaction(compaction, vec![meta])?);
        self.install_tree(&mut tree, new_tree);
        let tree_setsum2 = tree.compute_setsum();
        assert_eq!(tree_setsum1, tree_setsum2);
        self.stall.notify_all();
        Ok(())
    }

    fn get_tree(&self) -> Arc<Tree> {
        Arc::clone(&*self.tree.lock().unwrap())
    }

    fn install_tree(&self, tree1: &mut MutexGuard<'_, Arc<Tree>>, mut tree2: Arc<Tree>) {
        Self::explicit_ref(&self.references, &tree2);
        std::mem::swap(&mut **tree1, &mut tree2);
        self.explicit_unref(tree2);
    }

    fn explicit_ref(references: &ReferenceCounter<Setsum>, tree: &Tree) {
        for setsum in tree.setsums() {
            references.inc(setsum);
        }
    }

    fn explicit_unref(&self, tree: Arc<Tree>) {
        if Arc::strong_count(&tree) != 1 {
            return;
        }
        for setsum in tree.setsums() {
            if self.references.dec(setsum) {
                let sst_path = SST_FILE(&self.root, setsum);
                let trash_path = TRASH_SST(&self.root, setsum);
                // SAFETY(rescrv):  This will just leave an orphan.
                // The verifier will pick up on there being orphans.
                let _ = rename(sst_path, trash_path);
            }
        }
    }
}

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

#[derive(Default)]
pub struct WriteBatch {
    entries: Vec<KeyValuePair>,
}

impl WriteBatch {
    pub fn with_capacity(cap: usize) -> Self {
        let entries = Vec::with_capacity(cap);
        Self { entries }
    }
}

impl keyvalint::WriteBatch for WriteBatch {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: Some(value.into()),
        });
    }

    fn del(&mut self, key: &[u8], timestamp: u64) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: None,
        });
    }
}

impl<'a> keyvalint::WriteBatch for &'a mut WriteBatch {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: Some(value.into()),
        });
    }

    fn del(&mut self, key: &[u8], timestamp: u64) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: None,
        });
    }
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

struct KeyValueStoreState {
    seq_no: u64,
    imm: Option<Arc<MemTable>>,
    imm_trigger: u64,
    mem: Arc<MemTable>,
    mem_log: Arc<ConcurrentLogBuilder<File>>,
    mem_path: PathBuf,
    mem_seq_no: u64,
}

pub struct KeyValueStore {
    root: PathBuf,
    tree: LsmTree,
    options: LsmtkOptions,
    state: Mutex<KeyValueStoreState>,
    memtable_mutex: Mutex<()>,
    wait_list: WaitList<()>,
    cnd_needs_memtable_flush: Condvar,
    cnd_memtable_rolled_over: Condvar,
}

impl KeyValueStore {
    pub fn open(options: LsmtkOptions) -> Result<Self, Error> {
        let root: PathBuf = PathBuf::from(&options.path);
        ensure_dir(root.clone(), "root")?;
        make_all_dirs(&root)?;
        let mut mani = Manifest::open(options.mani.clone(), MANI_ROOT(&root))?;
        if mani.info('I').is_none() {
            let mut edit = Edit::default();
            edit.info('I', &Setsum::default().hexdigest())?;
            edit.info('D', &Setsum::default().hexdigest())?;
            edit.info('O', &Setsum::default().hexdigest())?;
            mani.apply(edit)?;
        }
        let mut seq_no = Self::recover(&options, &mut mani)? + 1;
        let tree = LsmTree::from_manifest(options.clone(), mani)?;
        let imm = None;
        let imm_trigger = 0;
        let mem = Arc::new(MemTable::default());
        let mem_path = LOG_FILE(&root, seq_no);
        let mem_log = Self::start_new_log(&mem_path, options.log.clone())?;
        let mem_seq_no = seq_no;
        seq_no += 1;
        let state = Mutex::new(KeyValueStoreState {
            seq_no,
            imm,
            imm_trigger,
            mem,
            mem_log,
            mem_path,
            mem_seq_no,
        });
        let memtable_mutex = Mutex::new(());
        let wait_list = WaitList::new();
        let cnd_needs_memtable_flush = Condvar::new();
        let cnd_memtable_rolled_over = Condvar::new();
        Ok(Self {
            root,
            tree,
            options,
            state,
            memtable_mutex,
            wait_list,
            cnd_needs_memtable_flush,
            cnd_memtable_rolled_over,
        })
    }

    fn recover(options: &LsmtkOptions, mani: &mut Manifest) -> Result<u64, Error> {
        let mut numbers = vec![];
        for entry in read_dir(&options.path)? {
            if let Some(number) = parse_log_file(entry?.file_name()) {
                numbers.push(number);
            }
        }
        numbers.sort();
        let mut seq_no = 0;
        for number in numbers.into_iter() {
            seq_no = std::cmp::max(Self::recover_one(options, number, mani)?, seq_no);
        }
        Ok(seq_no)
    }

    fn recover_one(options: &LsmtkOptions, number: u64, mani: &mut Manifest) -> Result<u64, Error> {
        let log_path = LOG_FILE(&options.path, number);
        let out = TEMP_ROOT(&options.path).join(format!("log.{number}.sst"));
        if out.exists() {
            remove_file(&out)?;
        }
        let sst_builder = sst::SstBuilder::new(options.sst.clone(), &out)?;
        // This will return None when the log is empty.
        let sst = match sst::log::log_to_builder(options.log.clone(), &log_path, sst_builder)? {
            Some(sst) => sst,
            None => {
                if let Some(file_name) = log_path.file_name() {
                    rename(&log_path, TRASH_ROOT(&options.path).join(file_name))?;
                }
                return Ok(0);
            }
        };
        let md = sst.metadata()?;
        let setsum = Setsum::from_digest(md.setsum);
        let sst_path = SST_FILE(&options.path, setsum);
        if !sst_path.exists() {
            hard_link(&out, &sst_path)?;
        }
        if !mani.strs().any(|d| *d == setsum.hexdigest()) {
            let input = mani
                .info('O')
                .and_then(|h| Setsum::from_hexdigest(h))
                .unwrap_or_default();
            let discard = Setsum::default() - setsum;
            let output = input - discard;
            let mut edit = Edit::default();
            edit.info('I', &input.hexdigest())?;
            edit.info('O', &output.hexdigest())?;
            edit.info('D', &discard.hexdigest())?;
            edit.add(&setsum.hexdigest())?;
            mani.apply(edit)?;
        }
        remove_file(out)?;
        if let Some(file_name) = log_path.file_name() {
            rename(&log_path, TRASH_ROOT(&options.path).join(file_name))?;
        }
        Ok(md.biggest_timestamp)
    }

    pub fn compaction_thread(&self) -> Result<(), Error> {
        self.poison(self.tree.compaction_thread())
    }

    pub fn memtable_thread(&self) -> Result<(), Error> {
        self.poison(self._memtable_thread())
    }

    pub fn _memtable_thread(&self) -> Result<(), Error> {
        let _memtable_mutex = self.memtable_mutex.lock().unwrap();
        loop {
            let (imm, imm_log, imm_path, imm_trigger) = {
                let mut state = self.state.lock().unwrap();
                while state.imm_trigger < state.mem_seq_no {
                    state = self.cnd_needs_memtable_flush.wait(state).unwrap();
                }
                let imm = Arc::clone(&state.mem);
                let imm_log = Arc::clone(&state.mem_log);
                let imm_trigger = state.mem_seq_no;
                let mut imm_path = LOG_FILE(&self.root, state.seq_no);
                std::mem::swap(&mut imm_path, &mut state.mem_path);
                state.imm = Some(Arc::clone(&state.mem));
                state.mem = Arc::new(MemTable::default());
                state.mem_log = self.poison(Self::start_new_log(
                    &state.mem_path,
                    self.options.log.clone(),
                ))?;
                state.mem_seq_no = state.seq_no;
                state.seq_no += 1;
                let mut wait_guard = self.wait_list.link(());
                while !wait_guard.is_head() {
                    state = wait_guard.naked_wait(state);
                }
                drop(wait_guard);
                self.wait_list.notify_head();
                (imm, imm_log, imm_path, imm_trigger)
            };
            self.poison::<(), Error>(Ok(()))?;
            if Arc::strong_count(&imm_log) != 1 {
                return Err(Error::LogicError {
                    core: ErrorCore::default(),
                    context:
                        "ordering invariant violated; someone still holds a reference to mem_log"
                            .to_string(),
                });
            }
            let imm_setsum = match Arc::try_unwrap(imm_log) {
                Ok(log) => self.poison(log.seal())?.into_inner(),
                Err(_) => {
                    return Err(Error::LogicError {
                        core: ErrorCore::default(),
                        context: "Arc::try_unwrap failed after strong count was confirmed to be 1"
                            .to_string(),
                    });
                }
            };
            let sst_path = TEMP_FILE(&self.root, imm_setsum);
            let mut builder = SstBuilder::new(self.options.sst.clone(), &sst_path)?;
            let mut cursor = imm.cursor();
            cursor.seek_to_first()?;
            while let Some(kvr) = cursor.key_value() {
                match kvr.value {
                    Some(value) => builder.put(kvr.key, kvr.timestamp, value)?,
                    None => builder.del(kvr.key, kvr.timestamp)?,
                };
                cursor.next()?;
            }
            let got_setsum = builder.seal()?.fast_setsum().into_inner();
            if got_setsum != imm_setsum {
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "Memtable checksum inconsistent".to_string(),
                }
                .with_variable("got", got_setsum.hexdigest())
                .with_variable("imm", imm_setsum.hexdigest());
                return Err(err);
            }
            self.tree.ingest(&sst_path)?;
            remove_file(sst_path)?;
            if let Some(file_name) = imm_path.file_name() {
                rename(&imm_path, TRASH_ROOT(&self.root).join(file_name))?;
            }
            let mut state = self.state.lock().unwrap();
            state.imm = None;
            state.imm_trigger = imm_trigger;
            self.cnd_memtable_rolled_over.notify_all();
        }
    }

    fn start_new_log(
        log_path: &PathBuf,
        options: LogOptions,
    ) -> Result<Arc<ConcurrentLogBuilder<File>>, Error> {
        let log = ConcurrentLogBuilder::new(options, log_path)?;
        Ok(Arc::new(log))
    }

    fn rollover_memtable<'a: 'b, 'b>(
        &'a self,
        mut lock_guard: MutexGuard<'b, KeyValueStoreState>,
    ) -> MutexGuard<'b, KeyValueStoreState> {
        lock_guard.imm_trigger = std::cmp::max(lock_guard.imm_trigger, lock_guard.mem_seq_no);
        self.cnd_needs_memtable_flush.notify_one();
        lock_guard
    }

    fn poison<T, E: Into<Error>>(&self, res: Result<T, E>) -> Result<T, Error> {
        // TODO(rescrv): Actually poison here.
        res.map_err(|e| e.into())
    }
}

impl keyvalint::KeyValueStore for KeyValueStore {
    type Error = Error;
    type WriteBatch<'a> = WriteBatch;

    fn put(&self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        let mut wb = WriteBatch::with_capacity(1);
        check_key_len(key)?;
        check_value_len(value)?;
        let key = key.to_vec();
        let value = Some(value.to_vec());
        wb.entries.push(KeyValuePair {
            key,
            timestamp,
            value,
        });
        self.write(wb)
    }

    fn del(&self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        let mut wb = WriteBatch::with_capacity(1);
        check_key_len(key)?;
        let key = key.to_vec();
        let value = None;
        wb.entries.push(KeyValuePair {
            key,
            timestamp,
            value,
        });
        self.write(wb)
    }

    fn write(&self, mut batch: Self::WriteBatch<'_>) -> Result<(), Error> {
        let max_timestamp = batch
            .entries
            .iter()
            .map(|x| x.timestamp)
            .max()
            .unwrap_or(u64::MAX);
        let (mut wait_guard, memtable, log) = {
            let mut state = self.state.lock().unwrap();
            let wait_guard = self.wait_list.link(());
            // TODO(rescrv):  Add a guardrail to prevent max_timestamp from being too far ahead of
            // wall-clock micros.  This is necessary for safety.
            let seq_no = max(state.seq_no + 1, max_timestamp);
            state.seq_no = seq_no;
            for entry in batch.entries.iter_mut() {
                entry.timestamp = seq_no;
            }
            if state.mem.approximate_size() >= self.options.memtable_size_bytes {
                state = self.rollover_memtable(state);
            }
            (
                wait_guard,
                Arc::clone(&state.mem),
                Arc::clone(&state.mem_log),
            )
        };
        let mut log_batch = sst::log::WriteBatch::default();
        for entry in batch.entries.iter() {
            log_batch.insert(KeyValueRef::from(entry))?;
        }
        self.poison(log.append(log_batch))?;
        self.poison(memtable.write(&mut batch))?;
        drop(memtable);
        drop(log);
        let mut state = self.state.lock().unwrap();
        while !wait_guard.is_head() {
            state = wait_guard.naked_wait(state);
        }
        drop(wait_guard);
        self.wait_list.notify_head();
        Ok(())
    }
}

impl keyvalint::KeyValueLoad for KeyValueStore {
    type Error = Error;
    type RangeScan<'a> = BoundsCursor<
        PruningCursor<MergingCursor<Box<dyn keyvalint::Cursor<Error = sst::Error>>>, sst::Error>,
        sst::Error,
    >;

    fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let (mem, imm, tree) = {
            let state = self.state.lock().unwrap();
            let mem = Arc::clone(&state.mem);
            let imm = state.imm.as_ref().map(Arc::clone);
            let tree = self.tree.get_tree();
            (mem, imm, tree)
        };
        *is_tombstone = false;
        let ret = mem.load(key, timestamp, is_tombstone)?;
        if ret.is_some() || *is_tombstone {
            self.tree.explicit_unref(tree);
            return Ok(ret);
        }
        if let Some(imm) = imm {
            let ret = imm.load(key, timestamp, is_tombstone)?;
            if ret.is_some() || *is_tombstone {
                self.tree.explicit_unref(tree);
                return Ok(ret);
            }
        }
        let ret = tree.load(&self.tree.file_manager, key, timestamp, is_tombstone)?;
        self.tree.explicit_unref(tree);
        Ok(ret)
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        let (mem, imm, tree) = {
            let state = self.state.lock().unwrap();
            let mem = Arc::clone(&state.mem);
            let imm = state.imm.as_ref().map(Arc::clone);
            let tree = self.tree.get_tree();
            (mem, imm, tree)
        };
        let mut cursors: Vec<Box<dyn Cursor<Error = sst::Error>>> = Vec::with_capacity(3);
        let mut mem_scan = mem.range_scan(start_bound, end_bound, timestamp)?;
        mem_scan.seek_to_first()?;
        cursors.push(Box::new(mem_scan));
        if let Some(imm) = imm {
            let mut imm_scan = imm.range_scan(start_bound, end_bound, timestamp)?;
            imm_scan.seek_to_first()?;
            cursors.push(Box::new(imm_scan));
        }
        let tree_scan =
            tree.range_scan(&self.tree.file_manager, start_bound, end_bound, timestamp)?;
        cursors.push(Box::new(tree_scan));
        let cursor = MergingCursor::new(cursors)?;
        let cursor = PruningCursor::new(cursor, timestamp)?;
        let cursor = BoundsCursor::new(cursor, start_bound, end_bound)?;
        Ok(cursor)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod test_util {
    use std::fs::remove_dir_all;
    use std::sync::Mutex;

    use super::*;

    pub static SST_FOR_TEST_MUTEX: Mutex<()> = Mutex::new(());

    pub fn test_root(root: &str, line: u32) -> String {
        let root: String = root
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let path = PathBuf::from(format!("{}_{}", root, line));
        if path.exists() {
            remove_dir_all(&path).expect("could not prepare for test");
        }
        String::from(path.to_string_lossy())
    }

    #[macro_export]
    macro_rules! sst_for_test {
        ($output_dir:ident: $($key:literal => $val:literal,)*) => {
            {
                let _mutex = test_util::SST_FOR_TEST_MUTEX.lock().unwrap();
                let tmp = PathBuf::from("sst.tmp");
                if tmp.exists() {
                    std::fs::remove_file(&tmp).as_z().pretty_unwrap();
                }
                std::fs::create_dir_all(&$output_dir).as_z().pretty_unwrap();
                let options = sst::SstOptions::default();
                let mut sst_builder = sst::SstBuilder::new(options, &tmp).as_z().pretty_unwrap();
                $(sst_builder.put($key.as_bytes(), 1, $val.as_bytes()).as_z().pretty_unwrap();)*
                let sst = sst_builder.seal().as_z().pretty_unwrap();
                let setsum = sst.fast_setsum();
                let output = PathBuf::from(&$output_dir).join(format!("{}.sst", setsum.hexdigest()));
                std::fs::rename(&tmp, &output).as_z().pretty_unwrap();
                output
            }
        };
    }

    #[macro_export]
    macro_rules! sst_check {
        ($test_root:expr; $relative_path:literal: $($key:literal => $val:literal,)*) => {
            let sst_path = PathBuf::from($test_root).join($relative_path);
            let sst = Sst::new(SstOptions::default(), sst_path).as_z().pretty_unwrap();
            let mut cursor = sst.cursor();
            cursor.seek_to_first().as_z().pretty_unwrap();
            $(
                cursor.next().as_z().pretty_unwrap();
                let kvr = cursor.key_value().expect("key-value pair should not be none");
                assert_eq!($key.as_bytes(), kvr.key);
                let value: &[u8] = $val.as_bytes();
                assert_eq!(Some(value), kvr.value);
            )*
            cursor.next().as_z().pretty_unwrap();
            assert_eq!(None, cursor.key_value());
        };
    }

    #[macro_export]
    macro_rules! log_for_test {
        ($test_root:expr; $relative_path:literal: $($key:literal => $val:literal,)*) => {
            {
                let _mutex = test_util::SST_FOR_TEST_MUTEX.lock().unwrap();
                let tmp = PathBuf::from("log.tmp");
                if tmp.exists() {
                    std::fs::remove_file(&tmp).as_z().pretty_unwrap();
                }
                let output_file = PathBuf::from($test_root).join($relative_path);
                std::fs::create_dir_all(&output_file.parent().map(PathBuf::from).unwrap_or(PathBuf::from("."))).as_z().pretty_unwrap();
                let options = sst::LogOptions::default();
                let mut log_builder = sst::LogBuilder::new(options, &tmp).as_z().pretty_unwrap();
                $(log_builder.put($key.as_bytes(), 1, $val.as_bytes()).as_z().pretty_unwrap();)*
                let setsum = log_builder.seal().as_z().pretty_unwrap();
                std::fs::rename(&tmp, &output_file).as_z().pretty_unwrap();
                setsum
            }
        };
    }

    #[macro_export]
    macro_rules! log_check {
        ($test_root:expr; $relative_path:literal: $($key:literal => $val:literal,)*) => {
            let log_path = PathBuf::from($test_root).join($relative_path);
            let mut log = sst::LogIterator::new(sst::LogOptions::default(), log_path).as_z().pretty_unwrap();
            $(
                let kvr = log.next().as_z().pretty_unwrap().expect("next should not be None");
                assert_eq!($key.as_bytes(), kvr.key);
                let value: &[u8] = $val.as_bytes();
                assert_eq!(Some(value), kvr.value);
            )*
            assert_eq!(None, log.next().as_z().pretty_unwrap());
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sst_for_test() {
        let test_root = PathBuf::from(test_util::test_root(module_path!(), line!()));
        let sst_root = test_root.join("sst");
        let _output = sst_for_test! {
            sst_root:
            "key1" => "value1",
            "key2" => "value2",
        };
        sst_check! {
            &test_root; "sst/fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a.sst":
            "key1" => "value1",
            "key2" => "value2",
        };
    }

    #[test]
    fn test_log_for_test() {
        let test_root = PathBuf::from(test_util::test_root(module_path!(), line!()));
        let setsum = log_for_test! {
            &test_root; "log.0":
            "key2" => "value2",
            "key1" => "value1",
        };
        assert_eq!(
            "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a",
            setsum.hexdigest()
        );
        log_check! {
            &test_root; "log.0":
            "key2" => "value2",
            "key1" => "value1",
        };
    }

    #[test]
    fn empty_kvs() {
        let root = test_util::test_root(module_path!(), line!());
        let options = LsmtkOptions {
            path: root.clone(),
            ..Default::default()
        };
        KeyValueStore::open(options).expect("key-value store should open");
    }

    #[test]
    fn log_no_sst() {
        let root = test_util::test_root(module_path!(), line!());
        let options = LsmtkOptions {
            path: root.clone(),
            ..Default::default()
        };
        let _setsum = log_for_test! {
            &root; "log.0":
            "key2" => "value2",
            "key1" => "value1",
        };
        KeyValueStore::open(options).as_z().pretty_unwrap();
        sst_check! {
            &root; "sst/fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a.sst":
            "key1" => "value1",
            "key2" => "value2",
        };
        assert!(!PathBuf::from(&root).join("log.0").exists());
        // TODO(rescrv): kvs_check
    }

    #[test]
    fn log_sst_no_manifest() {
        let root = test_util::test_root(module_path!(), line!());
        let options = LsmtkOptions {
            path: root.clone(),
            ..Default::default()
        };
        let _setsum = log_for_test! {
            &root; "log.0":
            "key2" => "value2",
            "key1" => "value1",
        };
        let sst_root = SST_ROOT(&root);
        let _path = sst_for_test! {
            sst_root:
            "key1" => "value1",
            "key2" => "value2",
        };
        let _kvs = KeyValueStore::open(options).expect("key-value store should open");
        sst_check! {
            &root; "sst/fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a.sst":
            "key1" => "value1",
            "key2" => "value2",
        };
        assert!(!PathBuf::from(&root).join("log.0").exists());
        // TODO(rescrv): kvs_check
    }

    #[test]
    fn log_sst_manifest_no_rm() {
        let root = test_util::test_root(module_path!(), line!());
        let options = LsmtkOptions {
            path: root.clone(),
            ..Default::default()
        };
        let _setsum = log_for_test! {
            &root; "log.0":
            "key2" => "value2",
            "key1" => "value1",
        };
        let sst_root = SST_ROOT(&root);
        let _path = sst_for_test! {
            sst_root:
            "key1" => "value1",
            "key2" => "value2",
        };
        let mut mani =
            Manifest::open(options.mani.clone(), MANI_ROOT(&root)).expect("manifest should open");
        let mut edit = Edit::default();
        edit.add("fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a")
            .expect("manifest edit should never fail");
        edit.info(
            'I',
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("manifest info should never fail");
        edit.info(
            'O',
            "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a",
        )
        .expect("manifest info should never fail");
        edit.info(
            'D',
            "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a",
        )
        .expect("manifest info should never fail");
        mani.apply(edit).expect("manifest apply should never fail");
        drop(mani);
        let _kvs = KeyValueStore::open(options).expect("key-value store should open");
        sst_check! {
            &root; "sst/fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a.sst":
            "key1" => "value1",
            "key2" => "value2",
        };
        assert!(!PathBuf::from(&root).join("log.0").exists());
        // TODO(rescrv): kvs_check
    }

    // TODO(rescrv): two log files
}
