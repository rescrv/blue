use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::{create_dir, hard_link, remove_dir, remove_dir_all, remove_file, rename};
use std::io::ErrorKind;
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, RwLock};

use biometrics::{Collector, Counter};
use indicio::clue;
use keyvalint::{compare_bytes, Cursor, KeyRef};
use mani::{Edit, Manifest, ManifestIterator};
use one_two_eight::{generate_id, generate_id_prototk};
use setsum::Setsum;
use sst::concat_cursor::ConcatenatingCursor;
use sst::file_manager::FileManager;
use sst::lazy_cursor::LazyCursor;
use sst::merging_cursor::MergingCursor;
use sst::pruning_cursor::PruningCursor;
use sst::{Builder, Sst, SstCursor, SstMetadata, SstMultiBuilder};
use sync42::lru::{LeastRecentlyUsedCache, Value as LruValue};
use zerror::Z;
use zerror_core::ErrorCore;

use super::{
    ensure_dir, make_all_dirs, Error, IoToZ, LsmtkOptions, TreeLogKey, TreeLogValue,
    COMPACTION_DIR, LSM_TREE_LOG, MANI_ROOT, SST_FILE, TRASH_SST,
};
use crate::reference_counter::ReferenceCounter;
use crate::verifier;

mod recover;

use recover::recover;

///////////////////////////////////////////// constants ////////////////////////////////////////////

pub const NUM_LEVELS: usize = 16;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static OPEN_DB: Counter = Counter::new("lsmtk.open");

static BYTES_INGESTED: Counter = Counter::new("lsmtk.ingest.bytes");
static INGEST_LINK: Counter = Counter::new("lsmtk.ingest.link");
static INGEST_STALL: Counter = Counter::new("lsmtk.ingest.stall");

static FIND_TRIVIAL_MOVE: Counter = Counter::new("lsmtk.tree.find_trivial_move.find_trivial_move");
static FIND_TRIVIAL_MOVE_EMPTY_LEVEL: Counter =
    Counter::new("lsmtk.tree.find_trivial_move.find_trivial_move_empty_level");
static FIND_TRIVIAL_MOVE_LEVEL0: Counter =
    Counter::new("lsmtk.tree.find_trivial_move.find_trivial_move_level0");
static FIND_TRIVIAL_MOVE_LEVELN: Counter =
    Counter::new("lsmtk.tree.find_trivial_move.find_trivial_move_leveln");
static FIND_TRIVIAL_MOVE_SST: Counter =
    Counter::new("lsmtk.tree.find_trivial_move.find_trivial_move_sst");
static FOUND_TRIVIAL_MOVE: Counter = Counter::new("lsmtk.tree.found_trivial_move");
static FIND_BEST_COMPACTION: Counter = Counter::new("lsmtk.tree.find_best_compaction");
static FIND_BEST_COMPACTION_LEVEL: Counter = Counter::new("lsmtk.tree.find_best_compaction.level");
static FIND_BEST_COMPACTION_ADD_INPUT: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.add_input");
static FIND_BEST_COMPACTION_MAX_BYTES_EXCEEDED: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.max_bytes_exceeded");
static FIND_BEST_COMPACTION_MAX_FILES_EXCEEDED: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.max_files_exceeded");
static FIND_BEST_COMPACTION_NEW_BEST_SCORE: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.new_best_score");
static FIND_BEST_COMPACTION_MAY_CHOOSE: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.may_choose");
static FIND_BEST_COMPACTION_EMPTY_LEVEL: Counter =
    Counter::new("lsmtk.tree.find_best_compaction.empty_level");
static MAY_CHOOSE_COMPACTION: Counter = Counter::new("lsmtk.tree.may_choose_compaction");
static MAY_NOT_CHOOSE_LEVELS_EQUAL: Counter =
    Counter::new("lsmtk.tree.may_not_choose.levels_equal");
static MAY_NOT_CHOOSE_TOO_MANY_FILES: Counter =
    Counter::new("lsmtk.tree.may_not_choose.too_many_files");
static MAY_NOT_CHOOSE_CONFLICT: Counter = Counter::new("lsmtk.tree.may_not_choose.conflict");
static MANDATORY_COMPACTION: Counter = Counter::new("lsmtk.tree.mandatory_compaction");
static SKIPPED_FOR_CURVE: Counter = Counter::new("lsmtk.tree.skipped_for_curve");
static CLEAR_OUT_FOR_L0: Counter = Counter::new("lsmtk.tree.clear_out_for_l0");

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
    collector.register_counter(&FIND_TRIVIAL_MOVE);
    collector.register_counter(&FIND_TRIVIAL_MOVE_EMPTY_LEVEL);
    collector.register_counter(&FIND_TRIVIAL_MOVE_LEVEL0);
    collector.register_counter(&FIND_TRIVIAL_MOVE_LEVELN);
    collector.register_counter(&FIND_TRIVIAL_MOVE_SST);
    collector.register_counter(&FOUND_TRIVIAL_MOVE);
    collector.register_counter(&FIND_BEST_COMPACTION);
    collector.register_counter(&FIND_BEST_COMPACTION_LEVEL);
    collector.register_counter(&FIND_BEST_COMPACTION_ADD_INPUT);
    collector.register_counter(&FIND_BEST_COMPACTION_MAX_BYTES_EXCEEDED);
    collector.register_counter(&FIND_BEST_COMPACTION_MAX_FILES_EXCEEDED);
    collector.register_counter(&FIND_BEST_COMPACTION_NEW_BEST_SCORE);
    collector.register_counter(&FIND_BEST_COMPACTION_MAY_CHOOSE);
    collector.register_counter(&FIND_BEST_COMPACTION_EMPTY_LEVEL);
    collector.register_counter(&MAY_CHOOSE_COMPACTION);
    collector.register_counter(&MAY_NOT_CHOOSE_LEVELS_EQUAL);
    collector.register_counter(&MAY_NOT_CHOOSE_TOO_MANY_FILES);
    collector.register_counter(&MAY_NOT_CHOOSE_CONFLICT);
    collector.register_counter(&MANDATORY_COMPACTION);
    collector.register_counter(&SKIPPED_FOR_CURVE);
    collector.register_counter(&CLEAR_OUT_FOR_L0);
    collector.register_counter(&COMPACTION_THREAD_NO_COMPACTION);
    collector.register_counter(&COMPACTION_PERFORM);
    collector.register_counter(&COMPACTION_NEW_CURSOR);
    collector.register_counter(&COMPACTION_KEYS_WRITTEN);
    collector.register_counter(&COMPACTION_BYTES_WRITTEN);
    collector.register_counter(&COMPACTION_LINK);
    collector.register_counter(&COMPACTION_REMOVE);
    collector.register_counter(&GARBAGE_COLLECTION_PERFORM);
    collector.register_counter(&GARBAGE_COLLECTION_KEYS_DROPPED);
}

