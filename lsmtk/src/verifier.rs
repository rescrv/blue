/// Run a verifier against an lsmtk LsmTree and remove the files that are deemed garbage due to
/// compaction and garbage collection.
///
/// Verification takes the following steps when there's no crashing:
/// 1.  Read the lowest numbered manifest that's not also the highest numbered manifest.
///     Break if no such manifest.
/// 2.  Collect the list of ssts and logs to be removed.  Wait until all are present.
/// 3.  Check setsums and possibly verify the gc.
/// 4.  Log the basename of every file to remove to the verifier manifest.
/// 5.  Unlink the manifest fragment.
/// 6.  Unlink the files logged in 4.
/// 7.  Log to remove every file listed in 4's edit.
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::{read_dir, remove_file};
use std::path::{Path, PathBuf};

use biometrics::{Collector, Counter};
use keyvalint::Cursor;
use mani::{Edit, Manifest, ManifestIterator};
use setsum::Setsum;
use sst::merging_cursor::MergingCursor;
use sst::{Sst, SstCursor};
use zerror::Z;
use zerror_core::ErrorCore;

use super::{
    parse_log_file, Error, IoToZ, LsmtkOptions, MANI_ROOT, SST_FILE, TRASH_ROOT, TRASH_SST,
    VERIFY_ROOT,
};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static RM_FILE: Counter = Counter::new("lsmtk.verifier.verifier_rm_file");
static RM_MANI: Counter = Counter::new("lsmtk.verifier.verifier_rm_mani");
static EDIT_VERIFIED: Counter = Counter::new("lsmtk.verifier.verifier_edit_verified");
static MANI_VERIFIED: Counter = Counter::new("lsmtk.verifier.verifier_mani_verified");

pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&RM_FILE);
    collector.register_counter(&RM_MANI);
    collector.register_counter(&EDIT_VERIFIED);
    collector.register_counter(&MANI_VERIFIED);
}

//////////////////////////////////////////// LsmVerifier ///////////////////////////////////////////

pub struct LsmVerifier {
    root: PathBuf,
    mani: Manifest,
    options: LsmtkOptions,
}

impl LsmVerifier {
    pub fn open(options: LsmtkOptions) -> Result<Self, Error> {
        let root: PathBuf = PathBuf::from(&options.path);
        let mani: Manifest = Manifest::open(options.mani.clone(), VERIFY_ROOT(&root))?;
        Ok(Self {
            root,
            mani,
            options,
        })
    }

    pub fn verify(&mut self) -> Result<(), Error> {
        let mut entries = list_mani_fragments(&self.root)?;
        // Drop the last #'d entry and the main file.
        // We need to keep it around so that a crash/restart
        // of mani will pick a strictly higher log number.
        //
        // We pop twice.  It's guaranteed by list_mani_fragments function
        // to put these at the end.  If we pop too much, that's OK.
        entries.pop();
        entries.pop();
        for entry in entries {
            // 1. We're going to always process the lowest numbered log.
            self.process_one(&entry)?;
        }
        Ok(())
    }

