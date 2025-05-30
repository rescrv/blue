//! error_core is a default implementation of [zerror::Z].

use std::backtrace::Backtrace;
use std::fmt::Debug;

use biometrics::{Collector, Counter};

use buffertk::{Packable, Unpackable};

use tatl::{HeyListen, Stationary};

use prototk::field_types::*;
use prototk::{FieldPackHelper, FieldUnpackHelper, Message, Tag};
use prototk_derive::Message;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

pub static DEFAULT_ERROR_CORE: Counter = Counter::new("zerror_core.default");
pub static DEFAULT_ERROR_CORE_MONITOR: Stationary =
    Stationary::new("zerror_core.default", &DEFAULT_ERROR_CORE);

/// Register the monitors for this crate.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&DEFAULT_ERROR_CORE_MONITOR);
}

/// Register the biometrics for this crate.
pub fn register_biometrics(collector: Collector) {
    collector.register_counter(&DEFAULT_ERROR_CORE);
}

///////////////////////////////////////////// ErrorCore ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
struct Info {
    #[prototk(1, string)]
    name: String,
    #[prototk(2, string)]
    value: String,
}

#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
struct Internals {
    // reserved 1: email
    // reserved 2: short
    #[prototk(3, string)]
    backtrace: String,
    #[prototk(7, message)]
    info: Vec<Info>,
}

/// [ErrorCore] implements 100% of Z for easy error reporting.  It's intended that people will wrap
/// and proxy ErrorCore and then implement a short summary on top that descends from an error enum.
#[derive(Clone, Debug)]
pub struct ErrorCore {
    internals: Box<Internals>,
}

impl ErrorCore {
    /// Create a new ErrorCore with the provided counter.  The provided counter will be clicked
    /// each time a new error is created, to give people insight into the error.  It's advisable to
    /// have a separate counter for different conditions.
    pub fn new(counter: &'static Counter) -> Self {
        counter.click();
        let backtrace = format!("{}", Backtrace::force_capture());
        let internals = Internals {
            backtrace,
            info: Vec::new(),
        };
        Self {
            internals: Box::new(internals),
        }
    }

    /// Print the long-form of the error.
    pub fn long_form(&self) -> String {
        let mut s = String::default();
        if !self.internals.info.is_empty() {
            for info in self.internals.info.iter() {
                s += &format!("\n{} = {}", info.name, info.value);
            }
        }
        s += &format!("\n\nbacktrace:\n{}", self.internals.backtrace);
        s.trim().to_owned() + "\n"
    }

    /// Add debug formatting of a local variable.
    pub fn set_info<X: Debug>(&mut self, name: &str, value: X) {
        self.internals.info.push(Info {
            name: name.to_owned(),
            value: format!("{:?}", value),
        });
    }

    /// Add debug formatting using a closure.
    pub fn set_lazy_info<F: FnOnce() -> String>(&mut self, name: &str, value: F) {
        self.internals.info.push(Info {
            name: name.to_owned(),
            value: value(),
        });
    }
}

impl Default for ErrorCore {
    fn default() -> Self {
        Self::new(&DEFAULT_ERROR_CORE)
    }
}

impl Packable for ErrorCore {
    fn pack_sz(&self) -> usize {
        <Internals as Packable>::pack_sz(&self.internals)
    }

    fn pack(&self, buf: &mut [u8]) {
        <Internals as Packable>::pack(&self.internals, buf)
    }
}

impl<'a> Unpackable<'a> for ErrorCore {
    type Error = prototk::Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (internals, buf) = <Internals as Unpackable<'a>>::unpack(buf)?;
        Ok((
            Self {
                internals: Box::new(internals),
            },
            buf,
        ))
    }
}

impl FieldPackHelper<'_, message<ErrorCore>> for ErrorCore {
    fn field_pack_sz(&self, tag: &Tag) -> usize {
        Internals::field_pack_sz(&self.internals, tag)
    }

    fn field_pack(&self, tag: &Tag, out: &mut [u8]) {
        Internals::field_pack(&self.internals, tag, out)
    }
}

impl FieldUnpackHelper<'_, message<ErrorCore>> for ErrorCore {
    fn merge_field(&mut self, proto: message<ErrorCore>) {
        *self = proto.unwrap_message();
    }
}

impl Message<'_> for ErrorCore {}

impl From<message<ErrorCore>> for ErrorCore {
    fn from(proto: message<Self>) -> Self {
        proto.unwrap_message()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use biometrics::Sensor;

    use buffertk::stack_pack;

    use super::*;

    static TEST_COUNTER1: Counter = Counter::new("zerror_core.test_counter1");
    static TEST_COUNTER2: Counter = Counter::new("zerror_core.test_counter2");

    #[test]
    fn serialize_empty_error_core() {
        assert_eq!(0, TEST_COUNTER1.read());
        let mut error_core = ErrorCore::new(&TEST_COUNTER1);
        assert_eq!(1, TEST_COUNTER1.read());
        "SOME-BACKTRACE\n".clone_into(&mut error_core.internals.backtrace);
        assert_eq!("backtrace:\nSOME-BACKTRACE\n", error_core.long_form());
        let buf = stack_pack(&error_core).to_vec();
        let got: ErrorCore = Unpackable::unpack(&buf).unwrap().0;
        assert_eq!(&error_core.internals, &got.internals);
    }

    #[test]
    fn serialize_used_error_core() {
        assert_eq!(0, TEST_COUNTER2.read());
        let mut error_core = ErrorCore::new(&TEST_COUNTER2);
        assert_eq!(1, TEST_COUNTER2.read());
        "SOME-BACKTRACE\n".clone_into(&mut error_core.internals.backtrace);
        error_core.set_info("VAR", 42);
        assert_eq!(
            "VAR = 42

backtrace:
SOME-BACKTRACE
",
            error_core.long_form()
        );
        let buf = stack_pack(&error_core).to_vec();
        let got: ErrorCore = Unpackable::unpack(&buf).unwrap().0;
        assert_eq!(&error_core.internals, &got.internals);
    }
}