/////////////////////////////////////////////// Level //////////////////////////////////////////////

#[derive(Clone, Default)]
struct Level {
    ssts: Vec<Arc<SstMetadata>>,
}

impl Level {
    fn size(&self) -> u64 {
        self.ssts
            .iter()
            .map(|x| x.file_size)
            .fold(0, u64::saturating_add)
    }

    fn lower_bound(&self, key: &[u8]) -> usize {
        self.ssts
            .partition_point(|x| compare_bytes(key, &x.last_key) == Ordering::Greater)
    }

    fn upper_bound(&self, key: &[u8]) -> usize {
        self.ssts
            .partition_point(|x| compare_bytes(key, &x.first_key) != Ordering::Less)
    }
}

//////////////////////////////////////////// LevelSlice ////////////////////////////////////////////

#[derive(Clone, Default)]
struct LevelSlice<'a> {
    lower_bound: usize,
    upper_bound: usize,
    first_key: &'a [u8],
    last_key: &'a [u8],
}

////////////////////////////////////////// CompactionCore //////////////////////////////////////////

#[derive(Debug)]
struct CompactionCore {
    compaction_id: CompactionID,
    lower_level: usize,
    upper_level: usize,
    first_key: Vec<u8>,
    last_key: Vec<u8>,
    inputs: Vec<Setsum>,
    size: u64,
}

impl CompactionCore {
    fn overlapping(lhs: &Self, rhs: &Self) -> bool {
        lhs.lower_level <= rhs.upper_level
            && rhs.lower_level <= lhs.upper_level
            && compare_bytes(&lhs.first_key, &rhs.last_key) != Ordering::Greater
            && compare_bytes(&rhs.first_key, &lhs.last_key) != Ordering::Greater
    }
}

/////////////////////////////////////////// CompactionID ///////////////////////////////////////////

generate_id! {CompactionID, "compaction:"}
generate_id_prototk! {CompactionID}

//////////////////////////////////////////// Compaction ////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct Compaction {
    core: Arc<CompactionCore>,
}

impl Compaction {
    pub fn compaction_id(&self) -> CompactionID {
        self.core.compaction_id
    }

    pub fn top_level(&self) -> bool {
        self.core.upper_level == NUM_LEVELS - 1
    }

    pub fn inputs(&self) -> impl Iterator<Item = Setsum> + '_ {
        self.core.inputs.iter().copied()
    }
}

////////////////////////////////////////////// Version /////////////////////////////////////////////

// TODO(rescrv): Hide this
pub(crate) struct Version {
    options: LsmtkOptions,
    levels: Vec<Level>,
    ongoing: Arc<Mutex<Vec<Arc<CompactionCore>>>>,
}

// TODO(rescrv): make compare_bytes this signature.
fn compare_for_min_max(lhs: &&[u8], rhs: &&[u8]) -> Ordering {
    compare_bytes(lhs, rhs)
}

impl Version {
    fn open(options: LsmtkOptions, metadata: Vec<SstMetadata>) -> Result<Self, Error> {
        recover(options, metadata)
    }

    fn should_stall_ingest(&self) -> bool {
        self.levels[0].ssts.len() >= self.options.l0_write_stall_threshold_files
            || self.levels[0].size() >= self.options.l0_write_stall_threshold_bytes as u64
    }

    fn should_perform_mandatory_compaction(&self) -> bool {
        self.levels[0].ssts.len() >= self.options.l0_mandatory_compaction_threshold_files
            || self.levels[0].size() >= self.options.l0_mandatory_compaction_threshold_bytes as u64
            || self.levels.iter().all(|x| !x.ssts.is_empty())
    }

    fn setsums(&self) -> Vec<Setsum> {
        let mut setsums = vec![];
        for level in self.levels.iter() {
            for md in level.ssts.iter() {
                setsums.push(Setsum::from_digest(md.setsum));
            }
        }
        setsums
    }

    fn ingest(&self, to_add: SstMetadata) -> Result<Self, Error> {
        // TODO(rescrv):  Put it at the highest level with a hole.
        clue! { LSM_TREE_LOG, TreeLogKey::BySetsum {
                setsum: to_add.setsum
            } => TreeLogValue::Ingest {
                level: 0,
                cardinality: self.levels[0].ssts.len() + 1,
            }
        };
        let mut new_tree = self.clone();
        new_tree.levels[0].ssts.push(Arc::new(to_add));
        Ok(new_tree)
    }

    fn compute_setsum(&self) -> Setsum {
        let mut acc = Setsum::default();
        for level in self.levels.iter() {
            for file in level.ssts.iter() {
                acc += Setsum::from_digest(file.setsum);
            }
        }
        acc
    }

    fn load(
        &self,
        fm: &FileManager,
        sc: &LeastRecentlyUsedCache<Setsum, CachedSst>,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        *is_tombstone = false;
        let mut level0 = self.levels[0].ssts.clone();
        level0.sort_by_key(|md| md.biggest_timestamp);
        for l0 in level0.into_iter().rev() {
            let ret = self.load_from_sst(fm, sc, &l0, key, timestamp, is_tombstone)?;
            if ret.is_some() || *is_tombstone {
                return Ok(ret);
            }
        }
        for level in self.levels[1..].iter() {
            let lower_bound = level.lower_bound(key);
            let upper_bound = level.upper_bound(key);
            for sst in level.ssts[lower_bound..upper_bound].iter() {
                let ret = self.load_from_sst(fm, sc, sst, key, timestamp, is_tombstone)?;
                if ret.is_some() || *is_tombstone {
                    return Ok(ret);
                }
            }
        }
        Ok(None)
    }

