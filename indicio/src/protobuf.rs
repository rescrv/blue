use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use buffertk::stack_pack;

use super::*;

///////////////////////////////////////////// ClueFrame ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct ClueFrame {
    #[prototk(1, message)]
    pub clue: Clue,
}

//////////////////////////////////////////// ClueVector ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, prototk_derive::Message)]
pub struct ClueVector {
    #[prototk(1, message)]
    pub clues: Vec<Clue>,
}

////////////////////////////////////////// ProtobufEmitter /////////////////////////////////////////

struct OutputState {
    buffer: Vec<u8>,
    file: Option<File>,
    size: u64,
    timestamp: u64,
    file_timestamp: u128,
}

/// An Emitter that writes key-value pairs to a series of log files.  When the file reaches its
/// size threshold, it rolls over to the next file.
pub struct ProtobufEmitter {
    prefix: PathBuf,
    target: u64,
    state: Mutex<OutputState>,
}

impl ProtobufEmitter {
    pub fn new<P: AsRef<Path>>(prefix: P, target: u64) -> Result<Self, std::io::Error> {
        if target == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "indicio protobuf emitter target must be greater than zero",
            ));
        }
        let prefix = prefix.as_ref().to_path_buf();
        let state = Mutex::new(OutputState {
            buffer: vec![],
            file: None,
            size: 0,
            timestamp: 0,
            file_timestamp: 0,
        });
        Ok(Self {
            prefix,
            target,
            state,
        })
    }

    fn path_for_timestamp(&self, timestamp: u128) -> PathBuf {
        let mut path = OsString::from(self.prefix.as_os_str());
        let ext = OsString::from(format!(".{timestamp}"));
        path.push(ext);
        PathBuf::from(path)
    }

    fn now_micros() -> u128 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_micros())
            .unwrap_or(0)
    }

    fn open(&self, state: &mut OutputState) -> Result<(), std::io::Error> {
        let mut timestamp = std::cmp::max(Self::now_micros(), state.file_timestamp + 1);
        for _ in 0..1024 {
            let path = self.path_for_timestamp(timestamp);
            match OpenOptions::new().create_new(true).write(true).open(path) {
                Ok(file) => {
                    state.file = Some(file);
                    state.size = 0;
                    state.file_timestamp = timestamp;
                    return Ok(());
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    timestamp += 1;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Err(Error::new(
            ErrorKind::AlreadyExists,
            "could not create a unique indicio protobuf log file",
        ))
    }

    fn close(&self, state: &mut OutputState) {
        if let Some(file) = state.file.as_mut() {
            let _ = file.flush();
        }
        state.file = None;
        state.size = 0;
    }

    fn drain(&self, state: &mut OutputState) {
        if state.buffer.is_empty() {
            return;
        }
        for _ in 0..3 {
            if state.file.is_none() && self.open(state).is_err() {
                EMITTER_FAILURE.click();
                return;
            }
            let buffer_len = state.buffer.len() as u64;
            if state.size > 0 && state.size.saturating_add(buffer_len) > self.target {
                self.close(state);
                continue;
            }
            let rollback_size = state.size;
            if let Some(file) = state.file.as_mut() {
                if rollback_size >= self.target {
                    self.close(state);
                    continue;
                }
                if file.write_all(&state.buffer).is_err() {
                    let _ = file.set_len(rollback_size);
                    self.close(state);
                    continue;
                }
                state.size = state.size.saturating_add(buffer_len);
                state.buffer.clear();
                return;
            }
        }
        EMITTER_FAILURE.click();
        self.close(state);
    }
}

impl Emitter for ProtobufEmitter {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_micros() as u64)
            .unwrap_or(0);
        let mut frame = ClueFrame {
            clue: Clue {
                // TODO(rescrv): make prototk support Cow::Borrowed so we can eliminate this.
                file: file.to_string(),
                line,
                level,
                timestamp,
                // TODO(rescrv): and this (so that we can take value by ref).
                value,
            },
        };
        let mut state = self.state.lock().unwrap();
        frame.clue.timestamp =
            std::cmp::max(state.timestamp.saturating_add(1), frame.clue.timestamp);
        state.timestamp = frame.clue.timestamp;
        stack_pack(&frame).append_to_vec(&mut state.buffer);
        if state.buffer.len() > 1 << 16 {
            self.drain(&mut state);
        }
    }

    fn flush(&self) {
        let mut state = self.state.lock().unwrap();
        self.drain(&mut state);
        if let Some(file) = state.file.as_mut() {
            let _ = file.flush();
        }
    }
}

impl Drop for ProtobufEmitter {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            self.drain(&mut state);
            self.close(&mut state);
        }
    }
}
