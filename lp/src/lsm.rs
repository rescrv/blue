use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;

use util::lockfile::Lockfile;

use buffertk::{stack_pack, Unpacker};

use super::file_manager::FileManager;
use super::merging_cursor::MergingCursor;
use super::sst::{SST, SSTBuilder, SSTBuilderOptions, SSTMetadata};
use super::{compare_bytes, Builder, Cursor, Error};

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
    pub fn open<P: AsRef<Path>>(options: LSMOptions, path: P) -> Result<LSMTree, Error> {
        let path: PathBuf = path.as_ref().canonicalize()?;
        // Deal with the lockfile first.
        let lockfile_path = path.join("LOCKFILE");
        let lockfile = if options.wait_for_lock {
            Lockfile::wait(lockfile_path.clone())?
        } else {
            Lockfile::lock(lockfile_path.clone())?
        };
        if lockfile.is_none() {
            return Err(Error::LockNotObtained { path: lockfile_path });
        }
        let lockfile = lockfile.unwrap();
        // FileManager.
        let file_manager = FileManager::new(options.max_open_files);
        // Create the correct directories, or at least make sure they exist.
        if !path.join("sst").is_dir() {
            create_dir(path.join("sst"))?;
        }
        if !path.join("meta").is_dir() {
            create_dir(path.join("meta"))?;
        }
        // LSMTree.
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

    pub fn ingest_ssts<P: AsRef<Path>>(&self, paths: &[P]) -> Result<(), Error> {
        // Make a directory into which all files will be linked.
        let ingest_time = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
            Error::SystemError {
                context: "system clock before UNIX epoch".to_string(),
            }
        })?.as_secs_f64();
        let ingest_root = self.root.join(format!("ingest:{}", ingest_time));
        create_dir(&ingest_root)?;
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
            hard_link(path.as_ref(), target)?;
            ssts.push((setsum_str, filename, metadata));
        }
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        ssts.sort_by(|lhs, rhs| { compare_bytes(lhs.0.as_bytes(), rhs.0.as_bytes()) });
        let meta_basename = format!("meta.{}.sst", ingest_time);
        let mut meta = SSTBuilder::new(ingest_root.join(&meta_basename), self.options.meta_options.clone())?;
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
                        return Err(Error::DuplicateSST {
                            what: sst.clone(),
                        });
                    } else {
                        return Err(err.into());
                    }
                },
            }
        }
        // Move the metadata
        let meta_path = self.root.join("meta").join(&format!("{}.{}.sst", meta.setsum(), ingest_time));
        hard_link(ingest_root.join(&meta_basename), meta_path)?;
        // Unlink the ingest directory last.
        for (_, sst, _) in ssts.iter() {
            remove_file(ingest_root.join(&sst))?;
        }
        remove_file(ingest_root.join(&meta_basename))?;
        remove_dir(ingest_root)?;
        self.reload_from_disk()
    }

    pub fn reload_from_disk(&self) -> Result<(), Error> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut state = self.state.lock().unwrap();
        let mut iters: Vec<Box<dyn Cursor>> = Vec::new();
        for meta in read_dir(self.root.join("meta"))? {
            let meta = meta?;
            let file = self.file_manager.open(meta.path())?;
            let sst = SST::from_file_handle(file)?;
            iters.push(Box::new(sst.iterate()));
        }
        let mut iter = MergingCursor::new(iters)?;
        iter.seek_to_first()?;
        let mut metadatas = Vec::new();
        loop {
            iter.next()?;
            let value = match iter.value() {
                Some(v) => v,
                None => {
                    break;
                }
            };
            let mut up = Unpacker::new(value.value.unwrap_or(&[]));
            let metadata: SSTMetadata = up.unpack().map_err(|_| {
                Error::Corruption {
                    context: format!("{} is corrupted in metadata", String::from_utf8(value.key.to_vec()).unwrap_or("<corrupted>".to_string())),
                }
            })?;
            metadatas.push(metadata);
        }
        state.sst_metadata = metadatas;
        Ok(())
    }
}