    fn process_one(&mut self, entry: &PathBuf) -> Result<(), Error> {
        // This will conditionally perform steps 6 and 7 if there's an unprocessed edit.
        self.possibly_complete_processing(entry)?;
        if let Some(last_entry_processed) = self.mani.info('M') {
            let log_num_old = mani::extract_backup(last_entry_processed);
            let log_num_new = mani::extract_backup(entry);
            if log_num_old == log_num_new {
                return Ok(());
            }
        }
        // At this point we know entry is the lowest numbered log that we can process.
        // Proceed to step 2.
        // SAFETY(rescrv): cleanup_log should assert this.
        assert!(self.mani.strs().count() == 0);
        // 2.  Collect the list of ssts and logs to be removed.  Wait until all are present.
        let verifier_setsum = setsum_from_info_default('O', self.mani.info('O'))?;
        let (output_setsum, ssts_to_rm, logs_to_rm) = self.verify_one(entry, verifier_setsum)?;
        let mut edit = Edit::default();
        for sst in ssts_to_rm.iter() {
            let path = TRASH_SST(&self.root, *sst);
            if !path.exists() {
                return Err(Error::Backoff {
                    core: ErrorCore::default(),
                    setsum: sst.hexdigest(),
                });
            }
            edit.add(&basename_string(&path)?)?;
        }
        let logs_by_setsum = self.get_logs_from_trash(logs_to_rm.len())?;
        for setsum in logs_to_rm.iter() {
            if let Some(path) = logs_by_setsum.get(setsum) {
                edit.add(&basename_string(path)?)?;
            } else {
                return Err(Error::Backoff {
                    core: ErrorCore::default(),
                    setsum: setsum.hexdigest(),
                });
            }
        }
        edit.info('O', &output_setsum.hexdigest())?;
        edit.info('M', &basename_string(entry)?)?;
        self.mani.apply(edit)?;
        self.possibly_complete_processing(entry)?;
        Ok(())
    }