    fn load_from_sst<'a: 'b, 'b>(
        &self,
        fm: &FileManager,
        sc: &LeastRecentlyUsedCache<Setsum, CachedSst>,
        md: &SstMetadata,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        let setsum = Setsum::from_digest(md.setsum);
        let sst = if let Some(sst) = sc.lookup(setsum) {
            sst.ptr
        } else {
            let sst_path = SST_FILE(&self.options.path, setsum);
            let file = fm.open(sst_path)?;
            let sst = Arc::new(Sst::from_file_handle(file)?);
            sc.insert(
                setsum,
                CachedSst {
                    ptr: Arc::clone(&sst),
                },
            );
            sst
        };
        Ok(sst.load(key, timestamp, is_tombstone)?)
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        fm: &Arc<FileManager>,
        sc: &Arc<LeastRecentlyUsedCache<Setsum, CachedSst>>,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<MergingCursor<Box<dyn Cursor<Error = sst::Error>>>, Error> {
        fn lazy_cursor(fm: &FileManager, sc: &LeastRecentlyUsedCache<Setsum, CachedSst>, root: &str, setsum: Setsum) -> Result<SstCursor, sst::Error> {
            if let Some(sst) = sc.lookup(setsum) {
                Ok(sst.ptr.cursor())
            } else {
                let sst_path = SST_FILE(root, setsum);
                let handle = fm.open(sst_path)?;
                let sst = Sst::from_file_handle(handle)?;
                Ok(sst.cursor())
            }
        }
        let mut cursors: Vec<Box<dyn Cursor<Error = sst::Error>>> = vec![];
        for sst in self.levels[0].ssts.iter() {
            let fm = Arc::clone(fm);
            let sc = Arc::clone(sc);
            let root = self.options.path.clone();
            let setsum = Setsum::from_digest(sst.setsum);
            let lazy = move || {
                lazy_cursor(&fm, &sc, &root, setsum)
            };
            cursors.push(Box::new(PruningCursor::new(
                LazyCursor::new(lazy),
                timestamp,
            )?));
        }
        fn bound_to_bound<U: AsRef<[u8]>>(u: &Bound<U>) -> Bound<&[u8]> {
            match u {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(x) => Bound::Included(x.as_ref()),
                Bound::Excluded(x) => Bound::Excluded(x.as_ref()),
            }
        }
        let start_bound = bound_to_bound(start_bound);
        let end_bound = bound_to_bound(end_bound);
        fn compare_bounds_le<T: AsRef<[u8]>, U: AsRef<[u8]>>(lhs: Bound<T>, rhs: Bound<U>) -> bool {
            let lhs = bound_to_bound(&lhs);
            let rhs = bound_to_bound(&rhs);
            match (lhs, rhs) {
                (Bound::Unbounded, Bound::Unbounded) => true,
                (Bound::Unbounded, Bound::Included(_)) => true,
                (Bound::Unbounded, Bound::Excluded(_)) => true,
                (Bound::Included(_), Bound::Unbounded) => true,
                (Bound::Included(x), Bound::Included(y)) => compare_bytes(x, y).is_le(),
                (Bound::Included(x), Bound::Excluded(y)) => compare_bytes(x, y).is_lt(),
                (Bound::Excluded(_), Bound::Unbounded) => true,
                (Bound::Excluded(x), Bound::Included(y)) => compare_bytes(x, y).is_lt(),
                (Bound::Excluded(x), Bound::Excluded(y)) => compare_bytes(x, y).is_lt(),
            }
        }
        for level in self.levels[1..].iter() {
            let mut this_level_cursors = vec![];
            for sst in level.ssts.iter() {
                let sb = Bound::Included(&sst.first_key);
                let eb = Bound::Included(&sst.last_key);
                // TODO(rescrv): Use lower_bound and upper_bound functions to speed this up.
                if compare_bounds_le(start_bound, eb) && compare_bounds_le(sb, end_bound) {
                    let fm = Arc::clone(fm);
                    let sc = Arc::clone(sc);
                    let root = self.options.path.clone();
                    let setsum = Setsum::from_digest(sst.setsum);
                    let lazy = move || {
                        lazy_cursor(&fm, &sc, &root, setsum)
                    };
                    this_level_cursors.push(PruningCursor::new(
                        LazyCursor::new(lazy),
                        timestamp,
                    )?);
                }
            }
            if !this_level_cursors.is_empty() {
                cursors.push(Box::new(ConcatenatingCursor::new(this_level_cursors)?));
            }
        }
        Ok(MergingCursor::new(cursors)?)
    }

