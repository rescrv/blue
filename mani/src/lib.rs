use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fs::{create_dir, hard_link, metadata, remove_file, rename, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use arrrg_derive::CommandLine;

use biometrics::{Collector, Counter};

use tatl::{HeyListen, Stationary};

use utilz::lockfile::Lockfile;

use zerror::Z;
use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn LOCKFILE<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("LOCKFILE")
}

#[allow(non_snake_case)]
fn MANIFEST<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("MANIFEST")
}

#[allow(non_snake_case)]
fn TEMPORARY<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("MANIFEST.tmp")
}

#[allow(non_snake_case)]
fn BACKUP<P: AsRef<Path>>(root: P, idx: u64) -> PathBuf {
    root.as_ref().to_path_buf().join(format!("MANIFEST.{}", idx))
}

const TX_SEPARATOR: &str = "--------";

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_OBTAINED: Counter = Counter::new("mani.lock_obtained");

static LOCK_NOT_OBTAINED: Counter = Counter::new("mani.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("mani.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_biometrics(collector: Collector) {
    collector.register_counter(&LOCK_OBTAINED);
    collector.register_counter(&LOCK_NOT_OBTAINED);
}

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Error {
    SystemError {
        core: ErrorCore,
        what: String,
    },
    Corruption {
        core: ErrorCore,
        what: String,
    },
    NewlineDisallowed {
        core: ErrorCore,
        what: String,
    },
    DbExists {
        core: ErrorCore,
        path: PathBuf,
    },
    DbNotExist {
        core: ErrorCore,
        path: PathBuf,
    },
    LockNotObtained {
        core: ErrorCore,
        path: PathBuf,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::SystemError { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::NewlineDisallowed { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::SystemError { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::NewlineDisallowed { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        // TODO(rescrv): put a one-line error as first line.
        self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.core_mut().set_token(identifier, value);
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.core_mut().set_url(identifier, url);
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.core_mut().set_variable(variable, x);
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError { core: ErrorCore::default(), what: what.to_string() }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(what: std::str::Utf8Error) -> Error {
        Error::Corruption { core: ErrorCore::default(), what: "utf8 error:".to_owned() + &what.to_string() }
    }
}

//////////////////////////////////////////// MapIoError ////////////////////////////////////////////

pub trait MapIoError {
    type Result;

    fn map_io_err(self) -> Self::Result;
}

impl<T> MapIoError for Result<T, std::io::Error> {
    type Result = Result<T, Error>;

    fn map_io_err(self) -> Self::Result {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }
}

////////////////////////////////////////// ManifestOptions /////////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Eq, PartialEq)]
pub struct ManifestOptions {
    #[arrrg(flag, "Fail if the manifest directory exists.")]
    fail_if_exists: bool,
    #[arrrg(flag, "Fail if the manifest directory does not exist.")]
    fail_if_not_exist: bool,
    #[arrrg(flag, "Fail if the manifest is locked.")]
    fail_if_locked: bool,
    #[arrrg(optional, "Ratio of (bytes in the log):(bytes in memory) at which log will rollover.")]
    log_rollover_ratio: u64,
}

impl Default for ManifestOptions {
    fn default() -> Self {
        Self {
            fail_if_exists: false,
            fail_if_not_exist: false,
            fail_if_locked: false,
            log_rollover_ratio: 2,
        }
    }
}

///////////////////////////////////////////// Manifest /////////////////////////////////////////////

pub struct Manifest {
    options: ManifestOptions,
    _lockfile: Lockfile,
    root: PathBuf,
    strs: BTreeSet<String>,
    poison: Option<Error>,
}

impl Manifest {
    pub fn open<P: AsRef<Path>>(options: ManifestOptions, root: P) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        if root.is_dir() && options.fail_if_exists {
            return Err(Error::DbExists { core: ErrorCore::default(), path: root });
        }
        if !root.is_dir() && options.fail_if_not_exist {
            return Err(Error::DbNotExist { core: ErrorCore::default(), path: root });
        } else if !root.is_dir() {
            create_dir(&root)
                .map_io_err()
                .with_variable("root", root.to_string_lossy())?;
        }
        // Deal with the lockfile first.
        let lockfile = if options.fail_if_locked {
            Lockfile::lock(LOCKFILE(&root))
                .map_io_err()
                .with_variable("root", root.to_string_lossy())?
        } else {
            Lockfile::wait(LOCKFILE(&root))
                .map_io_err()
                .with_variable("root", root.to_string_lossy())?
        };
        match lockfile {
            Some(_lockfile) => {
                LOCK_OBTAINED.click();
                let strs = Self::read_strs(MANIFEST(&root))?;
                Ok(Self {
                    options,
                    _lockfile,
                    root,
                    strs,
                    poison: None,
                })
            },
            None => {
                LOCK_NOT_OBTAINED.click();
                let err = Error::LockNotObtained {
                    core: ErrorCore::default(),
                    path: LOCKFILE(root),
                };
                Err(err)
            },
        }
    }

    pub fn strs(&self) -> impl Iterator<Item=&String> {
        self.strs.iter()
    }

    pub fn size(&self) -> u64 {
        self.strs.iter().map(|s| s.len() as u64).sum()
    }

    pub fn apply(&mut self, edit: Edit) -> Result<(), Error> {
        self._apply(&MANIFEST(&self.root), edit, true)
    }

    pub fn rollover(&mut self) -> Result<(), Error> {
        let strs = self.poison(Self::read_strs(MANIFEST(&self.root)))?;
        let mut edit = Edit::default();
        for s in strs {
            self.poison(edit.add(&s))?;
        }
        for idx in 0..u64::max_value() {
            let back = BACKUP(&self.root, idx);
            if !back.exists() {
                self.poison(hard_link(MANIFEST(&self.root), back))?;
                break;
            }
        }
        let tmp = TEMPORARY(&self.root);
        if tmp.exists() {
            self.poison(remove_file(&tmp))?
        }
        self._apply(&tmp, edit, false)?;
        self.poison(rename(&tmp, MANIFEST(&self.root)))?;
        Ok(())
    }

    fn _apply(&mut self, output: &PathBuf, edit: Edit, allow_rollover: bool) -> Result<(), Error> {
        let mut edit_str = String::new();
        for path in edit.rm_strs.iter() {
            self.strs.remove(&String::from(path));
            let line = "-".to_owned() + path;
            let cksum = crc32c::crc32c(line.as_bytes());
            edit_str += &format!("{:08x}{}\n", cksum, line);
        }
        for path in edit.add_strs.iter() {
            self.strs.insert(String::from(path));
            let line = "+".to_owned() + path;
            let cksum = crc32c::crc32c(line.as_bytes());
            edit_str += &format!("{:08x}{}\n", cksum, line);
        }
        edit_str += TX_SEPARATOR;
        edit_str += "\n";
        let mut fout = OpenOptions::new().create(true).append(true).open(output)?;
        self.poison(fout.write_all(edit_str.as_bytes()))?;
        self.poison(fout.flush())?;
        self.poison(fout.sync_data())?;
        if allow_rollover && self.poison(metadata(output))?.len() > self.options.log_rollover_ratio * self.size() {
            self.rollover()?;
        }
        Ok(())
    }

    fn poison<T, E>(&mut self, res: Result<T, E>) -> Result<T, Error>
    where
        Error: From<E>,
    {
        match res {
            Ok(t) => Ok(t),
            Err(e) => {
                if self.poison.is_none() {
                    self.poison = Some(e.into());
                }
                Err(self.poison.as_ref().unwrap().clone())
            },
        }
    }

    fn read_strs(path: PathBuf) -> Result<BTreeSet<String>, Error> {
        if path.is_dir() {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                what: "MANIFEST file is a directory".to_owned(),
            });
        }
        if !path.is_file() {
            return Ok(BTreeSet::new());
        }
        let file = File::open(&path).map_io_err().with_variable("path", path.to_string_lossy())?;
        let file = BufReader::new(file);
        let mut paths = BTreeSet::new();
        for (idx, line) in file.lines().enumerate() {
            let line = line?;
            if !line.is_ascii() {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    what: format!("line {} is not ascii", idx),
                });
            }
            if line == TX_SEPARATOR {
            } else if line.len() > 9 {
                let crc32c_expected = u32::from_str_radix(&line[..8], 16)
                    .map_err(|err| {
                        Error::Corruption {
                            core: ErrorCore::default(),
                            what: format!("crc32c is not hex on line {}: {}", idx, err),
                        }
                    })?;
                if crc32c::crc32c(&line.as_bytes()[8..]) != crc32c_expected {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        what: format!("crc32c failure on line {}", idx),
                    });
                }
                let action = line.as_bytes()[8] as char;
                if action == '+' {
                    paths.insert(String::from(&line[9..]));
                } else if action == '-' {
                    paths.remove(&String::from(&line[9..]));
                } else {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        what: format!("operation {} is not supported", action),
                    });
                }
            } else {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    what: format!("unhandled case on line {}", idx),
                });
            }
        }
        Ok(paths)
    }
}

