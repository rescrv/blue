use std::collections::hash_set::HashSet;
use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use util::lockfile::Lockfile;

use buffertk::{stack_pack, Unpacker};

use prototk::field_types::*;

use biometrics::Counter;

use hey_listen::{HeyListen, Stationary};

use clue::Trace;

use zerror::{FromIOError, ZError, ZErrorResult};

use super::file_manager::FileManager;
use super::merging_cursor::MergingCursor;
use super::pruning_cursor::PruningCursor;
use super::sst::{SSTBuilder, SSTBuilderOptions, SSTMetadata, SST};
use super::{compare_bytes, Builder, Cursor, Error};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_NOT_OBTAINED: Counter = Counter::new("lp.lsm.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lp.lsm.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

//////////////////////////////////////////// LSMOptions ////////////////////////////////////////////

pub struct LSMOptions {
    max_open_files: usize,
    meta_options: SSTBuilderOptions,
    wait_for_lock: bool,
}

impl Default for LSMOptions {
    fn default() -> Self {
        Self {
            max_open_files: 1 << 20,
            meta_options: SSTBuilderOptions::default(),
            wait_for_lock: true,
        }
    }
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

#[derive(Default)]
struct State {
    metadata_files: Vec<PathBuf>,
    sst_metadata: Vec<SSTMetadata>,
}

////////////////////////////////////////////// LSMTree /////////////////////////////////////////////

pub struct LSMTree {
    root: PathBuf,
    options: LSMOptions,
    file_manager: FileManager,
    state: Mutex<State>,
    _lockfile: Lockfile,
}

impl LSMTree {
    pub fn open<P: AsRef<Path>>(options: LSMOptions, path: P) -> Result<LSMTree, ZError<Error>> {
        let path: PathBuf = path
            .as_ref()
            .canonicalize()
            .from_io()
            .with_context::<stringref>("path", 1, &path.as_ref().to_string_lossy())?;
        // Deal with the lockfile first.
        let lockfile_path = path.join("LOCKFILE");
        let lockfile = if options.wait_for_lock {
            Lockfile::wait(lockfile_path.clone())
                .from_io()
                .with_context::<stringref>("path", 1, &lockfile_path.to_string_lossy())
                .with_backtrace()?
        } else {
            Lockfile::lock(lockfile_path.clone())
                .from_io()
                .with_context::<stringref>("path", 1, &lockfile_path.to_string_lossy())
                .with_backtrace()?
        };
        if lockfile.is_none() {
            LOCK_NOT_OBTAINED.click();
            let zerr = ZError::new(Error::LockNotObtained {
                path: lockfile_path,
            });
            return Err(zerr);
        }
        let lockfile = lockfile.unwrap();
        // FileManager.
        let file_manager = FileManager::new(options.max_open_files);
        // Create the correct directories, or at least make sure they exist.
        if !path.join("sst").is_dir() {
            create_dir(path.join("sst"))
                .from_io()
                .with_context::<stringref>("sst", 2, &path.join("sst").to_string_lossy())?;
        }
        if !path.join("meta").is_dir() {
            create_dir(path.join("meta"))
                .from_io()
                .with_context::<stringref>("meta", 3, &path.join("meta").to_string_lossy())?;
        }
        // LSMTree.
        Trace::new("lp.lsm.open")
            .with_context::<stringref>("path", 1, &path.to_string_lossy())
            .finish();
        let lsm = LSMTree {
            root: path,
            options,
            file_manager,
            state: Mutex::new(State::default()),
            _lockfile: lockfile,
        };
        lsm.reload_from_disk()?;
        Ok(lsm)
    }

    pub fn ingest_ssts<P: AsRef<Path>>(&self, paths: &[P]) -> Result<(), ZError<Error>> {
        // Make a directory into which all files will be linked.
        let ingest_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                ZError::new(Error::SystemError {
                    context: "system clock before UNIX epoch".to_string(),
                })
                .wrap_error(Box::new(e))
            })?
            .as_secs_f64();
        let ingest_root = self.root.join(format!("ingest:{}", ingest_time));
        create_dir(&ingest_root).from_io()?;
        let mut ssts = Vec::new();
        // For each SST, hardlink it into the ingest root.
        for path in paths {
            let file = self.file_manager.open(path.as_ref().to_path_buf())?;
            let sst = SST::from_file_handle(file)?;
            // Extract the metadata.
            let setsum_str = sst.setsum();
            let metadata = sst.metadata()?;
            // Hard-link the file into place.
            let filename = setsum_str.clone() + ".sst";
            let target = ingest_root.join(&filename);
            hard_link(path.as_ref(), target).from_io()?;
            ssts.push((setsum_str, filename, metadata));
        }
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        ssts.sort_by(|lhs, rhs| compare_bytes(lhs.0.as_bytes(), rhs.0.as_bytes()));
        let meta_basename = format!("meta.{}.sst", ingest_time);
        let mut meta = SSTBuilder::new(
            ingest_root.join(&meta_basename),
            self.options.meta_options.clone(),
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
                Ok(_) => {}
                Err(err) => {
                    if err.kind() == ErrorKind::AlreadyExists {
                        return Err(ZError::new(Error::DuplicateSST { what: sst.clone() }));
                    } else {
                        return Err(ZError::new(err.into()));
                    }
                }
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
        self.reload_from_disk()
    }

    pub fn reload_from_disk(&self) -> Result<(), ZError<Error>> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut state = self.state.lock().unwrap();
        let mut metadata_files = Vec::new();
        let mut cursors: Vec<Box<dyn Cursor>> = Vec::new();
        for meta in read_dir(self.root.join("meta")).from_io()? {
            let meta = meta.from_io()?;
            metadata_files.push(meta.path());
            let file = self.file_manager.open(meta.path())?;
            let sst = SST::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let cursor = MergingCursor::new(cursors)?;
        let mut cursor = PruningCursor::new(cursor, u64::max_value())?;
        cursor.seek_to_first()?;
        let mut metadatas = Vec::new();
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
                ZError::new(Error::Corruption {
                    context: "key is corrupted in metadata".to_string(),
                })
                .with_context::<stringref>(
                    "key",
                    1,
                    &String::from_utf8(value.key.to_vec()).unwrap_or("<corrupted>".to_string()),
                )
            })?;
            metadatas.push(metadata);
        }
        state.metadata_files = metadata_files;
        state.sst_metadata = metadatas;
        Ok(())
    }

    pub fn debug_dump(&self) {
        let state = self.state.lock().unwrap();
        println!("[metadata]");
        for meta in read_dir(self.root.join("meta")).expect("could not read dir") {
            let meta = meta.expect("could not read dirent");
            println!("metadata sst {}", meta.path().display());
        }

        println!("\n[cached ssts]");
        let mut cached_ssts = HashSet::new();
        for metadata in state.sst_metadata.iter() {
            println!("{}.sst first_key=\"{}\", last_key=\"{}\" smallest_timestamp={} biggest_timestamp={}",
                metadata.setsum(),
                metadata.first_key_escaped(),
                metadata.last_key_escaped(),
                metadata.smallest_timestamp,
                metadata.biggest_timestamp,
            );
            cached_ssts.insert(metadata.setsum() + ".sst");
        }

        println!("\n[ssts not loaded into memory]");
        for sst in read_dir(self.root.join("sst")).expect("could not read dir") {
            let name = sst.expect("could not understand dirent").file_name().into_string().expect("could not read OsString");
            if !cached_ssts.contains(&name) {
                println!("{}", name);
            }
        }

        println!("\n[ssts not present on disk]");
        for sst in cached_ssts.iter() {
            if !self.root.join("sst").join(sst).exists() {
                println!("{}", sst);
            }
        }
    }
}
