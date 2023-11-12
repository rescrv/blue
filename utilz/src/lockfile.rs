//! Support for unix file locking, with some concurrency protection.

use std::fs::{File, OpenOptions};
use std::io::Error;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::Mutex;

#[allow(non_camel_case_types)]
type dev_t = u64;
#[allow(non_camel_case_types)]
type ino_t = u64;

static ACTIVELY_LOCKING: Mutex<Vec<(dev_t, ino_t)>> = Mutex::new(Vec::new());

///////////////////////////////////////////// Lockfile /////////////////////////////////////////////

/// Lock a local file on a local filesystem.
pub struct Lockfile {
    file: Option<File>,
    dev: dev_t,
    ino: ino_t,
}

impl Lockfile {
    /// Try to acquire a lock, but don't block to wait for one.
    pub fn lock<P: AsRef<Path>>(path: P) -> Result<Option<Self>, Error> {
        Lockfile::_lock(path, libc::F_SETLK)
    }

    /// Block to wait for the [Lockfile].
    pub fn wait<P: AsRef<Path>>(path: P) -> Result<Option<Self>, Error> {
        Lockfile::_lock(path, libc::F_SETLKW)
    }

    fn _lock<P: AsRef<Path>>(path: P, what: libc::c_int) -> Result<Option<Self>, Error> {
        // Hold ACTIVELY_LOCKING during the entire lock protocol.
        let mut lock_table = ACTIVELY_LOCKING.lock().unwrap();
        // Open the lock.  It doesn't matter if the lock file already exists.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .open(path)?;
        // Use the metadata to see if this process already holds a lock.
        let metadata = file.metadata()?;
        for (dev, ino) in lock_table.iter() {
            if *dev == metadata.dev() && *ino == metadata.ino() {
                return Ok(None);
            }
        }
        // NOTE(rescrv): l_type,l_whence is 16 bits on some platforms and 32 bits on others.
        // The annotations here are for cross-platform compatibility.
        #[allow(clippy::useless_conversion)]
        #[allow(clippy::unnecessary_cast)]
        let flock = libc::flock {
            l_type: libc::F_WRLCK as i16,
            l_whence: libc::SEEK_SET as i16,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
            #[cfg(target_os = "freebsd")]
            l_sysid: 0,
        };
        loop {
            if unsafe { libc::fcntl(file.as_raw_fd(), what, &flock) < 0 } {
                let err = std::io::Error::last_os_error();
                if let Some(raw) = err.raw_os_error() {
                    match raw {
                        libc::EAGAIN => {
                            return Ok(None);
                        }
                        libc::EINTR => {
                            continue;
                        }
                        _ => {}
                    }
                }
                return Err(err);
            } else {
                break;
            }
        }
        lock_table.push((metadata.dev(), metadata.ino()));
        Ok(Some(Lockfile {
            file: Some(file),
            dev: metadata.dev(),
            ino: metadata.ino(),
        }))
    }

    /// Unlock the lockfile, allowing the next-waiting file to take it.
    pub fn unlock(&mut self) {
        let mut lock_table = ACTIVELY_LOCKING.lock().unwrap();
        for idx in 0..lock_table.len() {
            if lock_table[idx].0 == self.dev && lock_table[idx].1 == self.ino {
                lock_table.swap_remove(idx);
                break;
            }
        }
        self.file = None;
        self.dev = 0;
        self.ino = 0;
    }
}

impl Drop for Lockfile {
    fn drop(&mut self) {
        self.unlock();
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let lockfile = Lockfile::lock("LOCKFILE.basics").expect("no error expected");
        if !lockfile.is_some() {
            panic!("should have returned a valid lock");
        }
        drop(lockfile);
    }

    #[test]
    fn cannot_lock_twice() {
        let lockfile1 = Lockfile::lock("LOCKFILE.cannot_lock_twice").expect("no error expected");
        let lockfile2 = Lockfile::lock("LOCKFILE.cannot_lock_twice").expect("no error expected");
        if !lockfile1.is_some() {
            panic!("first lock should have succeeded");
        }
        if !lockfile2.is_none() {
            panic!("second lock should have failed");
        }
    }

    #[test]
    fn wait() {
        let lockfile = Lockfile::wait("LOCKFILE.wait").expect("no error expected");
        if !lockfile.is_some() {
            panic!("should have returned a valid lock");
        }
        drop(lockfile);
    }
}
