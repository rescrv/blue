use std::path::PathBuf;
use std::fmt::{Debug, Display, Formatter};

use biometrics::Counter;

use hey_listen::{Stationary, HeyListen};

use zerror::Z;

use zerror_core::ErrorCore;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_NOT_OBTAINED: Counter = Counter::new("lsmtk.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lsmtk.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
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
    CRC32CFailure {
        core: ErrorCore,
        start: u64,
        limit: u64,
        crc32c: u32,
    },
    LockNotObtained {
        core: ErrorCore,
        path: PathBuf,
    },
    DuplicateSST {
        core: ErrorCore,
        what: String,
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
        context: String,
    },
    IOError {
        core: ErrorCore,
        what: std::io::Error,
    },
    TooManyOpenFiles {
        core: ErrorCore,
        limit: usize,
    },
    SSTNotFound {
        core: ErrorCore,
        setsum: String,
    },
    DBExists {
        core: ErrorCore,
        path: PathBuf,
    },
    DBNotExist {
        core: ErrorCore,
        path: PathBuf,
    },
    PathError {
        core: ErrorCore,
        path: PathBuf,
        what: String,
    },
    MissingManifest {
        core: ErrorCore,
        path: PathBuf,
    },
    MissingSST {
        core: ErrorCore,
        path: PathBuf,
    },
    ExtraFile {
        core: ErrorCore,
        path: PathBuf,
    },
    InvalidManifestLine {
        core: ErrorCore,
        line: String,
    },
    InvalidManifestCommand {
        core: ErrorCore,
        cmd: String,
        arg: String,
    },
    InvalidManifestSetsum {
        core: ErrorCore,
        manifest: String,
        computed: String,
    },
    InvalidSSTSetsum {
        core: ErrorCore,
        expected: String,
        computed: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::CRC32CFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSST { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::IOError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SSTNotFound { core, .. } => { core } ,
            Error::DBExists { core, .. } => { core } ,
            Error::DBNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
            Error::MissingManifest { core, .. } => { core } ,
            Error::MissingSST { core, .. } => { core } ,
            Error::ExtraFile { core, .. } => { core } ,
            Error::InvalidManifestLine { core, .. } => { core } ,
            Error::InvalidManifestCommand { core, .. } => { core } ,
            Error::InvalidManifestSetsum { core, .. } => { core } ,
            Error::InvalidSSTSetsum { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::CRC32CFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSST { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::IOError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SSTNotFound { core, .. } => { core } ,
            Error::DBExists { core, .. } => { core } ,
            Error::DBNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
            Error::MissingManifest { core, .. } => { core } ,
            Error::MissingSST { core, .. } => { core } ,
            Error::ExtraFile { core, .. } => { core } ,
            Error::InvalidManifestLine { core, .. } => { core } ,
            Error::InvalidManifestCommand { core, .. } => { core } ,
            Error::InvalidManifestSetsum { core, .. } => { core } ,
            Error::InvalidSSTSetsum { core, .. } => { core } ,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
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

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv):  Make sure this isn't infinitely co-recursive with long_form
        write!(fmt, "{}", self.long_form())
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::IOError { core: ErrorCore::default(), what }
    }
}

////////////////////////////////////////////// FromIO //////////////////////////////////////////////

pub trait FromIO {
    type Result;

    fn from_io(self) -> Self::Result;
}

impl<T> FromIO for Result<T, std::io::Error> {
    type Result = Result<T, Error>;

    fn from_io(self) -> Self::Result {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }
}
