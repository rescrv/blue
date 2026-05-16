#![doc = include_str!("../README.md")]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use utf8path::Path;

const MAX_RETRIES: usize = 1024;

static LAST_MICROS: AtomicU64 = AtomicU64::new(0);

/// A Maildir-like queue rooted at one directory.
#[derive(Clone, Debug)]
pub struct Maildir2026 {
    root: Path<'static>,
    hostname: String,
    pid: u32,
}

impl Maildir2026 {
    /// Open or create a Maildir-like directory rooted at `root`.
    pub fn open<'a>(root: impl Into<Path<'a>>) -> Result<Self, std::io::Error> {
        let root = root.into().into_owned();
        std::fs::create_dir_all(root.join("tmp"))?;
        std::fs::create_dir_all(root.join("new"))?;
        std::fs::create_dir_all(root.join("cur"))?;
        let hostname = hostname()?;
        let pid = std::process::id();
        Ok(Self {
            root,
            hostname,
            pid,
        })
    }

    /// Return the root path.
    pub fn root(&self) -> &Path<'static> {
        &self.root
    }

    /// Write `bytes` to `tmp` and publish them by moving the completed file to `new`.
    pub fn write<B: AsRef<[u8]>>(&self, bytes: B) -> Result<Path<'static>, std::io::Error> {
        let bytes = bytes.as_ref();
        for _ in 0..MAX_RETRIES {
            let name = self.filename()?;
            let tmp_path = self.tmp_path().join(&name).into_owned();
            let new_path = self.new_path().join(&name).into_owned();
            let cur_path = self.cur_path().join(&name).into_owned();
            if new_path.exists()? || cur_path.exists()? {
                continue;
            }
            let mut file = match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
            {
                Ok(file) => file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err),
            };
            if let Err(err) = file.write_all(bytes) {
                let _ = std::fs::remove_file(&tmp_path);
                return Err(err);
            }
            if let Err(err) = file.sync_all() {
                let _ = std::fs::remove_file(&tmp_path);
                return Err(err);
            }
            drop(file);
            if new_path.exists()? || cur_path.exists()? {
                let _ = std::fs::remove_file(&tmp_path);
                continue;
            }
            match std::fs::rename(&tmp_path, &new_path) {
                Ok(()) => return Ok(new_path),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let _ = std::fs::remove_file(&tmp_path);
                    continue;
                }
                Err(err) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    return Err(err);
                }
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not generate a unique maildir2026 filename",
        ))
    }

    /// Claim one file by moving it from `new` to `cur`.
    pub fn claim_one(&self) -> Result<Option<ClaimedFile>, std::io::Error> {
        let mut candidates = Vec::new();
        for entry in std::fs::read_dir(self.new_path())? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = Path::try_from(entry.path()).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "path is not UTF-8")
                })?;
                candidates.push(path.into_owned());
            }
        }
        candidates.sort();
        for new_path in candidates {
            let file_name = new_path.basename();
            let cur_path = self.cur_path().join(file_name.as_str()).into_owned();
            // NOTE(rescrv):  We test this in order so sequencing guarantees we see cur_path before
            // new_path.
            let cur_path_exists = cur_path.exists()?;
            if cur_path_exists && new_path.exists()? {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("claimed file path already exists: {cur_path}"),
                ));
            } else if cur_path_exists {
                continue;
            }
            match std::fs::rename(&new_path, &cur_path) {
                Ok(()) => return Ok(Some(ClaimedFile { path: cur_path })),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(None)
    }

    fn filename(&self) -> Result<String, std::io::Error> {
        let micros = next_micros()?;
        Ok(format!("{}.{}.{}", self.hostname, micros, self.pid))
    }

    fn tmp_path(&self) -> Path<'static> {
        self.root.join("tmp").into_owned()
    }

    fn new_path(&self) -> Path<'static> {
        self.root.join("new").into_owned()
    }

    fn cur_path(&self) -> Path<'static> {
        self.root.join("cur").into_owned()
    }
}

/// A file claimed from `new` and moved into `cur`.
#[derive(Clone, Debug)]
pub struct ClaimedFile {
    path: Path<'static>,
}

