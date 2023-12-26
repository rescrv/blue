use std::cmp::Ordering;
use std::ops::Bound;
use std::sync::{Arc, Mutex};

use biometrics::{Collector, Counter};
use indicio::clue;
use keyvalint::{Cursor, KeyValueLoad};
use one_two_eight::{generate_id, generate_id_prototk};
use setsum::Setsum;
use sst::file_manager::FileManager;
use sst::lazy_cursor::LazyCursor;
use sst::merging_cursor::MergingCursor;
use sst::pruning_cursor::PruningCursor;
use sst::{Sst, SstMetadata};
use zerror_core::ErrorCore;

use super::{compare_bytes, Error, LsmtkOptions, TreeLogKey, TreeLogValue, LSM_TREE_LOG, SST_FILE};

mod recover;

use recover::recover;

///////////////////////////////////////////// constants ////////////////////////////////////////////

pub const NUM_LEVELS: usize = 16;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

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

pub fn register_biometrics(collector: &Collector) {
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

/////////////////////////////////////////////// Tree ///////////////////////////////////////////////

pub struct Tree {
    options: LsmtkOptions,
    levels: Vec<Level>,
    ongoing: Arc<Mutex<Vec<Arc<CompactionCore>>>>,
}

// TODO(rescrv): make compare_bytes this signature.
fn compare_for_min_max(lhs: &&[u8], rhs: &&[u8]) -> Ordering {
    compare_bytes(lhs, rhs)
}

impl Tree {
    pub fn open(options: LsmtkOptions, metadata: Vec<SstMetadata>) -> Result<Self, Error> {
        recover(options, metadata)
    }

    pub fn should_stall_ingest(&self) -> bool {
        self.levels[0].ssts.len() >= self.options.l0_write_stall_threshold_files
            || self.levels[0].size() >= self.options.l0_write_stall_threshold_bytes as u64
    }

    pub fn should_perform_mandatory_compaction(&self) -> bool {
        self.levels[0].ssts.len() >= self.options.l0_mandatory_compaction_threshold_files
            || self.levels[0].size() >= self.options.l0_mandatory_compaction_threshold_bytes as u64
            || self.levels.iter().all(|x| !x.ssts.is_empty())
    }

    pub fn setsums(&self) -> Vec<Setsum> {
        let mut setsums = vec![];
        for level in self.levels.iter() {
            for md in level.ssts.iter() {
                setsums.push(Setsum::from_digest(md.setsum));
            }
        }
        setsums
    }

    pub fn ingest(&self, to_add: SstMetadata) -> Result<Self, Error> {
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

    pub fn compute_setsum(&self) -> Setsum {
        let mut acc = Setsum::default();
        for level in self.levels.iter() {
            for file in level.ssts.iter() {
                acc += Setsum::from_digest(file.setsum);
            }
        }
        acc
    }

    pub fn load(
        &self,
        fm: &FileManager,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        *is_tombstone = false;
        let mut level0 = self.levels[0].ssts.clone();
        level0.sort_by_key(|md| md.biggest_timestamp);
        for l0 in level0.into_iter().rev() {
            let ret = self.load_from_sst(fm, &l0, key, timestamp, is_tombstone)?;
            if ret.is_some() || *is_tombstone {
                return Ok(ret);
            }
        }
        for level in self.levels[1..].iter() {
            let lower_bound = level.lower_bound(key);
            let upper_bound = level.upper_bound(key);
            for sst in level.ssts[lower_bound..upper_bound].iter() {
                let ret = self.load_from_sst(fm, sst, key, timestamp, is_tombstone)?;
                if ret.is_some() || *is_tombstone {
                    return Ok(ret);
                }
            }
        }
        Ok(None)
    }

    pub fn load_from_sst<'a: 'b, 'b>(
        &self,
        fm: &FileManager,
        md: &SstMetadata,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        let sst_path = SST_FILE(&self.options.path, Setsum::from_digest(md.setsum));
        let handle = fm.open(sst_path)?;
        let sst = Sst::from_file_handle(handle)?;
        Ok(sst.load(key, timestamp, is_tombstone)?)
    }

    pub fn range_scan<T: AsRef<[u8]>>(
        &self,
        fm: &Arc<FileManager>,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<MergingCursor<Box<dyn Cursor<Error = sst::Error>>>, Error> {
        let mut cursors: Vec<Box<dyn Cursor<Error = sst::Error>>> = vec![];
        for sst in self.levels[0].ssts.iter() {
            let sst_path = SST_FILE(&self.options.path, Setsum::from_digest(sst.setsum));
            cursors.push(Box::new(PruningCursor::new(
                LazyCursor::new(Arc::clone(fm), sst_path),
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
            // TODO(rescrv): Make it so that these cursors will not be on the heap.
            // It's not as bad as it seems.  We have a linear number of cursors on the heap, but
            // we'll seek to the end of one before pulling the next one up.
            //
            // Sequence cursor needs some clarity first.
            for sst in level.ssts.iter() {
                let sb = Bound::Included(&sst.first_key);
                let eb = Bound::Included(&sst.last_key);
                // TODO(rescrv): Use lower_bound and upper_bound functions to speed this up.
                if compare_bounds_le(start_bound, eb) && compare_bounds_le(sb, end_bound) {
                    let sst_path = SST_FILE(&self.options.path, Setsum::from_digest(sst.setsum));
                    cursors.push(Box::new(PruningCursor::new(
                        LazyCursor::new(Arc::clone(fm), sst_path),
                        timestamp,
                    )?));
                }
            }
        }
        Ok(MergingCursor::new(cursors)?)
    }

    pub fn next_compaction(&self) -> Option<Compaction> {
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

    pub fn release_compaction(&self, compaction: Compaction) -> Result<(), Error> {
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

    pub fn apply_compaction(
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

impl Clone for Tree {
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

pub struct SplitHint {
    tree: Arc<Tree>,
    index: SplitKey,
}

impl SplitHint {
    pub fn new(tree: Arc<Tree>) -> Self {
        let index = SplitKey::First(0);
        Self { tree, index }
    }

    pub fn hint_key(&self) -> &[u8] {
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

    pub fn witness(&mut self, key: &[u8]) -> bool {
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