use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file, File};
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use util::lockfile::Lockfile;
use util::time::now;

use buffertk::{stack_pack, Unpacker};

use prototk::field_types::*;

use biometrics::Counter;

use hey_listen::{HeyListen, Stationary};

use clue::Trace;

use zerror::{FromIOError, ZError, ZErrorResult};

use super::file_manager::FileManager;
use super::merging_cursor::MergingCursor;
use super::options::CompactionOptions;
use super::pruning_cursor::PruningCursor;
use super::setsum::Setsum;
use super::sst::{SSTBuilder, SSTMetadata, SST};
use super::{compare_bytes, Builder, Cursor, Error};

pub mod compaction;
pub use compaction::Compaction;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_NOT_OBTAINED: Counter = Counter::new("lp.db.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lp.db.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

///////////////////////////////////////////// constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn LOCKFILE<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("LOCKFILE")
}

/////////////////////////////////////////// ErrorHandler ///////////////////////////////////////////

trait ErrorHandler {
    fn error(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>>;
    fn warning(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>>;
}

#[derive(Default)]
struct NormalErrorHandler {}

impl ErrorHandler for NormalErrorHandler {
    fn error(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>> {
        Err(err)
    }

    fn warning(&mut self, _: ZError<Error>) -> Result<(), ZError<Error>> {
        Ok(())
    }
}

#[derive(Default)]
struct ParanoidErrorHandler {}

impl ErrorHandler for ParanoidErrorHandler {
    fn error(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>> {
        Err(err)
    }

    fn warning(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>> {
        Err(err)
    }
}

#[derive(Default)]
struct FsckErrorHandler {
    errors: Vec<ZError<Error>>,
}

impl ErrorHandler for FsckErrorHandler {
    fn error(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>> {
        self.errors.push(err);
        Ok(())
    }

    fn warning(&mut self, err: ZError<Error>) -> Result<(), ZError<Error>> {
        self.errors.push(err);
        Ok(())
    }
}

/////////////////////////////////////////// get_lockfile ///////////////////////////////////////////

pub fn get_lockfile(options: &DBOptions, root: &PathBuf) -> Result<Lockfile, ZError<Error>> {
    // Deal with making the root directory.
    if root.is_dir() && options.error_if_exists {
        return Err(ZError::new(Error::DBExists { path: root.clone() }));
    }
    if !root.is_dir() && !options.create_if_missing {
        return Err(ZError::new(Error::DBNotExist { path: root.clone() }));
    } else if !root.is_dir() {
        Trace::new("lp.db.create")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        create_dir(&root)
            .from_io()
            .with_context::<string, 1>("root", &root.to_string_lossy())?;
    }
    // Deal with the lockfile first.
    let lockfile = if options.wait_for_lock {
        Trace::new("lp.db.wait_lockfile")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        Lockfile::wait(LOCKFILE(&root))
            .from_io()
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .with_backtrace()?
    } else {
        Trace::new("lp.db.nowait_lockfile")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        Lockfile::lock(LOCKFILE(&root))
            .from_io()
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .with_backtrace()?
    };
    if lockfile.is_none() {
        LOCK_NOT_OBTAINED.click();
        let zerr = ZError::new(Error::LockNotObtained {
            path: LOCKFILE(root),
        });
        return Err(zerr);
    }
    Trace::new("lp.db.lock_obtained")
        .with_context::<string, 1>("root", &root.to_string_lossy())
        .finish();
    Ok(lockfile.unwrap())
}

///////////////////////////////////////////// DBOptions ////////////////////////////////////////////

pub struct DBOptions {
    create_if_missing: bool,
    error_if_exists: bool,
    paranoid_checks: bool,
    wait_for_lock: bool,
    max_open_files: usize,
    compaction: CompactionOptions,
}

impl Default for DBOptions {
    fn default() -> Self {
        Self {
            create_if_missing: true,
            error_if_exists: false,
            paranoid_checks: true,
            wait_for_lock: true,
            max_open_files: 1 << 20,
            compaction: CompactionOptions::default(),
        }
    }
}

//////////////////////////////////////////////// DB ////////////////////////////////////////////////

// There are at least two different data stores to be written that share the same interface.
pub trait DB {
    fn open<P: AsRef<Path>>(options: DBOptions, root: P) -> Result<Self, ZError<Error>> where Self: Sized;
    fn fsck<P: AsRef<Path>>(options: DBOptions, root: P) -> Vec<ZError<Error>> where Self: Sized;

    fn ingest(&self, paths: &[PathBuf]) -> Result<(), ZError<Error>>;
    fn reload(&self) -> Result<(), ZError<Error>>;

    fn suggest_compactions(&self) -> Result<Vec<Compaction>, ZError<Error>>;
}

////////////////////////////////////////////// DBType //////////////////////////////////////////////

pub enum DBType {
    Generational,
    Tree,
}

impl DBType {
    pub fn open<P: AsRef<Path>>(&self, options: DBOptions, root: P) -> Result<Box<dyn DB>, ZError<Error>> {
        Ok(match self {
            Generational => { Box::new(GenerationalDB::open(options, root)?) as Box<dyn DB> },
            Tree => { Box::new(TreeDB::open(options, root)?) as Box<dyn DB> },
        })
    }

    pub fn fsck<P: AsRef<Path>>(&self, options: DBOptions, root: P) -> Vec<ZError<Error>> {
        match self {
            Generational => { GenerationalDB::fsck(options, root) },
            Tree => { TreeDB::fsck(options, root) },
        }
    }
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

#[derive(Default)]
struct State {
    sst_metadata: Vec<SSTMetadata>,
}

impl State {
    fn get_metadata_by_setsum(&self, setsum: &str) -> Option<SSTMetadata> {
        for sst in self.sst_metadata.iter() {
            if sst.setsum() == setsum {
                return Some(sst.clone())
            }
        }
        None
    }
}

////////////////////////////////////////////// TreeDB //////////////////////////////////////////////

pub struct TreeDB {
    root: PathBuf,
    options: DBOptions,
    file_manager: FileManager,
    state: Mutex<Arc<State>>,
    _lockfile: Lockfile,
}

impl TreeDB {
    #[allow(non_snake_case)]
    fn SST<P: AsRef<Path>>(root: P) -> PathBuf {
        root.as_ref().to_path_buf().join("sst")
    }

    pub fn suggest_compactions(&self) -> Result<Vec<Compaction>, ZError<Error>> {
        let state = self.get_state();
        let graph = compaction::Graph::new(self.options.compaction.clone(), &state.sst_metadata)?;
        let mut compactions = graph.compactions();
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        Ok(compactions)
    }

    pub fn setup_compaction(&self, smallest_snapshot: u64, setsums: &[&str]) -> Result<String, ZError<Error>> {
        let mut ssts = Vec::new();
        let state = self.get_state();
        for setsum in setsums.iter() {
            let sst = state.get_metadata_by_setsum(setsum);
            if sst.is_none() {
                return Err(ZError::new(Error::SSTNotFound {
                    setsum: setsum.to_string(),
                }));
            }
            ssts.push(sst.unwrap());
        }
        //let compaction = Compaction::from_inputs(self.options.compaction.clone(), smallest_snapshot, ssts);
        //Ok(compaction.setup(&self.root)?)
        todo!();
    }

    fn get_state(&self) -> Arc<State> {
        let state = self.state.lock().unwrap();
        Arc::clone(&state)
    }
}

impl DB for TreeDB {
    fn open<P: AsRef<Path>>(options: DBOptions, root: P) -> Result<TreeDB, ZError<Error>> {
        let root: PathBuf = root
            .as_ref()
            .canonicalize()
            .from_io()
            .with_context::<string, 1>("root", &root.as_ref().to_string_lossy())?;
        let lockfile = get_lockfile(&options, &root)?;
        let file_manager = FileManager::new(options.max_open_files);
        // Create the correct directories, or at least make sure they exist.
        if !Self::SST(&root).is_dir() {
            create_dir(Self::SST(&root))
                .from_io()
                .with_context::<string, 2>("sst", &Self::SST(&root).to_string_lossy())?;
        }
        if !root.join("meta").is_dir() {
            create_dir(root.join("meta"))
                .from_io()
                .with_context::<string, 3>("meta", &root.join("meta").to_string_lossy())?;
        }
        // DB.
        Trace::new("lp.db.open")
            .with_context::<string, 1>("root", &root.to_string_lossy())
            .finish();
        let db = Self {
            root: root,
            options,
            file_manager,
            state: Mutex::new(Arc::new(State::default())),
            _lockfile: lockfile,
        };
        db.reload()?;
        Ok(db)
    }

    fn fsck<P: AsRef<Path>>(options: DBOptions, root: P) -> Vec<ZError<Error>> {
        todo!();
    }

    fn ingest(&self, paths: &[PathBuf]) -> Result<(), ZError<Error>> {
        // Make a directory into which all files will be linked.
        let ingest_time = now::millis();
        let ingest_root = self.root.join(format!("ingest:{}", ingest_time));
        create_dir(&ingest_root).from_io()?;
        let mut ssts = Vec::new();
        // For each SST, hardlink it into the ingest root.
        for path in paths {
            let file = self.file_manager.open(path.clone())?;
            let sst = SST::from_file_handle(file)?;
            // Extract the metadata.
            let setsum_str = sst.setsum();
            let metadata = sst.metadata()?;
            // Hard-link the file into place.
            let filename = setsum_str.clone() + ".sst";
            let target = ingest_root.join(&filename);
            hard_link(path, target).from_io()?;
            ssts.push((setsum_str, filename, metadata));
        }
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        ssts.sort_by(|lhs, rhs| compare_bytes(lhs.0.as_bytes(), rhs.0.as_bytes()));
        let meta_basename = format!("meta.{}.sst", ingest_time);
        let mut meta = SSTBuilder::new(
            ingest_root.join(&meta_basename),
            self.options.compaction.sst_options.clone(),
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
                Ok(_) => {},
                Err(err) => {
                    if err.kind() == ErrorKind::AlreadyExists {
                        return Err(ZError::new(Error::DuplicateSST { what: sst.clone() }));
                    } else {
                        return Err(ZError::new(err.into()));
                    }
                },
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
        self.reload()
    }

    fn reload(&self) -> Result<(), ZError<Error>> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut state = self.state.lock().unwrap();
        let mut cursors: Vec<Box<dyn Cursor>> = Vec::new();
        for meta in read_dir(self.root.join("meta")).from_io()? {
            let meta = meta.from_io()?;
            let file = self.file_manager.open(meta.path())?;
            let sst = SST::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let cursor = MergingCursor::new(cursors)?;
        let mut cursor = PruningCursor::new(cursor, u64::max_value())?;
        cursor.seek_to_first()?;
        let mut sst_metadata = Vec::new();
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
                .with_context::<string, 1>(
                    "key",
                    &String::from_utf8(value.key.to_vec()).unwrap_or("<corrupted>".to_string()),
                )
            })?;
            sst_metadata.push(metadata);
        }
        *state = Arc::new(State {
            sst_metadata,
        });
        Ok(())
    }

    fn suggest_compactions(&self) -> Result<Vec<Compaction>, ZError<Error>> {
        let state = self.get_state();
        let graph = compaction::Graph::new(self.options.compaction.clone(), &state.sst_metadata)?;
        let mut compactions = graph.compactions();
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        Ok(compactions)
    }
}

///////////////////////////////////////////// Manifest /////////////////////////////////////////////

#[derive(Default)]
struct Manifest {
    generation: u64,
    setsum: Setsum,
    dropped: Option<Setsum>,
    ingests: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
}

impl Manifest {
    fn parse<P: AsRef<Path>, H: ErrorHandler>(path: P, errors: &mut H) -> Result<Manifest, ZError<Error>> {
        let mut has_generation = false;
        let mut has_setsum = false;
        let mut num_ingests = None;
        let mut num_outputs = None;
        let mut manifest = Manifest {
            generation: 0,
            setsum: Setsum::default(),
            dropped: None,
            ingests: Vec::new(),
            outputs: Vec::new(),
        };
        let file = match File::open(&path).from_io() {
            Ok(file) => { BufReader::new(file) },
            Err(e) => {
                errors.error(e.with_context::<string, 1>("file", &path.as_ref().to_string_lossy()))?;
                return Ok(manifest)
            }
        };
        for line in file.lines() {
            let line = match line {
                Ok(line) => { line },
                Err(e) => {
                    errors.error(ZError::new(e.into()))?;
                    continue;
                },
            };
            let pieces: Vec<&str> = line.split(" ").collect();
            if pieces.len() != 2 {
                errors.error(ZError::new(Error::InvalidManifestLine { line }))?;
                continue;
            }
            let cmd = pieces[0];
            let arg = pieces[1];
            match (cmd, arg) {
                ("generation", x) => {
                    let mut current = if has_generation { Some(manifest.generation.clone()) } else { None };
                    if let Some(err) = Self::parse_u64_command(x, &mut current, "generation") {
                        errors.error(err)?;
                    }
                    if let Some(value) = current {
                        has_generation = true;
                        manifest.generation = value;
                    }
                }
                ("setsum", x) => {
                    let mut current = if has_setsum { Some(manifest.setsum.clone()) } else { None };
                    if let Some(err) = Self::parse_setsum_command(x, &mut current, "setsum") {
                        errors.error(err)?;
                    }
                    if let Some(value) = current {
                        has_setsum = true;
                        manifest.setsum = value;
                    }
                }
                ("dropped", x) => {
                    if let Some(err) = Self::parse_setsum_command(x, &mut manifest.dropped, "dropped") {
                        errors.error(err)?;
                    }
                }
                ("ingests", x) => {
                    if let Some(err) = Self::parse_u64_command(x, &mut num_ingests, "ingests") {
                        errors.error(err)?;
                    }
                }
                ("ingest", p) => {
                    manifest.ingests.push(PathBuf::from(p));
                },
                ("outputs", x) => {
                    if let Some(err) = Self::parse_u64_command(x, &mut num_outputs, "outputs") {
                        errors.error(err)?;
                    }
                }
                ("output", p) => {
                    manifest.outputs.push(PathBuf::from(p));
                },
                _ => {
                    errors.warning(ZError::new(Error::InvalidManifestCommand {
                        cmd: cmd.to_string(),
                        arg: arg.to_string(),
                    }))?;
                }
            }
        }
        if !has_generation {
            errors.error(ZError::new(Error::Corruption {
                context: "missing generation in manifest".to_string(),
            }))?;
        }
        if !has_setsum {
            errors.error(ZError::new(Error::Corruption {
                context: "missing setsum in manifest".to_string(),
            }))?;
        }
        if num_ingests.unwrap_or(0) != manifest.ingests.len() as u64 {
            errors.error(ZError::new(Error::Corruption {
                context: "wrong number of ingests".to_string(),
            })
            .with_context::<uint64, 1>("declared", num_outputs.unwrap_or(0))
            .with_context::<uint64, 2>("had", manifest.ingests.len() as u64))?;
        }
        if num_outputs.unwrap_or(0) != manifest.outputs.len() as u64 {
            errors.error(ZError::new(Error::Corruption {
                context: "wrong number of outputs".to_string(),
            })
            .with_context::<uint64, 1>("declared", num_outputs.unwrap_or(0))
            .with_context::<uint64, 2>("had", manifest.outputs.len() as u64))?;
        }
        Ok(manifest)
    }

    fn parse_u64_command(arg: &str, value: &mut Option<u64>, what: &str) -> Option<ZError<Error>> {
        let x: u64 = match arg.parse::<u64>() {
            Ok(x) => { x },
            Err(_) => {
                return Some(ZError::new(Error::Corruption {
                    context: format!("not a number: {}", arg),
                }));
            }
        };
        if let Some(value) = *value {
            if value != x {
                return Some(ZError::new(Error::Corruption {
                    context: format!("duplicate, conflicting {} commands: {} != {}", what, value, x),
                }));
            }
        }
        *value = Some(x);
        None
    }

    fn parse_setsum_command(arg: &str, value: &mut Option<Setsum>, what: &str) -> Option<ZError<Error>> {
        let x: Setsum = match Setsum::from_hexdigest(arg) {
            Some(x) => { x },
            None => {
                return Some(ZError::new(Error::Corruption {
                    context: format!("bad {} in manifest: {}", what, arg),
                }));
            },
        };
        if let Some(value) = value {
            if *value != x {
                return Some(ZError::new(Error::Corruption {
                    context: format!("duplicate, conflicting {}: {} != {}", what, value.hexdigest(), x.hexdigest()),
                }));
            }
        }
        *value = Some(x);
        None
    }
}

////////////////////////////////////////// GenerationalDB //////////////////////////////////////////

pub struct GenerationalDB {
    options: DBOptions,
    lockfile: Lockfile,
    file_manager: FileManager,
    root: PathBuf,
    state: Mutex<Arc<Manifest>>,
    cond_ingest: Condvar,
}

impl GenerationalDB {
    #[allow(non_snake_case)]
    fn INGEST<P: AsRef<Path>>(root: P) -> PathBuf {
        root.as_ref().to_path_buf().join("ingest")
    }

    #[allow(non_snake_case)]
    fn GENERATION_ROOT<P: AsRef<Path>>(root: P, generation: u64) -> PathBuf {
        root.as_ref().to_path_buf().join(format!("{}", generation))
    }

    #[allow(non_snake_case)]
    fn MANIFEST<P: AsRef<Path>>(root: P, generation: u64) -> PathBuf {
        Self::GENERATION_ROOT(root, generation).join("MANIFEST")
    }

    #[allow(non_snake_case)]
    fn SST<P: AsRef<Path>>(root: P, generation: u64, sst: P) -> PathBuf {
        Self::GENERATION_ROOT(root, generation).join(sst)
    }

    fn scan_generations<P: AsRef<Path>, H: ErrorHandler>(root: P, errors: &mut H) -> Result<Vec<u64>, ZError<Error>> {
        let mut generations = Vec::new();
        for entry in read_dir(&root).from_io()? {
            let path = entry.from_io()?.path();
            let entry = match path.file_name() {
                Some(e) => { e },
                None => {
                    errors.warning(ZError::new(Error::PathError {
                        path: path,
                        what: "no file name".to_owned(),
                    }))?;
                    continue;
                }
            };
            let entry = match entry.to_str() {
                Some(s) => { s },
                None => {
                    errors.warning(ZError::new(Error::PathError {
                        path: path,
                        what: "invalid unicode".to_owned(),
                    }))?;
                    continue;
                },
            };
            if entry == "ingest" || entry == "LOCKFILE" || entry == "meta" || entry == "sst" {
                continue;
            }
            let generation = match entry.parse::<u64>() {
                Ok(gen) => { gen },
                Err(_) => {
                    errors.warning(ZError::new(Error::PathError {
                        path: PathBuf::from(entry),
                        what: "not a number".to_string(),
                    }))?;
                    continue;
                }
            };
            generations.push(generation);
        }
        generations.sort();
        Ok(generations)
    }

    fn load_generation<P: AsRef<Path>>(options: &DBOptions, root: P, generation: u64) -> Result<Manifest, ZError<Error>> {
        let manifest = if options.paranoid_checks {
            Manifest::parse(Self::MANIFEST(&root, generation), &mut ParanoidErrorHandler::default())?
        } else {
            Manifest::parse(Self::MANIFEST(&root, generation), &mut NormalErrorHandler::default())?
        };
        Ok(manifest)
    }

    fn fsck_generation<P: AsRef<Path>, H: ErrorHandler>(root: P, generation: u64, errors: &mut H) -> Result<(), ZError<Error>> {
        let manifest = Manifest::parse(Self::MANIFEST(&root, generation), errors);
        // Checks against the manifest.
        let manifest = if let Ok(manifest) = manifest {
            manifest
        } else {
            return Ok(());
        };
        // Check generation is the one we thought we were when we wrote the manifest.
        if manifest.generation != generation {
            errors.error(ZError::new(Error::Corruption {
                context: "generation number in manifest does not match directory".to_string(),
            })
            .with_context::<uint64, 1>("generation", generation)
            .with_context::<uint64, 2>("manifest", manifest.generation))?;
        }
        // Check that every output exists and accumulate a setsum over the outputs.
        let gen_root = Self::GENERATION_ROOT(&root, generation);
        let mut setsum_acc = Setsum::default();
        for output in manifest.outputs.iter() {
            let path = gen_root.join(output);
            if path.is_file() {
                let sst = match SST::new(path) {
                    Ok(sst) => { sst },
                    Err(e) => {
                        errors.error(e)?;
                        continue;
                    },
                };
                let meta = match sst.metadata() {
                    Ok(meta) => { meta },
                    Err(e) => {
                        errors.error(e)?;
                        continue;
                    },
                };
                match Self::fsck_sst(sst, errors) {
                    Ok(_) => {},
                    Err(e) => {
                        errors.error(e)?;
                    }
                };
                let setsum = Setsum::from_digest(meta.setsum);
                setsum_acc = setsum_acc + setsum;
            } else {
                errors.error(ZError::new(Error::MissingSST {
                    path,
                }))?;
            }
        }
        if manifest.setsum != setsum_acc {
            errors.error(ZError::new(Error::InvalidManifestSetsum {
                manifest: manifest.setsum.hexdigest(),
                computed: setsum_acc.hexdigest(),
            }))?;
        }
        // Check that every file is included in outputs.
        let dir = match read_dir(gen_root).from_io() {
            Ok(dir) => { dir },
            Err(e) => {
                errors.error(e)?;
                return Ok(());
            }
        };
        for file in dir {
            let file = match file {
                Ok(file) => { file },
                Err(e) => {
                    errors.error(ZError::new(e.into()))?;
                    continue;
                }
            };
            let file_name = file.file_name();
            if file_name == "ingest" || file_name == "MANIFEST" {
                continue;
            }
            if !manifest.outputs.contains(&PathBuf::from(&file_name)) {
                errors.warning(ZError::new(Error::ExtraFile { path: file.path() }))?;
                continue;
            }
        }
        // TODO(rescrv): Compare across generations to check setsum.
        Ok(())
    }

    fn fsck_sst<H: ErrorHandler>(sst: SST, errors: &mut H) -> Result<(), ZError<Error>> {
        let mut setsum = Setsum::default();
        let mut cursor = sst.cursor();
        cursor.seek_to_first()?;
        loop {
            if let Err(e) = cursor.next() {
                errors.error(e)?;
                break;
            }
            if let Some(kvr) = cursor.value() {
                match kvr.value {
                    Some(v) => { setsum.put(kvr.key, kvr.timestamp, v); },
                    None => { setsum.del(kvr.key, kvr.timestamp); },
                }
            } else {
                break;
            }
        }
        let metadata = match sst.metadata() {
            Ok(metadata) => { metadata },
            Err(e) => {
                errors.error(e)?;
                return Ok(());
            }
        };
        if setsum.digest() != metadata.setsum {
            errors.error(ZError::new(Error::InvalidSSTSetsum {
                expected: Setsum::from_digest(metadata.setsum).hexdigest(),
                computed: setsum.hexdigest(),
            }))?;
        }
        Ok(())
    }
}

impl DB for GenerationalDB {
    fn open<P: AsRef<Path>>(options: DBOptions, root: P) -> Result<Self, ZError<Error>> {
        let root: PathBuf = root
            .as_ref()
            .canonicalize()
            .from_io()
            .with_context::<string, 1>("root", &P::as_ref(&root).to_string_lossy())?;
        let lockfile = get_lockfile(&options, &root)?;
        let file_manager = FileManager::new(options.max_open_files);
        // Create the necessary structures for the database.
        if !Self::INGEST(&root).is_dir() {
            create_dir(Self::INGEST(&root).clone())
                .from_io()
                .with_context::<string, 1>("root", &root.to_string_lossy())?;
        }
        // Parse off the disk.
        let db = Self {
            options,
            lockfile,
            file_manager,
            root,
            state: Mutex::new(Arc::new(Manifest::default())),
            cond_ingest: Condvar::new(),
        };
        db.reload()?;
        Ok(db)
    }

    fn fsck<P: AsRef<Path>>(mut options: DBOptions, root: P) -> Vec<ZError<Error>> {
        let mut errors = Vec::new();
        options.paranoid_checks = false;
        options.wait_for_lock = false;
        // Get Lockfile
        let root: PathBuf = match root
            .as_ref()
            .canonicalize()
            .from_io()
            .with_context::<string, 1>("root", &P::as_ref(&root).to_string_lossy())
        {
            Ok(root) => { root },
            Err(e) => { return vec![e.into()]; },
        };
        let _lockfile = match get_lockfile(&options, &root) {
            Ok(lockfile) => { Some(lockfile) },
            Err(e) => {
                errors.push(e);
                None
            },
        };
        // Scan the generation numbers.
        let mut errors = FsckErrorHandler::default();
        let generations: Vec<u64> = Self::scan_generations(&root, &mut errors).unwrap();
        for gen in generations.into_iter() {
            Self::fsck_generation(&root, gen, &mut errors).expect("all errors to be swallowed");
        }
        errors.errors
    }

    fn ingest(&self, paths: &[PathBuf]) -> Result<(), ZError<Error>> {
        for path in paths.into_iter() {
            let path: &Path = path.as_ref();
            let basename = match path.file_name() {
                Some(basename) => { basename },
                None => {
                    return Err(ZError::new(Error::PathError {
                        path: path.to_path_buf(),
                        what: "could not compute basename".to_string(),
                    }));
                },
            };
            let link = Self::INGEST(&self.root).join(basename);
            hard_link(path, link).from_io()?;
        }
        self.cond_ingest.notify_one();
        Ok(())
    }

    fn reload(&self) -> Result<(), ZError<Error>> {
        // Scan the generation numbers.
        let mut generations = if self.options.paranoid_checks {
            Self::scan_generations(&self.root, &mut ParanoidErrorHandler::default())?
        } else {
            Self::scan_generations(&self.root, &mut NormalErrorHandler::default())?
        };
        generations.reverse();
        let generation = if generations.is_empty() {
            0
        } else {
            generations[0]
        };
        // Create the database.
        let manifest = Self::load_generation(&self.options, &self.root, generation)?;
        let mut state = self.state.lock().unwrap();
        *state = Arc::new(manifest);
        Ok(())
    }

    fn suggest_compactions(&self) -> Result<Vec<Compaction>, ZError<Error>> {
        let mut ingests: Vec<PathBuf> = Vec::new();
        let mut compactions = Vec::new();
        let mut state = self.state.lock().unwrap();
        for ingest in read_dir(Self::INGEST(&self.root)).from_io()?  {
            ingests.push(ingest.from_io()?.path());
        }
        if ingests.len() > 0 {
            let mut inputs = Vec::new();
            inputs.append(&mut ingests);
            for sst in state.outputs.iter() {
                inputs.push(Self::SST(&self.root, state.generation, sst));
            }
            compactions.push(Compaction::from_paths(self.options.compaction.clone(), inputs, 0)?);
        }
        Ok(compactions)
    }
}