impl ClaimedFile {
    /// Return the path of the claimed file in `cur`.
    pub fn path(&self) -> &Path<'static> {
        &self.path
    }

    /// Read the entire claimed file.
    pub fn read_to_end(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut file = File::open(&self.path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Remove the claimed file.
    pub fn remove(self) -> Result<(), std::io::Error> {
        std::fs::remove_file(self.path)
    }
}

fn hostname() -> Result<String, std::io::Error> {
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let len = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    let hostname = String::from_utf8_lossy(&buf[..len]);
    let hostname = sanitize_hostname(&hostname);
    if hostname.is_empty() {
        Ok("unknown".to_string())
    } else {
        Ok(hostname)
    }
}

fn sanitize_hostname(hostname: &str) -> String {
    hostname
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn next_micros() -> Result<u64, std::io::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_micros();
    if now > u64::MAX as u128 {
        return Err(std::io::Error::other("time exceeds u64 micros"));
    }
    let now = now as u64;
    let mut observed = LAST_MICROS.load(Ordering::Relaxed);
    loop {
        let candidate = std::cmp::max(now, observed.saturating_add(1));
        match LAST_MICROS.compare_exchange(
            observed,
            candidate,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(candidate),
            Err(actual) => observed = actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct TempRoot {
        path: Path<'static>,
    }

    impl TempRoot {
        fn new(name: &str) -> Self {
            let path = Path::try_from(std::env::temp_dir())
                .unwrap()
                .join(format!(
                    "maildir2026-test-{}-{}-{}",
                    name,
                    std::process::id(),
                    next_micros().unwrap()
                ))
                .into_owned();
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn count_files(path: &Path<'_>) -> usize {
        std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| entry.file_type().unwrap().is_file())
            .count()
    }

    #[test]
    fn open_creates_directories() {
        let root = TempRoot::new("open-creates-directories");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        assert_eq!(&root.path, maildir.root());
        assert!(root.path.join("tmp").is_dir().unwrap());
        assert!(root.path.join("new").is_dir().unwrap());
        assert!(root.path.join("cur").is_dir().unwrap());
    }

    #[test]
    fn write_publishes_to_new() {
        let root = TempRoot::new("write-publishes-to-new");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        let path = maildir.write(b"hello").unwrap();
        assert_eq!(root.path.join("new"), path.dirname());
        assert_eq!(0, count_files(&root.path.join("tmp")));
        assert_eq!(1, count_files(&root.path.join("new")));
    }

    #[test]
    fn claim_moves_to_cur_and_reads() {
        let root = TempRoot::new("claim-moves-to-cur-and-reads");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        maildir.write(b"hello").unwrap();
        let claimed = maildir.claim_one().unwrap().unwrap();
        assert_eq!(root.path.join("cur"), claimed.path().dirname());
        assert_eq!(b"hello", claimed.read_to_end().unwrap().as_slice());
        assert_eq!(0, count_files(&root.path.join("new")));
        assert_eq!(1, count_files(&root.path.join("cur")));
    }

    #[test]
    fn remove_claimed_file() {
        let root = TempRoot::new("remove-claimed-file");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        maildir.write(b"hello").unwrap();
        let claimed = maildir.claim_one().unwrap().unwrap();
        claimed.remove().unwrap();
        assert_eq!(0, count_files(&root.path.join("cur")));
    }

    #[test]
    fn multiple_writes_have_unique_names() {
        let root = TempRoot::new("multiple-writes-have-unique-names");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        let mut names = HashSet::new();
        for _ in 0..64 {
            let path = maildir.write(b"hello").unwrap();
            let name = path.basename().into_owned();
            assert!(names.insert(name));
        }
        assert_eq!(64, count_files(&root.path.join("new")));
    }

    #[test]
    fn empty_claim_returns_none() {
        let root = TempRoot::new("empty-claim-returns-none");
        let maildir = Maildir2026::open(root.path.clone()).unwrap();
        assert!(maildir.claim_one().unwrap().is_none());
    }
}
