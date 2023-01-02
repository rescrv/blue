use std::env::args;

use lp::lsm::{LSMOptions, LSMTree};

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 2 {
        eprintln!("lsm-sst-ingest [lsm tree location] [sst] ...");
        std::process::exit(-1);
    }
    let lsm = &args[1];
    let ssts = &args[2..];
    let opts = LSMOptions::default();
    let tree = LSMTree::open(opts, lsm).expect("could not open LSM tree");
    tree.ingest_ssts(ssts).expect("could not ingest SSTs");
}

/*
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
        // For each SST, hardlink it into the ingest root.
        for path in paths {
            let file = self.file_manager.open(path.as_ref().to_path_buf())?;
            let sst = SST::from_file_handle(file)?;
            let setsum = sst.setsum();
            let mut filename = String::with_capacity(68);
            for i in 0..setsum.len() {
                write!(&mut filename, "{:02x}", setsum[i]).expect("unable to write to string");
            }
            filename += ".sst";
            let target = ingest_root.join(filename);
            hard_link(path.as_ref(), target)?;
        }
        todo!();
    }
}
*/