    fn next_compaction(&self) -> Option<Compaction> {
        let compaction_id = match CompactionID::generate() {
            Some(compaction_id) => compaction_id,
            None => CompactionID::BOTTOM,
        };
        for lower_level in 0..self.levels.len() - 1 {
            if let (Some(compaction), score) = self.find_trivial_move(compaction_id, lower_level) {
                return self.emit_compaction(compaction_id, compaction, score);
            }
        }
        let mut candidate = None;
        let mut best_score = i64::MIN;
        let mut mandatory = None;
        let mut mandatory_score = i64::MIN;
        if !self.levels[0].ssts.is_empty() {
            // SAFETY(rescrv):  There must be a max because the level is non-empty.
            let first_key: &[u8] = self.levels[0]
                .ssts
                .iter()
                .map(|x| x.first_key.as_slice())
                .min_by(compare_for_min_max)
                .unwrap();
            let last_key: &[u8] = self.levels[0]
                .ssts
                .iter()
                .map(|x| x.last_key.as_slice())
                .max_by(compare_for_min_max)
                .unwrap();
            let bounds = self.compute_bounds(0, first_key, last_key);
            if let (Some(compaction), score) = self.find_best_compaction(compaction_id, 0, bounds) {
                if self.should_perform_mandatory_compaction() {
                    MANDATORY_COMPACTION.click();
                    mandatory = Some(compaction);
                    mandatory_score = score;
                } else {
                    candidate = Some(compaction);
                    best_score = score;
                }
            }
        }
        for lower_level in (1..self.levels.len() - 1).rev() {
            fn level_curve(level: usize) -> u64 {
                if level <= 2 {
                    1
                } else {
                    (level as f64).log10().ceil() as u64 + 1
                }
            }
            if self.levels[lower_level].size() / level_curve(lower_level)
                > self.levels[lower_level - 1].size()
                && !self.should_perform_mandatory_compaction()
            {
                SKIPPED_FOR_CURVE.click();
                continue;
            }
            for sst in self.levels[lower_level].ssts.iter() {
                let first_key: &[u8] = &sst.first_key;
                let last_key: &[u8] = &sst.last_key;
                let bounds = self.compute_bounds(lower_level, first_key, last_key);
                let level_factor =
                    (lower_level as f64 + 1.0).log2() / (lower_level + 1) as f64 + 1.0;
                if let (Some(compaction), score) =
                    self.find_best_compaction(compaction_id, lower_level, bounds)
                {
                    if self.should_perform_mandatory_compaction()
                        && self.levels[lower_level].ssts.iter().all(|x| {
                            compaction
                                .core
                                .inputs
                                .contains(&Setsum::from_digest(x.setsum))
                        })
                        && compaction.core.size
                            < mandatory.as_ref().map(|x| x.core.size).unwrap_or_default()
                    {
                        CLEAR_OUT_FOR_L0.click();
                        mandatory = Some(compaction);
                        mandatory_score = (score as f64 * level_factor).ceil() as i64;
                    } else if score > best_score {
                        candidate = Some(compaction);
                        best_score = (score as f64 * level_factor).ceil() as i64;
                    }
                }
            }
        }
        if let Some(mandatory) = mandatory {
            self.emit_compaction(compaction_id, mandatory, mandatory_score)
        } else if let Some(candidate) = candidate {
            if best_score >= 0 {
                self.emit_compaction(compaction_id, candidate, best_score)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn release_compaction(&self, compaction: Compaction) -> Result<(), Error> {
        let mut ongoing_list = self.ongoing.lock().unwrap();
        for (idx, ongoing) in ongoing_list.iter().enumerate() {
            if Arc::ptr_eq(ongoing, &compaction.core) {
                ongoing_list.swap_remove(idx);
                return Ok(());
            }
        }
        Err(Error::LogicError {
            core: ErrorCore::default(),
            context: "Provided a compaction that is not ongoing".to_string(),
        })
    }

    fn apply_compaction(
        &self,
        compaction: Compaction,
        outputs: Vec<SstMetadata>,
    ) -> Result<Self, Error> {
        let mut ongoing_list = self.ongoing.lock().unwrap();
        for (idx, ongoing) in ongoing_list.iter().enumerate() {
            if Arc::ptr_eq(ongoing, &compaction.core) {
                ongoing_list.swap_remove(idx);
                return self.apply_compaction_inner(compaction.core, outputs);
            }
        }
        Err(Error::LogicError {
            core: ErrorCore::default(),
            context: "Provided a compaction that is not ongoing".to_string(),
        })
    }

    fn apply_compaction_inner(
        &self,
        compaction: Arc<CompactionCore>,
        outputs: Vec<SstMetadata>,
    ) -> Result<Self, Error> {
        clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                compaction_id: compaction.compaction_id,
            } => TreeLogValue::ApplyCompaction {
                outputs: outputs.iter().map(|x| x.setsum).collect(),
            }
        };
        let mut new_tree = self.clone();
        // NOTE(rescrv): Intentionally do not include upper level.
        for level in compaction.lower_level..compaction.upper_level {
            let this_level = &self.levels[level];
            let mut new_level = this_level.clone();
            new_level
                .ssts
                .retain(|x| !compaction.inputs.contains(&Setsum::from_digest(x.setsum)));
            clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                    compaction_id: compaction.compaction_id,
                } => TreeLogValue::CompactLevel {
                    level,
                    before: this_level.ssts.iter().map(|x| x.setsum).collect(),
                    after: new_level.ssts.iter().map(|x| x.setsum).collect(),
                }
            };
            new_tree.levels[level] = new_level;
        }
        let upper_level = &new_tree.levels[compaction.upper_level];
        let lower_bound = upper_level.lower_bound(&compaction.first_key);
        let upper_bound = upper_level.upper_bound(&compaction.last_key);
        let mut new_level = Level {
            ssts: Vec::with_capacity(
                upper_level.ssts.len() - (upper_bound - lower_bound) + outputs.len(),
            ),
        };
        new_level
            .ssts
            .extend_from_slice(&upper_level.ssts[..lower_bound]);
        let mut outputs = outputs.into_iter().map(Arc::new).collect::<Vec<_>>();
        new_level.ssts.append(&mut outputs);
        new_level
            .ssts
            .extend_from_slice(&upper_level.ssts[upper_bound..]);
        clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                compaction_id: compaction.compaction_id,
            } => TreeLogValue::CompactLevel {
                level: compaction.upper_level,
                before: upper_level.ssts.iter().map(|x| x.setsum).collect(),
                after: new_level.ssts.iter().map(|x| x.setsum).collect(),
            }
        };
        clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                compaction_id: compaction.compaction_id,
            } => TreeLogValue::CompactUpperLevelBounds {
                level: compaction.upper_level,
                lower_bound,
                upper_bound,
            }
        };
        new_tree.levels[compaction.upper_level] = new_level;
        Ok(new_tree)
    }

    fn compute_bounds<'a>(
        &'a self,
        lower_level: usize,
        mut first_key: &'a [u8],
        mut last_key: &'a [u8],
    ) -> Vec<LevelSlice> {
        let mut bounds = Vec::with_capacity(NUM_LEVELS);
        for _ in 0..lower_level {
            bounds.push(LevelSlice {
                lower_bound: 0,
                upper_bound: 0,
                first_key: &[],
                last_key: &[],
            });
        }
        for upper_level in lower_level..self.levels.len() {
            if upper_level == 0 {
                let this_level = &self.levels[0];
                let lower_bound = 0;
                let upper_bound = this_level.ssts.len();
                // SAFTEY(rescrv):  This should never trigger because the one place where we call
                // compute_bounds with lower_level=0 computes first_key and last_key to be the min
                // and max keys for level 0 respectively.
                assert!(this_level
                    .ssts
                    .iter()
                    .all(|x| compare_bytes(first_key, &x.first_key) != Ordering::Greater));
                assert!(this_level
                    .ssts
                    .iter()
                    .all(|x| compare_bytes(&x.last_key, last_key) != Ordering::Greater));
                bounds.push(LevelSlice {
                    lower_bound,
                    upper_bound,
                    first_key,
                    last_key,
                });
            } else {
                let this_level = &self.levels[upper_level];
                let mut lower_bound = this_level.lower_bound(first_key);
                let mut upper_bound = this_level.upper_bound(last_key);
                let mut fixed_point = false;
                while !fixed_point {
                    fixed_point = true;
                    if lower_bound < this_level.ssts.len()
                        && compare_bytes(&this_level.ssts[lower_bound].first_key, first_key).is_lt()
                    {
                        fixed_point = false;
                        first_key = &this_level.ssts[lower_bound].first_key.as_slice();
                    }
                    if upper_bound > lower_bound
                        && compare_bytes(&this_level.ssts[upper_bound - 1].last_key, last_key)
                            .is_gt()
                    {
                        fixed_point = false;
                        last_key = &this_level.ssts[upper_bound - 1].last_key.as_slice();
                    }
                    let new_lower_bound = this_level.lower_bound(first_key);
                    let new_upper_bound = this_level.upper_bound(last_key);
                    fixed_point = fixed_point
                        && new_lower_bound == lower_bound
                        && new_upper_bound == upper_bound;
                    lower_bound = new_lower_bound;
                    upper_bound = new_upper_bound;
                }
                assert_eq!(lower_bound, this_level.lower_bound(first_key));
                assert_eq!(upper_bound, this_level.upper_bound(last_key));
                bounds.push(LevelSlice {
                    lower_bound,
                    upper_bound,
                    first_key,
                    last_key,
                });
            }
        }
        bounds
    }

    fn find_trivial_move(
        &self,
        compaction_id: CompactionID,
        level: usize,
    ) -> (Option<Compaction>, i64) {
        // SAFETY(rescrv):  This should always be ensured by the caller.
        assert!(level < self.levels.len());
        FIND_TRIVIAL_MOVE.click();
        if self.levels[level].ssts.is_empty() {
            FIND_TRIVIAL_MOVE_EMPTY_LEVEL.click();
            return (None, i64::MIN);
        }
        if level == 0 {
            // SAFETY(rescrv):  This is guaranteed safe by the if is_empty() check above.
            let sst = self.levels[0]
                .ssts
                .iter()
                .min_by(|lhs, rhs| lhs.biggest_timestamp.cmp(&rhs.biggest_timestamp))
                .unwrap();
            FIND_TRIVIAL_MOVE_LEVEL0.click();
            return self.find_trivial_move_for_one_sst(compaction_id, level, sst);
        } else {
            FIND_TRIVIAL_MOVE_LEVELN.click();
            for sst in self.levels[level].ssts.iter() {
                FIND_TRIVIAL_MOVE_SST.click();
                if let (Some(compaction), score) =
                    self.find_trivial_move_for_one_sst(compaction_id, level, sst)
                {
                    FOUND_TRIVIAL_MOVE.click();
                    return (Some(compaction), score);
                }
            }
        }
        (None, i64::MIN)
    }

    fn find_trivial_move_for_one_sst(
        &self,
        compaction_id: CompactionID,
        lower_level: usize,
        sst: &Arc<SstMetadata>,
    ) -> (Option<Compaction>, i64) {
        let first_key: &[u8] = &sst.first_key;
        let last_key: &[u8] = &sst.last_key;
        let upper_level = lower_level + 1;
        if upper_level < self.levels.len()
            && self.levels[upper_level].lower_bound(first_key)
                == self.levels[upper_level].upper_bound(last_key)
        {
            let first_key = first_key.to_vec();
            let last_key = last_key.to_vec();
            let inputs = vec![Setsum::from_digest(sst.setsum)];
            let size = sst.file_size;
            let core = CompactionCore {
                compaction_id,
                lower_level,
                upper_level,
                first_key,
                last_key,
                inputs,
                size,
            };
            if self.may_choose_compaction(&core) {
                return (
                    Some(Compaction {
                        core: Arc::new(core),
                    }),
                    sst.file_size as i64,
                );
            }
        }
        (None, i64::MIN)
    }

    fn find_best_compaction(
        &self,
        compaction_id: CompactionID,
        lower_level: usize,
        bounds: Vec<LevelSlice>,
    ) -> (Option<Compaction>, i64) {
        // SAFETY(rescrv):  This should always be ensured by the caller.
        assert!(lower_level < self.levels.len());
        assert!(!self.levels[lower_level].ssts.is_empty());
        FIND_BEST_COMPACTION.click();
        let mut candidate = None;
        let mut best_score = i64::MIN;
        let mut overlap = [0i64; NUM_LEVELS];
        let mut inputs = vec![];
        for upper_level in lower_level..self.levels.len() {
            FIND_BEST_COMPACTION_LEVEL.click();
            let this_level = &self.levels[upper_level];
            let LevelSlice {
                lower_bound,
                upper_bound,
                first_key,
                last_key,
            } = bounds[upper_level];
            for idx in lower_bound..upper_bound {
                overlap[upper_level] =
                    overlap[upper_level].saturating_add(this_level.ssts[idx].file_size as i64);
                assert!(
                    compare_bytes(first_key, &this_level.ssts[idx].first_key) != Ordering::Greater
                );
                assert!(
                    compare_bytes(&this_level.ssts[idx].last_key, last_key) != Ordering::Greater
                );
                inputs.push(Setsum::from_digest(this_level.ssts[idx].setsum));
                FIND_BEST_COMPACTION_ADD_INPUT.click();
            }
            let acc = overlap[lower_level..upper_level]
                .iter()
                .copied()
                .fold(0i64, |lhs, rhs| lhs.saturating_add(lhs).saturating_add(rhs));
            let score = acc - overlap[upper_level];
            let compaction_size = overlap[lower_level..=upper_level]
                .iter()
                .copied()
                .fold(0i64, i64::saturating_add);
            if compaction_size > self.options.max_compaction_bytes as i64 && lower_level != 0 {
                FIND_BEST_COMPACTION_MAX_BYTES_EXCEEDED.click();
                return (candidate, best_score);
            }
            if inputs.len() > self.options.max_compaction_files
                || inputs.len() > self.options.max_open_files
            {
                FIND_BEST_COMPACTION_MAX_FILES_EXCEEDED.click();
                return (candidate, best_score);
            }
            if lower_level < upper_level && score > best_score {
                FIND_BEST_COMPACTION_NEW_BEST_SCORE.click();
                let first_key = first_key.to_vec();
                let last_key = last_key.to_vec();
                let inputs = inputs.clone();
                let size = compaction_size as u64;
                let mut core = CompactionCore {
                    compaction_id,
                    lower_level,
                    upper_level,
                    first_key,
                    last_key,
                    inputs,
                    size,
                };
                self.expand_compaction(&mut core);
                if self.may_choose_compaction(&core) {
                    FIND_BEST_COMPACTION_MAY_CHOOSE.click();
                    candidate = Some(Compaction {
                        core: Arc::new(core),
                    });
                    best_score = score;
                }
            }
            if lower_bound == upper_bound {
                FIND_BEST_COMPACTION_EMPTY_LEVEL.click();
                break;
            }
        }
        (candidate, best_score)
    }

    fn expand_compaction(&self, compaction: &mut CompactionCore) {
        let mut first_key: &[u8] = &compaction.first_key;
        let mut last_key: &[u8] = &compaction.last_key;
        for level in (compaction.lower_level..=compaction.upper_level).rev() {
            let this_level = &self.levels[level];
            let mut to_add = vec![];
            // TODO(rescrv): Make this efficient now that it's correct.
            for sst in this_level.ssts.iter() {
                let num_inputs = compaction.inputs.len() + to_add.len();
                if num_inputs > self.options.max_compaction_files
                    || num_inputs > self.options.max_open_files
                {
                    return;
                }
                if compare_bytes(first_key, &sst.first_key) != Ordering::Greater
                    && compare_bytes(&sst.last_key, last_key) != Ordering::Greater
                    && !compaction.inputs.contains(&Setsum::from_digest(sst.setsum))
                {
                    to_add.push(sst);
                }
            }
            if !to_add.is_empty() {
                // SAFETY(rescrv): It's non-empty so min/max exist.
                first_key = to_add
                    .iter()
                    .map(|x| x.first_key.as_slice())
                    .min_by(compare_for_min_max)
                    .unwrap();
                last_key = to_add
                    .iter()
                    .map(|x| x.last_key.as_slice())
                    .max_by(compare_for_min_max)
                    .unwrap();
                let mut to_add = to_add
                    .into_iter()
                    .map(|x| Setsum::from_digest(x.setsum))
                    .collect();
                compaction.inputs.append(&mut to_add);
            }
        }
    }

    fn may_choose_compaction(&self, core: &CompactionCore) -> bool {
        if core.lower_level == core.upper_level {
            MAY_NOT_CHOOSE_LEVELS_EQUAL.click();
            return false;
        }
        let ongoing = self.ongoing.lock().unwrap();
        if core.inputs.len()
            + ongoing
                .iter()
                .map(|x| x.inputs.len())
                .fold(0, usize::saturating_add)
            >= self.options.max_open_files
        {
            MAY_NOT_CHOOSE_TOO_MANY_FILES.click();
            return false;
        }
        for ongoing in ongoing.iter() {
            if CompactionCore::overlapping(ongoing, core) {
                MAY_NOT_CHOOSE_CONFLICT.click();
                return false;
            }
        }
        MAY_CHOOSE_COMPACTION.click();
        true
    }

    fn emit_compaction(
        &self,
        compaction_id: CompactionID,
        compaction: Compaction,
        score: i64,
    ) -> Option<Compaction> {
        clue! { LSM_TREE_LOG, TreeLogKey::ByCompactionID {
                compaction_id,
            } => TreeLogValue::CandidateCompaction {
                score,
                lower_level: compaction.core.lower_level,
                upper_level: compaction.core.upper_level,
                first_key: compaction.core.first_key.clone(),
                last_key: compaction.core.last_key.clone(),
                inputs: compaction.core.inputs.iter().map(|x| x.digest()).collect(),
            }
        };
        self.ongoing
            .lock()
            .unwrap()
            .push(Arc::clone(&compaction.core));
        Some(compaction)
    }
}

