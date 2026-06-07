use std::fmt::Debug;
use std::fs::create_dir;
use std::path::{Path, PathBuf};

use biometrics::Collector;
pub use handled::{SError, SExpr, extract_string};
use mani::ManifestOptions;
use setsum::Setsum;
use sst::SstOptions;
use sst::gc::GarbageCollectionPolicy;
use sst::log::LogOptions;

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

fn ensure_dir(path: PathBuf, commentary: &str) -> Result<(), SError> {
    if !path.is_dir() {
        create_dir(&path).map_err(|err| {
            system_error(err)
                .with_string_field("path", path.to_string_lossy().as_ref())
                .with_string_field("directory", commentary)
        })?;
        Ok(())
    } else {
        Ok(())
    }
}

fn make_all_dirs<P: AsRef<Path>>(root: P) -> Result<(), SError> {
    ensure_dir(VERIFY_ROOT(&root), "verify")?;
    ensure_dir(SST_ROOT(&root), "sst")?;
    ensure_dir(COMPACTION_ROOT(&root), "compaction")?;
    ensure_dir(TRASH_ROOT(&root), "trash")?;
    ensure_dir(INGEST_ROOT(&root), "ingest")?;
    ensure_dir(TEMP_ROOT(&root), "tmp")?;
    Ok(())
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

const PHASE: &str = "lsmtk";

/// A system error was encountered.
pub const CODE_SYSTEM_ERROR: &str = "system-error";
/// Persistent LSM state is corrupt.
pub const CODE_CORRUPTION: &str = "corruption";
/// An internal invariant was violated.
pub const CODE_LOGIC_ERROR: &str = "logic-error";
/// An SST already exists where a new SST would be linked.
pub const CODE_DUPLICATE_SST: &str = "duplicate-sst";
/// Verification should back off and retry later.
pub const CODE_BACKOFF: &str = "backoff";

fn error(code: &str) -> SError {
    SError::new(PHASE).with_code(code)
}

fn system_error(err: std::io::Error) -> SError {
    error(CODE_SYSTEM_ERROR)
        .with_message("lsmtk system error")
        .with_string_field("kind", &format!("{:?}", err.kind()))
        .with_string_field("cause", &err.to_string())
}

fn corruption(context: impl AsRef<str>) -> SError {
    error(CODE_CORRUPTION)
        .with_message("lsmtk corruption")
        .with_string_field("context", context.as_ref())
}

fn logic_error(context: impl AsRef<str>) -> SError {
    error(CODE_LOGIC_ERROR)
        .with_message("lsmtk logic error")
        .with_string_field("context", context.as_ref())
}

fn duplicate_sst(what: impl AsRef<str>) -> SError {
    error(CODE_DUPLICATE_SST)
        .with_message("duplicate SST")
        .with_string_field("what", what.as_ref())
}

fn backoff(path: impl AsRef<str>) -> SError {
    error(CODE_BACKOFF)
        .with_message("verification backoff")
        .with_string_field("path", path.as_ref())
}

pub(crate) trait ResultSErrorExt<T> {
    fn with_debug_field<V: Debug>(self, name: &str, value: V) -> Result<T, SError>;
}

impl<T, E> ResultSErrorExt<T> for Result<T, E>
where
    E: Into<SError>,
{
    fn with_debug_field<V: Debug>(self, name: &str, value: V) -> Result<T, SError> {
        self.map_err(|err| err.into().with_debug_field(name, value))
    }
}

fn error_field<'a>(err: &'a SError, name: &str) -> Option<&'a SExpr> {
    match err.detail() {
        SExpr::List(fields) => fields.iter().find_map(|field| match field {
            SExpr::List(pair) if pair.len() == 2 => match &pair[0] {
                SExpr::Atom(field_name) if field_name == name => Some(&pair[1]),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

pub fn error_code(err: &SError) -> Option<&str> {
    match error_field(err, "code") {
        Some(SExpr::Atom(code)) => Some(code.as_str()),
        _ => None,
    }
}

pub fn backoff_path(err: &SError) -> Option<String> {
    if error_code(err) == Some(CODE_BACKOFF) {
        error_field(err, "path").map(extract_string)
    } else {
        None
    }
}

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

////////////////////////////////////////////// tracing /////////////////////////////////////////////

pub static TRACING: indicio::Collector = indicio::Collector::new();

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
        let path = PathBuf::from(format!("{root}_{line}"));
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
                    std::fs::remove_file(&tmp).unwrap_or_else(|err| panic!("{err}"));
                }
                std::fs::create_dir_all(&$output_dir).unwrap_or_else(|err| panic!("{err}"));
                let options = sst::SstOptions::default();
                let mut sst_builder = sst::SstBuilder::new(options, &tmp).unwrap_or_else(|err| panic!("{err}"));
                $(sst_builder.put($key.as_bytes(), 1, $val.as_bytes()).unwrap_or_else(|err| panic!("{err}"));)*
                let sst = sst_builder.seal().unwrap_or_else(|err| panic!("{err}"));
                let setsum = sst.fast_setsum();
                let output = PathBuf::from(&$output_dir).join(format!("{}.sst", setsum.hexdigest()));
                std::fs::rename(&tmp, &output).unwrap_or_else(|err| panic!("{err}"));
                output
            }
        };
    }

    #[macro_export]
    macro_rules! sst_check {
        ($test_root:expr; $relative_path:literal: $($key:literal => $val:literal,)*) => {
            let sst_path = PathBuf::from($test_root).join($relative_path);
            let sst = sst::Sst::<sst::file_manager::FileHandle>::new(SstOptions::default(), sst_path).unwrap_or_else(|err| panic!("{err}"));
            let mut cursor = sst.cursor();
            cursor.seek_to_first().unwrap_or_else(|err| panic!("{err}"));
            $(
                cursor.next().unwrap_or_else(|err| panic!("{err}"));
                let kvr = cursor.key_value().expect("key-value pair should not be none");
                assert_eq!($key.as_bytes(), kvr.key);
                let value: &[u8] = $val.as_bytes();
                assert_eq!(Some(value), kvr.value);
            )*
            cursor.next().unwrap_or_else(|err| panic!("{err}"));
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
                    std::fs::remove_file(&tmp).unwrap_or_else(|err| panic!("{err}"));
                }
                let output_file = PathBuf::from($test_root).join($relative_path);
                std::fs::create_dir_all(&output_file.parent().map(PathBuf::from).unwrap_or(PathBuf::from("."))).unwrap_or_else(|err| panic!("{err}"));
                let options = sst::LogOptions::default();
                let mut log_builder = sst::LogBuilder::new(options, &tmp).unwrap_or_else(|err| panic!("{err}"));
                $(log_builder.put($key.as_bytes(), 1, $val.as_bytes()).unwrap_or_else(|err| panic!("{err}"));)*
                let setsum = log_builder.seal().unwrap_or_else(|err| panic!("{err}")).0;
                std::fs::rename(&tmp, &output_file).unwrap_or_else(|err| panic!("{err}"));
                setsum
            }
        };
    }

    #[macro_export]
    macro_rules! log_check {
        ($test_root:expr; $relative_path:literal: $($key:literal => $val:literal,)*) => {
            let log_path = PathBuf::from($test_root).join($relative_path);
            let mut log = sst::LogIterator::new(sst::LogOptions::default(), log_path).unwrap_or_else(|err| panic!("{err}"));
            $(
                let kvr = log.next().unwrap_or_else(|err| panic!("{err}")).expect("next should not be None");
                assert_eq!($key.as_bytes(), kvr.key);
                let value: &[u8] = $val.as_bytes();
                assert_eq!(Some(value), kvr.value);
            )*
            assert_eq!(None, log.next().unwrap_or_else(|err| panic!("{err}")));
        };
    }
}

#[cfg(test)]
mod tests {
    use mani::{Edit, Manifest};
    use sst::{Builder, Cursor};

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
        KeyValueStore::open(options).unwrap_or_else(|err| panic!("{err}"));
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
        assert!(
            TRASH_SST(
                &root,
                Setsum::from_hexdigest(
                    "fb93e8e143482d6eef570088782f6bee22e519dc17a4ef56347a65d5fddf7b6a"
                )
                .expect("valid setsum")
            )
            .exists()
        );
    }
    // TODO(rescrv): two log files
}
