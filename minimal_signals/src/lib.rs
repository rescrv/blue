use std::mem::MaybeUninit;

////////////////////////////////////////////// Signals /////////////////////////////////////////////

#[derive(Clone, Copy, Default, Eq, PartialEq, Hash)]
pub struct Signal(i32);

impl Signal {
    pub fn from_i32(signal: i32) -> Option<Self> {
        match signal {
            libc::SIGHUP
            | libc::SIGINT
            | libc::SIGQUIT
            | libc::SIGKILL
            | libc::SIGALRM
            | libc::SIGTERM
            | libc::SIGCHLD
            | libc::SIGUSR1
            | libc::SIGUSR2 => Some(Signal(signal)),
            _ => None,
        }
    }

    pub fn into_i32(self) -> i32 {
        self.0
    }
}

pub const SIGHUP: Signal = Signal(libc::SIGHUP);
pub const SIGINT: Signal = Signal(libc::SIGINT);
pub const SIGQUIT: Signal = Signal(libc::SIGQUIT);
pub const SIGKILL: Signal = Signal(libc::SIGKILL);
pub const SIGALRM: Signal = Signal(libc::SIGALRM);
pub const SIGTERM: Signal = Signal(libc::SIGTERM);
pub const SIGCHLD: Signal = Signal(libc::SIGCHLD);
pub const SIGUSR1: Signal = Signal(libc::SIGUSR1);
pub const SIGUSR2: Signal = Signal(libc::SIGUSR2);

///////////////////////////////////////////// SignalSet ////////////////////////////////////////////

pub struct SignalSet {
    sigset: libc::sigset_t,
}

impl SignalSet {
    pub fn new() -> Self {
        let mut none: MaybeUninit<libc::sigset_t> = { MaybeUninit::uninit() };
        // SAFETY(rescrv): This function cannot fail as we give it a valid place to write and it is
        // assumed to be able to work with uninitialized pointers.
        unsafe {
            libc::sigemptyset(none.as_mut_ptr());
            SignalSet {
                sigset: none.assume_init(),
            }
        }
    }

    pub fn empty(mut self) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigemptyset(&mut self.sigset);
        }
        self
    }

    pub fn fill(mut self) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigfillset(&mut self.sigset);
        }
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, signal: Signal) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigaddset(&mut self.sigset, signal.0);
        }
        self
    }

    pub fn del(mut self, signal: Signal) -> Self {
        // SAFETY(rescrv): This function cannot fail as we give it a valid and initialized value.
        unsafe {
            libc::sigdelset(&mut self.sigset, signal.0);
        }
        self
    }

    pub fn ismember(&self, signal: Signal) -> bool {
        unsafe { libc::sigismember(&self.sigset, signal.0) == 1 }
    }

    pub fn iter(&self) -> impl Iterator<Item = Signal> {
        let mut members = vec![];
        let mut maybe_add = |signal| {
            if self.ismember(signal) {
                members.push(signal)
            }
        };
        maybe_add(SIGHUP);
        maybe_add(SIGINT);
        maybe_add(SIGQUIT);
        maybe_add(SIGKILL);
        maybe_add(SIGALRM);
        maybe_add(SIGTERM);
        maybe_add(SIGCHLD);
        maybe_add(SIGUSR1);
        maybe_add(SIGUSR2);
        members.into_iter()
    }
}

impl Default for SignalSet {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SignalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut display = String::new();
        let mut append = |signum, signal| {
            if self.ismember(signum) {
                if !display.is_empty() {
                    display.push('|');
                }
                display += signal;
            }
        };
        append(SIGHUP, "SIGHUP");
        append(SIGINT, "SIGINT");
        append(SIGQUIT, "SIGQUIT");
        append(SIGKILL, "SIGKILL");
        append(SIGALRM, "SIGALRM");
        append(SIGTERM, "SIGTERM");
        append(SIGCHLD, "SIGCHLD");
        append(SIGUSR1, "SIGUSR1");
        append(SIGUSR2, "SIGUSR2");
        if display.is_empty() {
            display += "()";
        }
        write!(f, "{}", display)
    }
}

impl std::fmt::Display for SignalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

/////////////////////////////////////////////// block //////////////////////////////////////////////

pub fn block() {
    let signals = SignalSet::new().fill();
    // SAFETY(rescrv):  We know this is safe because fill fills the set using libc functions.
    unsafe {
        libc::sigprocmask(libc::SIG_SETMASK, &signals.sigset, std::ptr::null_mut());
    };
}

/////////////////////////////////////////////// wait ///////////////////////////////////////////////

pub fn wait(set: SignalSet) -> Option<Signal> {
    let mut signal = -1i32;
    unsafe {
        libc::sigwait(&set.sigset, &mut signal);
    };
    Signal::from_i32(signal)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_set_display() {
        assert_eq!("()", format!("{}", SignalSet::new().empty()));
        assert_eq!(
            "SIGHUP|SIGINT|SIGQUIT|SIGKILL|SIGALRM|SIGTERM|SIGCHLD|SIGUSR1|SIGUSR2",
            format!("{}", SignalSet::new().fill())
        );
        assert_eq!(
            "SIGHUP",
            format!("{}", SignalSet::new().empty().add(SIGHUP))
        );
        assert_eq!(
            "SIGINT",
            format!("{}", SignalSet::new().empty().add(SIGINT))
        );
        assert_eq!(
            "SIGQUIT",
            format!("{}", SignalSet::new().empty().add(SIGQUIT))
        );
        assert_eq!(
            "SIGKILL",
            format!("{}", SignalSet::new().empty().add(SIGKILL))
        );
        assert_eq!(
            "SIGALRM",
            format!("{}", SignalSet::new().empty().add(SIGALRM))
        );
        assert_eq!(
            "SIGTERM",
            format!("{}", SignalSet::new().empty().add(SIGTERM))
        );
        assert_eq!(
            "SIGCHLD",
            format!("{}", SignalSet::new().empty().add(SIGCHLD))
        );
        assert_eq!(
            "SIGUSR1",
            format!("{}", SignalSet::new().empty().add(SIGUSR1))
        );
        assert_eq!(
            "SIGUSR2",
            format!("{}", SignalSet::new().empty().add(SIGUSR2))
        );
        assert_eq!(
            "SIGUSR1|SIGUSR2",
            format!("{}", SignalSet::new().empty().add(SIGUSR1).add(SIGUSR2))
        );
    }
}