impl Clone for Version {
    fn clone(&self) -> Self {
        let options = self.options.clone();
        let levels = self.levels.clone();
        let ongoing = Arc::clone(&self.ongoing);
        Self {
            options,
            levels,
            ongoing,
        }
    }
}

///////////////////////////////////////////// SplitHint ////////////////////////////////////////////

enum SplitKey {
    First(usize),
    Last(usize),
}

struct SplitHint {
    tree: Arc<Version>,
    index: SplitKey,
}

impl SplitHint {
    fn new(tree: Arc<Version>) -> Self {
        let index = SplitKey::First(0);
        Self { tree, index }
    }

    fn hint_key(&self) -> &[u8] {
        match self.index {
            SplitKey::First(x) => {
                if x < self.ssts().len() {
                    return &self.ssts()[x].first_key;
                }
            }
            SplitKey::Last(x) => {
                if x < self.ssts().len() {
                    return &self.ssts()[x].last_key;
                }
            }
        }
        keyvalint::MAX_KEY
    }

    fn witness(&mut self, key: &[u8]) -> bool {
        let mut should_split = false;
        while self.index() < self.ssts().len()
            && compare_bytes(key, self.hint_key()) == Ordering::Greater
        {
            match self.index {
                SplitKey::First(x) => {
                    self.index = SplitKey::Last(x);
                    should_split = true;
                }
                SplitKey::Last(x) => {
                    self.index = SplitKey::First(x + 1);
                    should_split = true;
                }
            }
        }
        should_split
    }

