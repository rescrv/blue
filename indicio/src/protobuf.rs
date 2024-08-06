use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use buffertk::stack_pack;

use super::*;

///////////////////////////////////////////// ClueFrame ////////////////////////////////////////////

#[derive(Default, prototk_derive::Message)]
pub struct ClueFrame {
    #[prototk(1, message)]
    pub clue: Clue,
}

//////////////////////////////////////////// ClueVector ////////////////////////////////////////////

#[derive(Default, prototk_derive::Message)]
pub struct ClueVector {
    #[prototk(1, message)]
    pub clues: Vec<Clue>,
}

////////////////////////////////////////// ProtobufEmitter /////////////////////////////////////////

struct ProtobufOutputState {
    buffer: Vec<u8>,
    file: Option<File>,
    size: u64,
    timestamp: u64,
}

/// An Emitter that writes key-value pairs to a series of log files.  When the file reaches its
/// size threshold, it rolls over to the next file.
pub struct ProtobufEmitter {
    prefix: PathBuf,
    target: u64,
    state: Mutex<ProtobufOutputState>,
}

impl ProtobufEmitter {
    pub fn new<P: AsRef<Path>>(prefix: P, target: u64) -> Result<Self, std::io::Error> {
        let prefix = prefix.as_ref().to_path_buf();
        let state = Mutex::new(ProtobufOutputState {
            buffer: vec![],
            file: None,
            size: 0,
            timestamp: 0,
        });
        Ok(Self {
            prefix,
            target,
            state,
        })
    }

    fn open(&self, state: &mut ProtobufOutputState) {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_micros())
            .unwrap_or(0);
        let mut path = OsString::from(self.prefix.as_os_str());
        let ext = OsString::from(format!(".{}", ts));
        path.push(ext);
        let path = PathBuf::from(path);
        let Ok(file) = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
        else {
            return;
        };
        state.file = Some(file);
        state.size = 0;
    }

    fn close(&self, state: &mut ProtobufOutputState) {
        if let Some(file) = state.file.as_mut() {
            let _ = file.flush();
        }
        state.file = None;
        state.size = 0;
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
                value,
            },
        };
        let mut state = self.state.lock().unwrap();
        frame.clue.timestamp = std::cmp::max(state.timestamp + 1, frame.clue.timestamp);
        state.timestamp = frame.clue.timestamp;
        stack_pack(&frame).append_to_vec(&mut state.buffer);
        if state.buffer.len() > 1 << 16 {
            let buffer = std::mem::take(&mut state.buffer);
            'retry: for _ in 0..3 {
                if state.file.is_none() {
                    self.open(&mut state);
                }
                let size = state.size;
                if let Some(file) = state.file.as_mut() {
                    if size >= self.target {
                        self.close(&mut state);
                        continue;
                    } else {
                        if file.write_all(&buffer).is_err() {
                            break 'retry;
                        }
                        state.size += buffer.len() as u64;
                        return;
                    }
                }
            }
            EMITTER_FAILURE.click();
            self.close(&mut state);
        }
    }

    fn flush(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(file) = state.file.as_mut() {
            let _ = file.flush();
        }
    }
}

