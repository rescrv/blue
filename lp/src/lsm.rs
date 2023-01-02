use std::fs::{create_dir, hard_link, remove_dir, remove_file};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use util::lockfile::Lockfile;

use super::file_manager::FileManager;
use super::sst::{SST, SSTBuilder, SSTBuilderOptions};
use super::{Builder, Error};

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

////////////////////////////////////////////// LSMTree /////////////////////////////////////////////

pub struct LSMTree {
    root: PathBuf,
    options: LSMOptions,
    file_manager: FileManager,
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
        Ok(LSMTree {
            root: path,
            options,
            file_manager,
            _lockfile: lockfile,
        })
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
        // Create one file that will be linked into meta.
        let meta_base = format!("meta.{}.sst", ingest_time);
        let meta = SSTBuilder::new(ingest_root.join(&meta_base), self.options.meta_options.clone())?;
        // For each SST, hardlink it into the ingest root.
        let mut ssts = Vec::new();
        for path in paths {
            let file = self.file_manager.open(path.as_ref().to_path_buf())?;
            let sst = SST::from_file_handle(file)?;
            // TODO(rescrv): put the sst metadata into meta.
            let filename = sst.setsum() + ".sst";
            let target = ingest_root.join(&filename);
            hard_link(path.as_ref(), target)?;
            ssts.push(filename);
        }
        // Now that everything's been put in the ingest root, link into the sst dir.
        for sst in ssts.iter() {
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
        let meta = meta.seal()?;
        let meta_path = self.root.join("meta").join(&format!("{}.{}.sst", meta.setsum(), ingest_time));
        hard_link(ingest_root.join(&meta_base), meta_path)?;
        //let meta_path = ingest_root.join(format!("meta.{}.sst", ingest_time));
        // Unlink the ingest directory last.
        for sst in ssts.iter() {
            remove_file(ingest_root.join(&sst))?;
        }
        remove_file(ingest_root.join(&meta_base))?;
        remove_dir(ingest_root)?;
        Ok(())
    }
}