    fn index(&self) -> usize {
        match self.index {
            SplitKey::First(x) => x,
            SplitKey::Last(x) => x,
        }
    }

    fn ssts(&self) -> &[Arc<SstMetadata>] {
        &self.tree.levels[self.tree.levels.len() - 1].ssts
    }
}

///////////////////////////////////////////// CachedSst ////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct CachedSst {
    ptr: Arc<Sst>,
}

impl LruValue for CachedSst {
    fn approximate_size(&self) -> usize {
        self.ptr.approximate_size()
    }
}

//////////////////////////////////////////// VersionRef ////////////////////////////////////////////

pub struct VersionRef<'a> {
    tree: &'a LsmTree,
    version: Arc<Version>,
}

impl<'a> VersionRef<'a> {
    pub fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        self.version.load(
            &self.tree.file_manager,
            &self.tree.sst_cache,
            key,
            timestamp,
            is_tombstone,
        )
    }

    pub fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<MergingCursor<Box<dyn Cursor<Error = sst::Error>>>, Error> {
        self.version
            .range_scan(&self.tree.file_manager, &self.tree.sst_cache, start_bound, end_bound, timestamp)
    }
}

impl<'a> Drop for VersionRef<'a> {
    fn drop(&mut self) {
        self.tree.explicit_unref(&self.version);
    }
}