    fn possibly_complete_processing(&mut self, entry: &PathBuf) -> Result<(), Error> {
        if let Some(last_entry_processed) = self.mani.info('M') {
            let log_num_old = mani::extract_backup(last_entry_processed);
            let log_num_new = mani::extract_backup(entry);
            if log_num_old > log_num_new {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "clean up saw log out of order".to_string(),
                }
                .with_variable("old log num", log_num_old)
                .with_variable("new log num", log_num_new));
            }
            if log_num_old == log_num_new && entry.exists() {
                RM_MANI.click();
                remove_file(entry).as_z().with_variable("path", entry)?;
            }
            let mut edit = Edit::default();
            for path in self.mani.strs() {
                RM_FILE.click();
                let full_path = TRASH_ROOT(&self.root).join(path);
                if full_path.exists() {
                    remove_file(&full_path)
                        .as_z()
                        .with_variable("path", full_path)?;
                }
                edit.rm(path)?;
            }
            self.mani.apply(edit)?;
        }
        Ok(())
    }

    fn verify_one(
        &self,
        entry: &PathBuf,
        mut acc: Setsum,
    ) -> Result<(Setsum, Vec<Setsum>, Vec<Setsum>), Error> {
        let mani_iter = ManifestIterator::open(entry)?;
        let mut ssts_to_remove = vec![];
        let mut logs_to_remove = vec![];
        let mut last_outputs = None;
        let mut first = true;
        for edit in mani_iter {
            let edit = edit?;
            let inputs = setsum_from_info('I', edit.get_info('I'))?;
            let outputs = setsum_from_info('O', edit.get_info('O'))?;
            let discard = setsum_from_info('D', edit.get_info('D'))?;
            // The first entry is known to not balance as it carries over the inputs and discard
            // from the last transaction of the previous fragment.  The output should match the acc
            // in this case.
            //
            // Do a little dance with whether we compare against outputs or inputs.
            if first && outputs != acc {
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "manifest does not continue with accumulated setsum".to_string(),
                }
                .with_variable("outputs", outputs.hexdigest())
                .with_variable("acc", acc.hexdigest())
                .with_variable("fragment", entry.to_string_lossy());
                return Err(err);
            }
            if !first && inputs != acc {
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "manifest does not continue with accumulated setsum".to_string(),
                }
                .with_variable("inputs", inputs.hexdigest())
                .with_variable("acc", acc.hexdigest())
                .with_variable("fragment", entry.to_string_lossy());
                return Err(err);
            }
            if !first && inputs != outputs + discard {
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "manifest does not balance inputs == outputs + discard".to_string(),
                }
                .with_variable("inputs", inputs.hexdigest())
                .with_variable("outputs", outputs.hexdigest())
                .with_variable("discard", discard.hexdigest())
                .with_variable("discard^-1", (Setsum::default() - discard).hexdigest())
                .with_variable("inputs - outputs", (inputs - outputs).hexdigest());
                return Err(err);
            }
            last_outputs = Some(outputs);
            let mut computed_discard = Setsum::default();
            for added in edit.added() {
                let setsum = Setsum::from_hexdigest(added).ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("manifest added has bad digest: {added}"),
                })?;
                computed_discard -= setsum;
            }
            for rmed in edit.rmed() {
                let setsum = Setsum::from_hexdigest(rmed).ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("manifest rmed has bad digest: {rmed}"),
                })?;
                computed_discard += setsum;
                ssts_to_remove.push(setsum);
            }
            if !first {
                if edit.added().count() == 1 && edit.rmed().count() == 0 {
                    // SAFETY(rescrv):  We are adding data, so compute the inverse of what we discard.
                    logs_to_remove.push(Setsum::default() - computed_discard);
                }
                if discard != computed_discard {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        context: format!(
                            "manifest has bad discard: expected {discard:?}, but got {computed_discard:?}"
                        ),
                    });
                }
                if discard != Setsum::default() && edit.rmed().count() > 0 {
                    self.verify_gc(&edit, discard)?;
                }
                acc -= computed_discard;
            }
            first = false;
            EDIT_VERIFIED.click();
        }
        MANI_VERIFIED.click();
        if last_outputs != Some(acc) {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: format!("manifest has bad output setsum: expected {acc:?}"),
            });
        }
        Ok((acc, ssts_to_remove, logs_to_remove))
    }

    fn verify_gc(&self, edit: &Edit, discard: Setsum) -> Result<(), Error> {
        fn from_hexdigest(hex_digest: &str) -> Result<Setsum, Error> {
            match Setsum::from_hexdigest(hex_digest) {
                Some(setsum) => Ok(setsum),
                None => Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("manifest field has bad digest: {hex_digest}"),
                }),
            }
        }
        let mut input_cursors: Vec<SstCursor> = vec![];
        let mut gc_cursors: Vec<SstCursor> = vec![];
        for rm in edit.rmed() {
            let cursor = self.get_cursor(from_hexdigest(rm)?)?;
            input_cursors.push(cursor.clone());
            gc_cursors.push(cursor);
        }
        let mut output_cursors: Vec<SstCursor> = vec![];
        for add in edit.added() {
            output_cursors.push(self.get_cursor(from_hexdigest(add)?)?);
        }
        let mut input = MergingCursor::new(input_cursors)?;
        let mut output = MergingCursor::new(output_cursors)?;
        let mut gc = MergingCursor::new(gc_cursors)?;
        input.seek_to_first()?;
        input.next()?;
        output.seek_to_first()?;
        output.next()?;
        gc.seek_to_first()?;
        gc.next()?;
        let mut gc = self.options.gc_policy.collector(gc, 0)?;
        let mut gc_next = gc.next()?;
        let mut computed_discard = Setsum::default();
        while let (Some(i), Some(o)) = (input.key(), output.key()) {
            let mut must_return = false;
            if let Some(gc_next) = gc_next {
                match gc_next.cmp(&i) {
                    Ordering::Less => {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            context: "gc key less than input".to_string(),
                        })
                        .with_variable("gc", gc_next)
                        .with_variable("input", i);
                    }
                    Ordering::Equal => {
                        must_return = true;
                    }
                    Ordering::Greater => {}
                };
            }
            match i.cmp(&o) {
                Ordering::Less => {
                    if must_return {
                        return Err(Error::Corruption {
                            core: ErrorCore::default(),
                            context: "data loss".to_string(),
                        })
                        .with_variable("input", i);
                    }
                    let mut setsum = sst::Setsum::default();
                    setsum.insert(input.key_value().unwrap());
                    computed_discard += setsum.into_inner();
                    input.next()?;
                }
                Ordering::Greater => {
                    // NOTE(rescrv):  This should never happen.
                    //
                    // It means a key was manufactured out of thin err,
                    // or maybe from a previous compaction.
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        context: "data construction".to_string(),
                    })
                    .with_variable("output", o);
                }
                Ordering::Equal => {
                    input.next()?;
                    output.next()?;
                }
            };
            if must_return {
                gc_next = gc.next()?;
            }
        }
        if let Some(o) = output.key() {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "data construction".to_string(),
            })
            .with_variable("output", o);
        }
        while let Some(i) = input.key_value() {
            let mut setsum = sst::Setsum::default();
            setsum.insert(i);
            computed_discard += setsum.into_inner();
            input.next()?;
        }
        if computed_discard != discard {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "garbage collection has bad discard".to_string(),
            })
            .with_variable("discard", discard.hexdigest())
            .with_variable("discard^-1", (Setsum::default() - discard).hexdigest())
            .with_variable("computed_discard", computed_discard.hexdigest())
            .with_variable(
                "computed_discard^-1",
                (Setsum::default() - computed_discard).hexdigest(),
            );
        }
        Ok(())
    }

    fn get_cursor(&self, setsum: Setsum) -> Result<sst::SstCursor, Error> {
        let trash_path = TRASH_SST(&self.root, setsum);
        let sst_path = SST_FILE(&self.root, setsum);
        let file = match sst::file_manager::open_without_manager(&trash_path) {
            Ok(file) => file,
            Err(_) => match sst::file_manager::open_without_manager(sst_path) {
                Ok(file) => file,
                Err(_) => sst::file_manager::open_without_manager(&trash_path)?,
            },
        };
        let sst = Sst::from_file_handle(file)?;
        Ok(sst.cursor())
    }

    fn get_logs_from_trash(&mut self, count: usize) -> Result<HashMap<Setsum, String>, Error> {
        let trash_root = TRASH_ROOT(&self.root);
        let mut entries = Vec::with_capacity(count);
        for entry in read_dir(trash_root)? {
            let entry = entry?;
            if let Some(number) = parse_log_file(entry.path()) {
                entries.push((number, entry.path()));
            }
        }
        entries.sort();
        let mut map = HashMap::new();
        let entries = if entries.len() >= count {
            &entries[..count]
        } else {
            &[]
        };
        for (number, path) in entries {
            let setsum = sst::log::log_to_setsum(self.options.log.clone(), path)?;
            map.insert(setsum.into_inner(), format!("log.{number}"));
        }
        Ok(map)
    }
}

