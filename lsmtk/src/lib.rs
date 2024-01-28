use std::fmt::Debug;
use std::fs::create_dir;
use std::path::{Path, PathBuf};

use biometrics::Collector;
use mani::ManifestOptions;
use setsum::Setsum;
use sst::gc::GarbageCollectionPolicy;
use sst::log::LogOptions;
use sst::SstOptions;
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

mod kvs;
mod reference_counter;
mod tree;
mod verifier;

pub use kvs::{KeyValueStore, WriteBatch};
pub use tree::{CompactionID, LsmTree, NUM_LEVELS};
pub use verifier::{LsmVerifier, ManifestVerifier};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

pub fn register_biometrics(collector: &Collector) {
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
pub fn TRASH_LOG<P: AsRef<Path>>(root: P, number: u64) -> PathBuf {
    TRASH_ROOT(root).join(format!("log.{number}"))
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
        Ok(create_dir(&path).as_z().with_info(commentary, path)?)
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

#[derive(Clone, zerror_derive::Z)]
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
        path: String,
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
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Number of bytes to use for the sst cache.", "BYTES")
    )]
    sst_cache_bytes: usize,
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
            max_open_files: 1 << 19,
            max_compaction_bytes: 1 << 29,
            max_compaction_files: 1 << 6,
            l0_mandatory_compaction_threshold_files: 4,
            l0_mandatory_compaction_threshold_bytes: 1 << 26,
            l0_write_stall_threshold_files: 12,
            l0_write_stall_threshold_bytes: 1 << 28,
            memtable_size_bytes: 1 << 26,
            gc_policy: GarbageCollectionPolicy::try_from("versions = 1").unwrap(),
            sst_cache_bytes: 1 << 26,
        }
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
            let sst = sst::Sst::new(SstOptions::default(), sst_path).as_z().pretty_unwrap();
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
    use keyvalint::Cursor;
    use mani::{Edit, Manifest};
    use sst::Builder;

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
            "006c171eacb7d291d0a7ff7725d09411731ae623625b10a933859a2a4a1f8495",
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
    }

    #[test]
    fn orphan_cleanup() {
        let root = test_util::test_root(module_path!(), line!());
        let options = LsmtkOptions {
            path: root.clone(),
            ..Default::default()
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
        edit.info(
            'I',
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("manifest info should never fail");
        edit.info(
            'O',
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("manifest info should never fail");
        edit.info(
            'D',
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("manifest info should never fail");
        mani.apply(edit).expect("manifest apply should never fail");
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
            "006c171eacb7d291d0a7ff7725d09411731ae623625b10a933859a2a4a1f8495",
        )
        .expect("manifest info should never fail");
        mani.apply(edit).expect("manifest apply should never fail");
        let mut edit = Edit::default();
        edit.rm("fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a")
            .expect("manifest edit should never fail");
        edit.info(
            'I',
            "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a",
        )
        .expect("manifest info should never fail");
        edit.info(
            'O',
            "0000000000000000000000000000000000000000000000000000000000000000",
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
        assert!(TRASH_SST(
            &root,
            Setsum::from_hexdigest(
                "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a"
            )
            .expect("valid setsum")
        )
        .exists());
    }
    // TODO(rescrv): two log files
}