////////////////////////////////////////////// LsmTree /////////////////////////////////////////////

pub struct LsmTree {
    root: PathBuf,
    options: LsmtkOptions,
    file_manager: Arc<FileManager>,
    mani: RwLock<Manifest>,
    version: Mutex<Arc<Version>>,
    compaction: Mutex<()>,
    stall: Condvar,
    compact: Condvar,
    references: ReferenceCounter<Setsum>,
    sst_cache: Arc<LeastRecentlyUsedCache<Setsum, CachedSst>>,
}

impl LsmTree {
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
        Self::from_manifest(options, mani)
    }

    pub(crate) fn from_manifest(options: LsmtkOptions, mani: Manifest) -> Result<Self, Error> {
        let root: PathBuf = PathBuf::from(&options.path);
        let mani = RwLock::new(mani);
        let file_manager = Arc::new(FileManager::new(options.max_open_files));
        let metadata = Self::list_ssts_from_manifest(&root, &mani.read().unwrap(), &file_manager)?;
        let version = Mutex::new(Arc::new(Version::open(options.clone(), metadata)?));
        let compaction = Mutex::new(());
        let version_setsum = version.lock().unwrap().compute_setsum().hexdigest();
        let mani_setsum = mani
            .read()
            .unwrap()
            .info('O')
            .map(|s| s.to_string())
            .unwrap_or(Setsum::default().hexdigest());
        if version_setsum != mani_setsum {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "setsum of tree does not match setsum of manifest".to_string(),
            })
            .as_z()
            .with_variable("tree", version_setsum)
            .with_variable("mani", mani_setsum);
        }
        let stall = Condvar::new();
        let compact = Condvar::new();
        let references = ReferenceCounter::default();
        let sst_cache = Arc::new(LeastRecentlyUsedCache::new(options.sst_cache_bytes));
        Self::explicit_ref(&references, &version.lock().unwrap());
        OPEN_DB.click();
        let mut db = Self {
            root,
            options,
            file_manager,
            mani,
            version,
            compaction,
            stall,
            compact,
            references,
            sst_cache,
        };
        db.cleanup_orphans()?;
        Ok(db)
    }

    fn list_ssts_from_manifest<P: AsRef<Path>>(
        root: P,
        mani: &Manifest,
        file_manager: &FileManager,
    ) -> Result<Vec<SstMetadata>, Error> {
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
        Ok(metadata)
    }

    fn cleanup_orphans(&mut self) -> Result<(), Error> {
        let manis = verifier::list_mani_fragments(&self.root)?;
        let mut ssts_to_remove = HashSet::new();
        for mani in manis.into_iter() {
            let mani_iter = ManifestIterator::open(mani)?;
            let mut first = true;
            for edit in mani_iter {
                let edit = edit?;
                if first {
                    first = false;
                    continue;
                }
                for rmed in edit.rmed() {
                    if let Some(setsum) = Setsum::from_hexdigest(rmed) {
                        ssts_to_remove.insert(setsum);
                    }
                }
                for added in edit.added() {
                    if let Some(setsum) = Setsum::from_hexdigest(added) {
                        ssts_to_remove.remove(&setsum);
                    }
                }
            }
        }
        for setsum in ssts_to_remove.into_iter() {
            let sst_path = SST_FILE(&self.root, setsum);
            let trash_path = TRASH_SST(&self.root, setsum);
            if sst_path.exists() && !trash_path.exists() {
                // The verifier will pick up on there being orphans, so we can ignore error here.
                // TODO(rescrv):  Make the verifier detect this case.
                let _ = rename(sst_path, trash_path);
            }
        }
        Ok(())
    }

    pub fn ingest<P: AsRef<Path>>(&self, sst_path: P, log_num: Option<u64>) -> Result<(), Error> {
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
        if let Some(log_num) = log_num {
            edit.info('L', &format!("{}", log_num))?;
        }
        self.apply_manifest_ingest(acc, edit, metadata)?;
        Ok(())
    }

    pub fn compaction_thread(&self) -> Result<(), Error> {
        loop {
            let compaction = {
                let mut mutex = self.compaction.lock().unwrap();
                'inner: loop {
                    let version = self.take_snapshot();
                    let compaction = version.version.next_compaction();
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
                let version = self.take_snapshot();
                let _ = version.version.release_compaction(compaction);
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
        let version = self.take_snapshot();
        let mut split_hint = SplitHint::new(version.version.clone());
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
            mani_edit.rm(&input.hexdigest())?;
            let sst = self.open_sst(input)?;
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
        let mut mutex = self.compaction.lock().unwrap();
        let mut version = self.take_snapshot();
        while version.version.should_stall_ingest() {
            INGEST_STALL.click();
            mutex = self.stall.wait(mutex).unwrap();
            let mut version2 = self.take_snapshot();
            std::mem::swap(&mut version, &mut version2);
            drop(version2);
        }
        let tree_setsum = version.version.compute_setsum();
        // NOTE(rescrv):  We subtract because we are removing the discard setsum.
        // This has the happy effect of subtracting the inverse of what we added.
        let output_setsum = tree_setsum - setsum;
        // TODO(rescrv): poison here.
        mani_edit.info('I', &tree_setsum.hexdigest())?;
        mani_edit.info('O', &output_setsum.hexdigest())?;
        mani_edit.info('D', &setsum.hexdigest())?;
        // TODO(rescrv):  Do not hold tree lock across manifest edit.
        self.mani.write().unwrap().apply(mani_edit)?;
        let new_version = Arc::new(version.version.ingest(new)?);
        // TODO(rescrv): don't hold the lock for computing setsum.
        let tree_setsum = new_version.compute_setsum();
        assert_eq!(tree_setsum, output_setsum);
        self.install_version(new_version);
        self.compact.notify_all();
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
        let version = self.take_snapshot();
        let tree_setsum = version.version.compute_setsum();
        let output_setsum = tree_setsum - discard_setsum;
        // TODO(rescrv): poison here.
        mani_edit.info('I', &tree_setsum.hexdigest())?;
        mani_edit.info('O', &output_setsum.hexdigest())?;
        mani_edit.info('D', &discard_setsum.hexdigest())?;
        self.mani.write().unwrap().apply(mani_edit)?;
        let new_version = Arc::new(version.version.apply_compaction(compaction, outputs)?);
        // TODO(rescrv): don't hold the lock for computing setsum.
        let tree_setsum = new_version.compute_setsum();
        assert_eq!(tree_setsum, output_setsum);
        self.install_version(new_version);
        self.stall.notify_all();
        Ok(())
    }

    fn apply_moving_compaction(&self, compaction: Compaction, output: Setsum) -> Result<(), Error> {
        let _mutex = self.compaction.lock().unwrap();
        let sst = self.open_sst(output)?;
        let meta = sst.metadata()?;
        let version = self.take_snapshot();
        let tree_setsum1 = version.version.compute_setsum();
        let new_version = Arc::new(version.version.apply_compaction(compaction, vec![meta])?);
        let tree_setsum2 = new_version.compute_setsum();
        assert_eq!(tree_setsum1, tree_setsum2);
        self.install_version(new_version);
        self.stall.notify_all();
        Ok(())
    }

    // TODO(rescrv): Dedupe with tree.
    fn open_sst(&self, setsum: Setsum) -> Result<Arc<Sst>, Error> {
        if let Some(sst) = self.sst_cache.lookup(setsum) {
            Ok(sst.ptr)
        } else {
            let sst_path = SST_FILE(&self.root, setsum);
            let file = self.file_manager.open(sst_path)?;
            let sst = Arc::new(Sst::from_file_handle(file)?);
            self.sst_cache.insert(
                setsum,
                CachedSst {
                    ptr: Arc::clone(&sst),
                },
            );
            Ok(sst)
        }
    }

    pub fn take_snapshot(&self) -> VersionRef {
        let version = Arc::clone(&*self.version.lock().unwrap());
        VersionRef {
            tree: self,
            version,
        }
    }

    fn install_version(&self, mut version2: Arc<Version>) {
        Self::explicit_ref(&self.references, &version2);
        let mut version1 = self.version.lock().unwrap();
        std::mem::swap(&mut *version1, &mut version2);
        self.explicit_unref(&version2);
    }

    fn explicit_ref(references: &ReferenceCounter<Setsum>, version: &Version) {
        for setsum in version.setsums() {
            references.inc(setsum);
        }
    }

    fn explicit_unref(&self, version: &Arc<Version>) {
        if Arc::strong_count(version) != 1 {
            return;
        }
        for setsum in version.setsums() {
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

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sst::SstMetadata;

    use super::Level;

    fn key_range(first_key: Vec<u8>, last_key: Vec<u8>) -> Arc<SstMetadata> {
        Arc::new(SstMetadata {
            setsum: [0u8; 32],
            first_key,
            last_key,
            smallest_timestamp: 0,
            biggest_timestamp: 0,
            file_size: 0,
        })
    }

    #[test]
    fn bounds() {
        let level = Level {
            ssts: vec![
                key_range(vec![b'A'], vec![b'B']),
                key_range(vec![b'C'], vec![b'D']),
                key_range(vec![b'E'], vec![b'F']),
                key_range(vec![b'F'], vec![b'F']),
                key_range(vec![b'F'], vec![b'G']),
                key_range(vec![b'H'], vec![b'I']),
                key_range(vec![b'J'], vec![b'K']),
            ],
        };

        assert_eq!(0, level.lower_bound(&[]));
        assert_eq!(0, level.upper_bound(&[]));

        assert_eq!(0, level.lower_bound(&[b'A']));
        assert_eq!(1, level.upper_bound(&[b'A']));

        assert_eq!(2, level.lower_bound(&[b'F']));
        assert_eq!(5, level.upper_bound(&[b'F']));

        assert_eq!(6, level.upper_bound(&[b'H', b'A']));

        assert_eq!(7, level.upper_bound(&[b'Z']));
    }
}