/////////////////////////////////////////////// Edit ///////////////////////////////////////////////

#[derive(Debug, Default)]
pub struct Edit {
    add_strs: BTreeSet<String>,
    rm_strs: BTreeSet<String>,
}

impl Edit {
    pub fn add(&mut self, s: &str) -> Result<(), Error> {
        let s = Self::check_str(s)?;
        self.add_strs.insert(s);
        Ok(())
    }

    pub fn rm(&mut self, s: &str) -> Result<(), Error> {
        let s = Self::check_str(s)?;
        self.rm_strs.insert(s);
        Ok(())
    }

    fn check_str(s: &str) -> Result<String, Error> {
        if s.chars().any(|c| c == '\n') {
            Err(Error::NewlineDisallowed {
                core: ErrorCore::default(),
                what: "added strings must not contain newlines".to_owned(),
            })
        } else {
            Ok(s.to_owned())
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::{read_to_string, remove_dir_all};

    use super::*;

    fn test_root(root: &str, line: u32) -> PathBuf {
        let root: String = root.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        let path = PathBuf::from(format!("{}_{}", root, line));
        if path.exists() {
            remove_dir_all(&path).expect("could not prepare for test");
        }
        path
    }

    #[test]
    fn lockfile_const() {
        assert_eq!("/path/to/LOCKFILE", LOCKFILE("/path/to").to_string_lossy());
        assert_eq!("/path/to/LOCKFILE", LOCKFILE("/path/to/").to_string_lossy());
    }

    #[test]
    fn test_test_root() {
        let line = line!();
        let root = test_root(module_path!(), line);
        assert_eq!(PathBuf::from(format!("mani__tests_{}", line)), root);
    }

    #[test]
    fn not_exist_defaults() {
        let root = test_root(module_path!(), line!());
        let opts = ManifestOptions::default();
        let _mani = Manifest::open(opts, &root).unwrap();
        assert!(!root.join("MANIFEST").exists());
    }

    #[test]
    fn not_exist_fail_if_not_exist() {
        let root = test_root(module_path!(), line!());
        let mut opts = ManifestOptions::default();
        opts.fail_if_not_exist = true;
        if let Err(Error::DbNotExist { .. }) = Manifest::open(opts, &root) {
        } else {
            panic!("bad case");
        }
    }

    #[test]
    fn not_exist_fail_if_exists() {
        let root = test_root(module_path!(), line!());
        let mut opts = ManifestOptions::default();
        let mut _mani = Manifest::open(opts.clone(), &root);
        opts.fail_if_exists = true;
        if let Err(Error::DbExists { .. }) = Manifest::open(opts, &root) {
        } else {
            panic!("bad case");
        }
    }

    #[test]
    fn simple_addition() {
        let root = test_root(module_path!(), line!());
        let opts = ManifestOptions::default();
        let mut mani = Manifest::open(opts.clone(), &root).unwrap();
        let mut edit = Edit::default();
        edit.add("thing one").unwrap();
        edit.add("thing two").unwrap();
        mani.apply(edit).unwrap();
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
--------
", read_to_string(root.join("MANIFEST")).unwrap());
    }

    #[test]
    fn removal() {
        let root = test_root(module_path!(), line!());
        let opts = ManifestOptions::default();
        let mut mani = Manifest::open(opts.clone(), &root).unwrap();
        let mut edit = Edit::default();
        edit.add("thing one").unwrap();
        edit.add("thing two").unwrap();
        mani.apply(edit).unwrap();
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
--------
", read_to_string(root.join("MANIFEST")).unwrap());
        let mut edit = Edit::default();
        edit.rm("thing one").unwrap();
        mani.apply(edit).unwrap();
        assert_eq!("a4e79c62+thing two
--------
", read_to_string(root.join("MANIFEST")).unwrap());
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
--------
", read_to_string(root.join("MANIFEST.0")).unwrap());
    }
}