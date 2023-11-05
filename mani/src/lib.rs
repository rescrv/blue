#![doc = include_str!("../README.md")]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::fs::{create_dir, hard_link, metadata, read_dir, remove_file, rename, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use arrrg_derive::CommandLine;

use biometrics::{Collector, Counter};

use prototk_derive::Message;

use tatl::{HeyListen, Stationary};

use utilz::lockfile::Lockfile;

use zerror::{iotoz, Z};
use zerror_core::ErrorCore;
use zerror_derive::ZerrorCore;

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

/// Error for the manifest.
#[derive(Clone, Message, ZerrorCore)]
pub enum Error {
    #[prototk(376832, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(376833, message)]
    SystemError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(376834, message)]
    Corruption {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(376835, message)]
    NewlineDisallowed {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(376836, message)]
    ManifestExists {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        path: PathBuf,
    },
    #[prototk(376837, message)]
    ManifestNotExist {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        path: PathBuf,
    },
    #[prototk(376838, message)]
    LockNotObtained {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        path: PathBuf,
    },
}

impl Default for Error {
    fn default() -> Error {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

iotoz!{Error}

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

////////////////////////////////////////// ManifestOptions /////////////////////////////////////////

/// [ManifestOptions] provides the options for commandline programs.
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

/// Manifest provies an append-driven log.
pub struct Manifest {
    options: ManifestOptions,
    _lockfile: Lockfile,
    root: PathBuf,
    strs: BTreeSet<String>,
    info: BTreeMap<char, String>,
    last_rollover: u64,
    poison: Option<Error>,
}

impl Manifest {
    /// Open a new manifest.
    pub fn open<P: AsRef<Path>>(options: ManifestOptions, root: P) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        if root.is_dir() && options.fail_if_exists {
            return Err(Error::ManifestExists { core: ErrorCore::default(), path: root });
        }
        if !root.is_dir() && options.fail_if_not_exist {
            return Err(Error::ManifestNotExist { core: ErrorCore::default(), path: root });
        } else if !root.is_dir() {
            create_dir(&root)
                .as_z()
                .with_variable("root", root.to_string_lossy())?;
        }
        // Deal with the lockfile first.
        let lockfile = if options.fail_if_locked {
            Lockfile::lock(LOCKFILE(&root))
                .as_z()
                .with_variable("root", root.to_string_lossy())?
        } else {
            Lockfile::wait(LOCKFILE(&root))
                .as_z()
                .with_variable("root", root.to_string_lossy())?
        };
        match lockfile {
            Some(_lockfile) => {
                LOCK_OBTAINED.click();
                let (strs, info) = Self::read_mani(MANIFEST(&root))?;
                let last_rollover = Self::next_manifest_identifier(&root)?;
                Ok(Self {
                    options,
                    _lockfile,
                    root,
                    strs,
                    info,
                    last_rollover,
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

    /// Iterate over the log's contents (in-memory).
    pub fn strs(&self) -> impl Iterator<Item=&String> {
        self.strs.iter()
    }

    /// Number of bytes used for this log.
    pub fn size(&self) -> u64 {
        let strs: u64 = self.strs.iter().map(|s| s.len() as u64).sum();
        let info: u64 = self.info.iter().map(|(_, s)| s.len() as u64).sum();
        strs + info
    }

    /// Apply an edit to the log.
    pub fn apply(&mut self, edit: Edit) -> Result<(), Error> {
        self._apply(&MANIFEST(&self.root), edit, true)
    }

    /// Rollover the log.
    pub fn rollover(&mut self) -> Result<(), Error> {
        let mut edit = Edit::default();
        for s in self.strs.iter() {
            edit.add(&s).expect("previously added string should always add");
        }
        for (c, s) in self.info.iter() {
            edit.info(*c, &s).expect("previously added info should always add");
        }
        let next_id = self.last_rollover;
        self.last_rollover += 1;
        let back = BACKUP(&self.root, next_id);
        self.poison(hard_link(MANIFEST(&self.root), back))?;
        let tmp = TEMPORARY(&self.root);
        if tmp.exists() {
            self.poison(remove_file(&tmp))?
        }
        self._apply(&tmp, edit, false)?;
        self.poison(rename(&tmp, MANIFEST(&self.root)))?;
        Ok(())
    }

    fn _apply(&mut self, output: &PathBuf, edit: Edit, allow_rollover: bool) -> Result<(), Error> {
        let was_empty = self.strs.is_empty();
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
        for (key, value) in edit.info.iter() {
            self.info.insert(*key, value.clone());
            let line = format!("{}{}", key, value);
            let cksum = crc32c::crc32c(line.as_bytes());
            edit_str += &format!("{:08x}{}\n", cksum, line);
        }
        edit_str += TX_SEPARATOR;
        edit_str += "\n";
        let mut fout = OpenOptions::new().create(true).append(true).open(output)?;
        self.poison(fout.write_all(edit_str.as_bytes()))?;
        self.poison(fout.flush())?;
        self.poison(fout.sync_data())?;
        if allow_rollover {
            let on_disk_bytes = self.poison(metadata(output))?.len();
            let in_memory_bytes = self.size();
            if on_disk_bytes > self.options.log_rollover_ratio * in_memory_bytes && !was_empty {
                self.rollover()?;
            }
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

    fn read_mani(path: PathBuf) -> Result<(BTreeSet<String>, BTreeMap<char, String>), Error> {
        let mut strs = BTreeSet::new();
        let mut info = BTreeMap::new();
        let iter = ManifestIterator::open(path)?;
        for edit in iter {
            let edit = edit?;
            for s in edit.rm_strs.iter() {
                strs.remove(s);
            }
            for s in edit.add_strs.iter() {
                strs.insert(s.clone());
            }
            for (c, s) in edit.info.iter() {
                info.insert(*c, s.clone());
            }
        }
        Ok((strs, info))
    }

    fn next_manifest_identifier<P: AsRef<Path>>(root: P) -> Result<u64, Error> {
        let mut max_next_id = 0;
        for dir in read_dir(root.as_ref())? {
            let dir = dir?;
            const MANIFEST: &'static str = "MANIFEST.";
            let path = dir.path();
            let path = path.as_os_str().to_str();
            let path = match path {
                Some(path) => path,
                None => { continue },
            };
            if path.starts_with(MANIFEST) {
                let next_id = match path[MANIFEST.len()..].parse::<u64>() {
                    Ok(next_id) => next_id,
                    Err(_) => { continue },
                };
                max_next_id = std::cmp::max(max_next_id, next_id);
            }
        }
        Ok(max_next_id + 1)
    }
}

/////////////////////////////////////////////// Edit ///////////////////////////////////////////////

/// An edit adds some strings and removes others.
#[derive(Debug, Default)]
pub struct Edit {
    add_strs: BTreeSet<String>,
    rm_strs: BTreeSet<String>,
    info: BTreeMap<char, String>,
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

    pub fn info(&mut self, c: char, s: &str) -> Result<(), Error> {
        Self::check_str(&c.to_string())?;
        let s = Self::check_str(s)?;
        self.info.insert(c, s);
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

///////////////////////////////////////// ManifestIterator /////////////////////////////////////////

pub struct ManifestIterator {
    file: Option<BufReader<File>>,
    poison: Option<Error>,
}

impl ManifestIterator {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        if path.as_ref().is_dir() {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                what: "MANIFEST file is a directory".to_owned(),
            });
        }
        if !path.as_ref().is_file() {
            return Ok(Self {
                file: None,
                poison: None,
            });
        }
        let file = Some(BufReader::new(File::open(path)?));
        Ok(Self {
            file,
            poison: None,
        })
    }

    fn poison<E: Into<Error>>(&mut self, err: E) -> Option<Result<Edit, Error>> {
        let err = err.into();
        self.poison = Some(err.clone());
        self.file = None;
        Some(Err(err))
    }
}

impl Iterator for ManifestIterator {
    type Item = Result<Edit, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = match &mut self.file {
            Some(file) => file,
            None => { return None; },
        };
        let mut edit = Edit::default();
        for (idx, line) in file.lines().enumerate() {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    return self.poison(err);
                },
            };
            if !line.is_ascii() {
                return Some(Err(Error::Corruption {
                    core: ErrorCore::default(),
                    what: format!("line {} is not ascii", idx),
                }));
            }
            if line == TX_SEPARATOR {
                return Some(Ok(edit));
            } else if line.len() > 9 {
                let crc32c_expected = match u32::from_str_radix(&line[..8], 16) {
                    Ok(crc32c_expected) => crc32c_expected,
                    Err(err) => {
                        return self.poison(Error::Corruption {
                            core: ErrorCore::default(),
                            what: format!("crc32c is not hex on line {}: {}", idx, err),
                        });
                    }
                };
                if crc32c::crc32c(&line.as_bytes()[8..]) != crc32c_expected {
                    return self.poison(Error::Corruption {
                        core: ErrorCore::default(),
                        what: format!("crc32c failure on line {}", idx),
                    });
                }
                let action = line.as_bytes()[8] as char;
                if action == '+' {
                    if let Err(err) = edit.add(&line[9..]) {
                        return self.poison(err);
                    }
                } else if action == '-' {
                    if let Err(err) = edit.rm(&line[9..]) {
                        return self.poison(err);
                    }
                } else if action == '\n' {
                    return self.poison(Error::Corruption {
                        core: ErrorCore::default(),
                        what: "operation \\n is not supported".to_owned(),
                    });
                } else {
                    if let Err(err) = edit.info(action, &line[9..]) {
                        return self.poison(err);
                    }
                }
            } else {
                return self.poison(Error::Corruption {
                    core: ErrorCore::default(),
                    what: format!("unhandled case on line {}", idx),
                });
            }
        }
        self.file = None;
        None
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
        if let Err(Error::ManifestNotExist { .. }) = Manifest::open(opts, &root) {
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
        if let Err(Error::ManifestExists { .. }) = Manifest::open(opts, &root) {
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
        assert!(root.join("MANIFEST").exists());
        assert!(root.join("MANIFEST.1").exists());
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
--------
6c866914-thing one
--------
", read_to_string(root.join("MANIFEST.1")).unwrap());
    }

    #[test]
    fn info() {
        let root = test_root(module_path!(), line!());
        let opts = ManifestOptions::default();
        let mut mani = Manifest::open(opts.clone(), &root).unwrap();
        let mut edit = Edit::default();
        edit.add("thing one").unwrap();
        edit.add("thing two").unwrap();
        // We want to record the following for the target use case of mani:
        // C:  A unique client identifier string.  Used for debugging who wrote what.
        // A:  The setsum over the added files.
        // R:  The setsum over the removed files.
        // T:  The setsum of trash removed.
        // S:  The setsum that covers the set of strings after the edit is applied.
        //
        // NOTE(rescrv):  Lacking setsum tooling I just made up a value for each of the following.
        // It doesn't get verified by mani, only the tooling built on top that can read information
        // knows about these setsums.
        edit.info('C', "some-client-identifier").unwrap();
        edit.info('A', "71332261daaa6dc30ad627b09349c6af").unwrap();
        edit.info('R', "00000000000000000000000000000000").unwrap();
        edit.info('T', "e0785f2a185aaf6fe0a099bc98ce1e70").unwrap();
        edit.info('S', "9ba1e4d7aa7a39a91b00d90c36436414").unwrap();
        mani.apply(edit).unwrap();
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
4d82ac08A71332261daaa6dc30ad627b09349c6af
a2281ab8Csome-client-identifier
442a0186R00000000000000000000000000000000
1736a268S9ba1e4d7aa7a39a91b00d90c36436414
79810703Te0785f2a185aaf6fe0a099bc98ce1e70
--------
", read_to_string(root.join("MANIFEST")).unwrap());
    }

    #[test]
    fn iterator() {
        let root = test_root(module_path!(), line!());
        let opts = ManifestOptions::default();
        let mut mani = Manifest::open(opts.clone(), &root).unwrap();
        let mut edit = Edit::default();
        edit.add("thing one").unwrap();
        edit.info('1', "thing one metadata").unwrap();
        mani.apply(edit).unwrap();
        let mut edit = Edit::default();
        edit.add("thing two").unwrap();
        edit.info('2', "thing two metadata").unwrap();
        mani.apply(edit).unwrap();
        assert_eq!("dcab9d28+thing one
a4e79c62+thing two
05a03b0d1thing one metadata
bc9dae362thing two metadata
--------
", read_to_string(root.join("MANIFEST")).unwrap());
        assert_eq!("dcab9d28+thing one
05a03b0d1thing one metadata
--------
a4e79c62+thing two
bc9dae362thing two metadata
--------
", read_to_string(root.join("MANIFEST.1")).unwrap());
        // Now iterate that we know the logs are good.
        let mut iter = ManifestIterator::open(root.join("MANIFEST.1")).unwrap();
        // first record
        let edit = iter.next().unwrap().unwrap();
        assert_eq!(1, edit.add_strs.len());
        assert!(edit.add_strs.contains("thing one"));
        assert_eq!(0, edit.rm_strs.len());
        assert_eq!(1, edit.info.len());
        assert_eq!(Some("thing one metadata"), edit.info.get(&'1').map(|s| s.as_str()));
        // second record
        let edit = iter.next().unwrap().unwrap();
        assert_eq!(1, edit.add_strs.len());
        assert!(edit.add_strs.contains("thing two"));
        assert_eq!(0, edit.rm_strs.len());
        assert_eq!(1, edit.info.len());
        assert_eq!(Some("thing two metadata"), edit.info.get(&'2').map(|s| s.as_str()));
        // no record
        assert!(iter.next().is_none());
    }
}
