use std::fs::{hard_link, read_dir, remove_file, rename, File};
use std::ops::Bound;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use keyvalint::{Cursor, KeyValuePair, KeyValueRef};
use mani::{Edit, Manifest};
use setsum::Setsum;
use sst::bounds_cursor::BoundsCursor;
use sst::log::{ConcurrentLogBuilder, LogOptions};
use sst::merging_cursor::MergingCursor;
use sst::pruning_cursor::PruningCursor;
use sst::{check_key_len, check_value_len, Builder, SstBuilder};
use sync42::wait_list::WaitList;
use zerror::Z;
use zerror_core::ErrorCore;

mod memtable;

use crate::{
    ensure_dir, make_all_dirs, parse_log_file, Error, LsmTree, LsmtkOptions, LOG_FILE, MANI_ROOT,
    SST_FILE, TEMP_FILE, TEMP_ROOT, TRASH_ROOT,
};
use memtable::MemTable;

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

    fn _put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: Some(value.into()),
        });
    }

    fn _del(&mut self, key: &[u8], timestamp: u64) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp,
            value: None,
        });
    }
}

impl keyvalint::WriteBatch for WriteBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp: 0,
            value: Some(value.into()),
        });
    }

    fn del(&mut self, key: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp: 0,
            value: None,
        });
    }
}

impl<'a> keyvalint::WriteBatch for &'a mut WriteBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp: 0,
            value: Some(value.into()),
        });
    }

    fn del(&mut self, key: &[u8]) {
        self.entries.push(KeyValuePair {
            key: key.into(),
            timestamp: 0,
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
        seq_no = std::cmp::max(seq_no, tree.max_timestamp());
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
                .with_info("got", got_setsum.hexdigest())
                .with_info("imm", imm_setsum.hexdigest());
                return Err(err);
            }
            self.tree.ingest(&sst_path, Some(imm_trigger))?;
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

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let mut wb = WriteBatch::with_capacity(1);
        check_key_len(key)?;
        check_value_len(value)?;
        let key = key.to_vec();
        let timestamp = 0;
        let value = Some(value.to_vec());
        wb.entries.push(KeyValuePair {
            key,
            timestamp,
            value,
        });
        self.write(wb)
    }

    fn del(&self, key: &[u8]) -> Result<(), Error> {
        let mut wb = WriteBatch::with_capacity(1);
        check_key_len(key)?;
        let key = key.to_vec();
        let timestamp = 0;
        let value = None;
        wb.entries.push(KeyValuePair {
            key,
            timestamp,
            value,
        });
        self.write(wb)
    }

    fn write(&self, mut batch: Self::WriteBatch<'_>) -> Result<(), Error> {
        let (mut wait_guard, memtable, log) = {
            let mut state = self.state.lock().unwrap();
            let wait_guard = self.wait_list.link(());
            let seq_no = state.seq_no + 1;
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

    fn load(&self, key: &[u8], is_tombstone: &mut bool) -> Result<Option<Vec<u8>>, Self::Error> {
        let (mem, imm, version, timestamp) = {
            let state = self.state.lock().unwrap();
            let mem = Arc::clone(&state.mem);
            let imm = state.imm.as_ref().map(Arc::clone);
            let version = self.tree.take_snapshot();
            (mem, imm, version, state.seq_no)
        };
        *is_tombstone = false;
        let ret = mem.load(key, timestamp, is_tombstone)?;
        if ret.is_some() || *is_tombstone {
            return Ok(ret);
        }
        if let Some(imm) = imm {
            let ret = imm.load(key, timestamp, is_tombstone)?;
            if ret.is_some() || *is_tombstone {
                return Ok(ret);
            }
        }
        let ret = version.load(key, timestamp, is_tombstone)?;
        Ok(ret)
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        let (mem, imm, version, timestamp) = {
            let state = self.state.lock().unwrap();
            let mem = Arc::clone(&state.mem);
            let imm = state.imm.as_ref().map(Arc::clone);
            let version = self.tree.take_snapshot();
            (mem, imm, version, state.seq_no)
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
        let version_scan = version.range_scan(start_bound, end_bound, timestamp)?;
        cursors.push(Box::new(version_scan));
        let cursor = MergingCursor::new(cursors)?;
        let cursor = PruningCursor::new(cursor, timestamp)?;
        let cursor = BoundsCursor::new(cursor, start_bound, end_bound)?;
        Ok(cursor)
    }
}