///////////////////////////////////////// ManifestVerifier /////////////////////////////////////////

pub struct ManifestVerifier {}

impl ManifestVerifier {
    pub fn open() -> Result<Self, Error> {
        Ok(ManifestVerifier {})
    }

    pub fn verify(&self, entry: &PathBuf) -> Result<Vec<(Setsum, Setsum, Setsum)>, Error> {
        let mani_iter = ManifestIterator::open(entry)?;
        let mut first = true;
        let mut acc = Setsum::default();
        let mut ret = vec![];
        for edit in mani_iter {
            let edit = edit?;
            let inputs = setsum_from_info('I', edit.get_info('I'))?;
            let outputs = setsum_from_info('O', edit.get_info('O'))?;
            let discard = setsum_from_info('D', edit.get_info('D'))?;
            if first {
                acc = outputs;
            } else {
                ret.push((inputs, outputs, discard));
                if inputs != acc {
                    let err = Error::Corruption {
                        core: ErrorCore::default(),
                        context: "manifest does not continue with accumulated setsum".to_string(),
                    }
                    .with_variable("inputs", inputs.hexdigest())
                    .with_variable("acc", acc.hexdigest())
                    .with_variable("fragment", entry.to_string_lossy());
                    return Err(err);
                }
                if inputs != outputs + discard {
                    let err = Error::Corruption {
                        core: ErrorCore::default(),
                        context: "manifest does not balance inputs == outputs + discard"
                            .to_string(),
                    }
                    .with_variable("inputs", inputs.hexdigest())
                    .with_variable("outputs", outputs.hexdigest())
                    .with_variable("discard", discard.hexdigest())
                    .with_variable("discard^-1", (Setsum::default() - discard).hexdigest())
                    .with_variable("inputs - outputs", (inputs - outputs).hexdigest());
                    return Err(err);
                }
            }
            let mut computed_discard = Setsum::default();
            for added in edit.added() {
                let setsum = Setsum::from_hexdigest(added).ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("manifest added has bad digest: {added}"),
                })?;
                computed_discard -= setsum;
            }
            for rmed in edit.rmed() {
                let setsum = Setsum::from_hexdigest(rmed).ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("manifest rmed has bad digest: {rmed}"),
                })?;
                computed_discard += setsum;
            }
            if !first {
                if discard != computed_discard {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        context: format!(
                            "manifest has bad discard: expected {discard:?}, but got {computed_discard:?}"
                        ),
                    })
                    .with_variable("discard", discard.hexdigest())
                    .with_variable("discard^-1", (Setsum::default() - discard).hexdigest())
                    .with_variable("computed_discard", computed_discard.hexdigest())
                    .with_variable("computed_discard^-1", (Setsum::default() - computed_discard).hexdigest());
                }
                acc -= computed_discard;
            }
            first = false;
        }
        Ok(ret)
    }
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn basename_string<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    if let Some(file_name) = path.as_ref().file_name() {
        let file_name_string = file_name.to_string_lossy().to_string();
        if PathBuf::from(&file_name_string) != file_name {
            Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "file name contains lossy characters".to_string(),
            })
            .with_variable("path", path.as_ref().to_string_lossy())
        } else {
            Ok(file_name_string)
        }
    } else {
        Err(Error::Corruption {
            core: ErrorCore::default(),
            context: "file name has no basename".to_string(),
        })
        .with_variable("path", path.as_ref().to_string_lossy())
    }
}

