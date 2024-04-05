use biometrics::{Counter, Emitter};

/////////////////////////////////////////// BiometricsSys //////////////////////////////////////////

pub struct BiometricsSys {
    utime: u64,
    stime: u64,
    minflt: u64,
    majflt: u64,
    inblock: u64,
    oublock: u64,
    nvcsw: u64,
    nivcsw: u64,
    errors: u64,
}

impl BiometricsSys {
    pub fn new() -> Self {
        Self {
            utime: 0,
            stime: 0,
            minflt: 0,
            majflt: 0,
            inblock: 0,
            oublock: 0,
            nvcsw: 0,
            nivcsw: 0,
            errors: 0,
        }
    }

    pub fn emit<E: Emitter>(&mut self, emitter: &mut E, now: u64) {
        let rusage = self.getrusage();
        let errors = self.errors;
        let mut output_counter = |label, count: u64| {
            let counter = Counter::new(label);
            counter.count(count);
            if emitter.emit_counter(&counter, now).is_err() {
                self.errors += 1;
            }
        };
        self.utime = std::cmp::max(
            self.utime,
            rusage
                .ru_utime
                .tv_sec
                .saturating_mul(1_000_000)
                .saturating_add(rusage.ru_utime.tv_usec) as u64,
        );
        output_counter("biometrics.sys.utime", self.utime);
        self.stime = std::cmp::max(
            self.stime,
            rusage
                .ru_stime
                .tv_sec
                .saturating_mul(1_000_000)
                .saturating_add(rusage.ru_stime.tv_usec) as u64,
        );
        output_counter("biometrics.sys.stime", self.stime);
        self.minflt = std::cmp::max(self.minflt, rusage.ru_minflt as u64);
        output_counter("biometrics.sys.minflt", self.minflt);
        self.majflt = std::cmp::max(self.majflt, rusage.ru_majflt as u64);
        output_counter("biometrics.sys.majflt", self.majflt);
        self.inblock = std::cmp::max(self.inblock, rusage.ru_inblock as u64);
        output_counter("biometrics.sys.inblock", self.inblock);
        self.oublock = std::cmp::max(self.oublock, rusage.ru_oublock as u64);
        output_counter("biometrics.sys.oublock", self.oublock);
        self.nvcsw = std::cmp::max(self.nvcsw, rusage.ru_nvcsw as u64);
        output_counter("biometrics.sys.nvcsw", self.nvcsw);
        self.nivcsw = std::cmp::max(self.nivcsw, rusage.ru_nivcsw as u64);
        output_counter("biometrics.sys.nivcsw", self.nivcsw);
        output_counter("biometrics.sys.errors", errors);
    }

    fn getrusage(&mut self) -> libc::rusage {
        let mut rusage = libc::rusage {
            ru_utime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_stime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_maxrss: 0,
            ru_ixrss: 0,
            ru_idrss: 0,
            ru_isrss: 0,
            ru_minflt: 0,
            ru_majflt: 0,
            ru_nswap: 0,
            ru_inblock: 0,
            ru_oublock: 0,
            ru_msgsnd: 0,
            ru_msgrcv: 0,
            ru_nsignals: 0,
            ru_nvcsw: 0,
            ru_nivcsw: 0,
        };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage) } < 0 {
            self.errors += 1;
        }
        rusage
    }
}

impl Default for BiometricsSys {
    fn default() -> Self {
        Self::new()
    }
}
