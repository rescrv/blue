//! Tools for ingesting data into a directory.

use std::fs::{remove_file, rename, File};
use std::path::PathBuf;

use super::log::log_to_builder;
use super::setsum::Setsum;
use super::{Builder, Error, LogBuilder, LogOptions, SstBuilder, SstOptions, TABLE_FULL_SIZE};

/////////////////////////////////////////// IngestOptions //////////////////////////////////////////

/// IngestOptions captures what we care about for ingesting data.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct IngestOptions {
    /// The directory in which to write log files.
    #[cfg_attr(feature = "command_line", arrrg(required, "Path to write logs."))]
    log_dir: String,
    /// The LogOptions to use for ingesting data.
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    log: LogOptions,
    /// The directory in which to put ssts once generated.
    #[cfg_attr(feature = "command_line", arrrg(required, "Path to write ssts."))]
    sst_dir: String,
    /// The options to use for creating ssts.
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    sst: SstOptions,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            log_dir: "logs".to_owned(),
            log: LogOptions::default(),
            sst_dir: "ssts".to_owned(),
            sst: SstOptions::default(),
        }
    }
}

////////////////////////////////////////////// Jester //////////////////////////////////////////////

/// Jester provides a builder interface and writes logs that get converted into ssts.
///
/// It's not intended to be a general-purpose key-value store.  Rather, it is intended for things
/// like logging of stats.
///
/// NOTE:  Jester isn't well tested and doesn't recover logs on errors.  It's a TODO to do so.
// TODO(rescrv): Make this recover logs on crash/restart.
pub struct Jester {
    options: IngestOptions,
    counter: u64,
    builder: Option<LogBuilder<File>>,
    recent: Option<PathBuf>,
}

impl Jester {
    /// Create a new Jester from IngestOptions.
    pub fn new(options: IngestOptions) -> Self {
        Self {
            options,
            counter: 0,
            builder: None,
            recent: None,
        }
    }

    /// Flush the Jester.
    pub fn flush(&mut self) -> Result<(), Error> {
        self.get_builder()?.flush()
    }

    fn get_builder(&mut self) -> Result<&mut LogBuilder<File>, Error> {
        if let Some(builder) = &self.builder {
            let size = builder.approximate_size();
            if size >= TABLE_FULL_SIZE || size >= self.options.log.rollover_size {
                self.rollover_builder()?;
                return self.get_builder();
            }
            return Ok(self.builder.as_mut().unwrap());
        } else {
            loop {
                let path =
                    PathBuf::from(&self.options.log_dir).join(format!("{}.log", self.counter));
                self.counter += 1;
                if !path.exists() {
                    self.builder = Some(LogBuilder::new(self.options.log.clone(), &path)?);
                    self.recent = Some(path);
                    return Ok(self.builder.as_mut().unwrap());
                }
            }
        }
    }

    fn rollover_builder(&mut self) -> Result<(), Error> {
        if self.builder.is_some() {
            let builder = self.builder.take().unwrap();
            let setsum = builder.seal()?.0;
            let recent = self.recent.take().unwrap();
            self.convert_builder(recent, setsum)?;
        }
        Ok(())
    }

    fn convert_builder(&mut self, input: PathBuf, setsum: Setsum) -> Result<(), Error> {
        let output =
            PathBuf::from(&self.options.sst_dir).join(format!("{}.tmp", setsum.hexdigest()));
        let builder = SstBuilder::new(self.options.sst.clone(), &output)?;
        log_to_builder(self.options.log.clone(), &input, builder)?;
        let final_file =
            PathBuf::from(&self.options.sst_dir).join(format!("{}.sst", setsum.hexdigest()));
        rename(output, final_file)?;
        remove_file(input)?;
        Ok(())
    }
}

impl Builder for Jester {
    type Sealed = ();

    /// The approximate size of the current log segment.
    fn approximate_size(&self) -> usize {
        match &self.builder {
            Some(b) => b.approximate_size(),
            None => 0,
        }
    }

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        match self.get_builder()?.put(key, timestamp, value) {
            Ok(_) => Ok(()),
            Err(Error::TableFull { .. }) => {
                self.rollover_builder()?;
                self.put(key, timestamp, value)
            }
            Err(err) => Err(err),
        }
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        match self.get_builder()?.del(key, timestamp) {
            Ok(_) => Ok(()),
            Err(Error::TableFull { .. }) => {
                self.rollover_builder()?;
                self.del(key, timestamp)
            }
            Err(err) => Err(err),
        }
    }

    fn seal(mut self) -> Result<(), Error> {
        self.rollover_builder()?;
        Ok(())
    }
}