fn setsum_from_info(info: char, value: Option<&String>) -> Result<Setsum, Error> {
    let hex_digest = match value {
        Some(hex_digest) => hex_digest,
        None => {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: format!("manifest edit missing '{info}'"),
            });
        }
    };
    match Setsum::from_hexdigest(hex_digest) {
        Some(setsum) => Ok(setsum),
        None => Err(Error::Corruption {
            core: ErrorCore::default(),
            context: format!("manifest '{info}' field has bad digest: {hex_digest}"),
        }),
    }
}

fn setsum_from_info_default(info: char, value: Option<&String>) -> Result<Setsum, Error> {
    let hex_digest = match value {
        Some(hex_digest) => hex_digest,
        None => {
            return Ok(Setsum::default());
        }
    };
    match Setsum::from_hexdigest(hex_digest) {
        Some(setsum) => Ok(setsum),
        None => Err(Error::Corruption {
            core: ErrorCore::default(),
            context: format!("manifest '{info}' field has bad digest: {hex_digest}"),
        }),
    }
}

////////////////////////////////////////// public helpers //////////////////////////////////////////

pub fn list_mani_fragments<P: AsRef<Path>>(root: P) -> Result<Vec<PathBuf>, Error> {
    let mut entries = vec![];
    let mani_root = MANI_ROOT(root.as_ref());
    for entry in read_dir(&mani_root)? {
        let entry = entry?;
        entries.push(entry.path());
    }
    let mut entries = entries
        .iter()
        .filter_map(mani::extract_backup)
        .collect::<Vec<_>>();
    entries.sort();
    let mut entries = entries
        .into_iter()
        .map(|x| mani::BACKUP(&mani_root, x))
        .collect::<Vec<_>>();
    entries.push(mani::MANIFEST(&mani_root));
    Ok(entries)
}
