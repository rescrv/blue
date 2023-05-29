use std::collections::VecDeque;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use biometrics::{Collector, Counter};

use hey_listen::{HeyListen, Stationary};

use rpc_pb::Error;

///////////////////////////////////////////// constants ////////////////////////////////////////////

pub const POLLIN: u32 = 0x0001;
pub const POLLOUT: u32 = 0x0004;
pub const POLLERR: u32 = 0x0008;
pub const POLLHUP: u32 = 0x0010;

pub fn to_poll_constants(x: u32) -> i16 {
    let mut fx: i16 = 0;
    if x & POLLIN != 0 {
        fx |= libc::POLLIN;
    }
    if x & POLLOUT != 0 {
        fx |= libc::POLLOUT;
    }
    if x & POLLERR != 0 {
        fx |= libc::POLLERR;
    }
    if x & POLLHUP != 0 {
        fx |= libc::POLLHUP;
    }
    fx
}

pub fn from_epoll_constants(x: i32) -> u32 {
    let mut events: u32 = 0;
    if x & libc::EPOLLIN != 0 {
        events |= POLLIN;
    }
    if x & libc::EPOLLOUT != 0 {
        events |= POLLOUT;
    }
    if x & libc::EPOLLERR != 0 {
        events |= POLLERR;
    }
    if x & libc::EPOLLHUP != 0 {
        events |= POLLHUP;
    }
    events
}

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static FD_TRUNCATED: Counter = Counter::new("rivulet.fd_truncated");
static FD_TRUNCATED_MONITOR: Stationary = Stationary::new("rivulet.fd_truncated", &FD_TRUNCATED);

static NEW_THREAD: Counter = Counter::new("rivulet.new_thread");

static CONSERVE_POLLIN: Counter = Counter::new("rivulet.conserve_pollin");
static CONSERVE_POLLOUT: Counter = Counter::new("rivulet.conserve_pollout");
static RETURN_CONSERVED_POLLIN: Counter = Counter::new("rivulet.return_pollin");
static RETURN_CONSERVED_POLLOUT: Counter = Counter::new("rivulet.return_pollout");

static POLL_ERROR: Counter = Counter::new("rivulet.poll.error");
static POLL_TIMEOUT: Counter = Counter::new("rivulet.poll.timeout");
static POLL_RETURN: Counter = Counter::new("rivulet.poll.return");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&FD_TRUNCATED);
    collector.register_counter(&NEW_THREAD);
    collector.register_counter(&CONSERVE_POLLIN);
    collector.register_counter(&CONSERVE_POLLOUT);
    collector.register_counter(&RETURN_CONSERVED_POLLIN);
    collector.register_counter(&RETURN_CONSERVED_POLLOUT);
}

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&FD_TRUNCATED_MONITOR);
}

//////////////////////////////////////////// ThreadState ///////////////////////////////////////////

pub struct ThreadState {
    ratio: usize,
    offset: usize,
}

////////////////////////////////////////////// OsPoll //////////////////////////////////////////////

pub trait OsPoll: Send + Sync {
    fn new_thread(&self) -> ThreadState;
    fn insert(&self, fd: RawFd) -> Result<(), Error>;
    fn poll(&self, ts: &mut ThreadState, timeout_ms: i32) -> Result<Option<(RawFd, u32)>, Error>;
}

/////////////////////////////////////////////// Poll ///////////////////////////////////////////////

pub trait Poll: OsPoll {
    fn conserve(&self, ts: &mut ThreadState, fd: RawFd, events: u32);
}

/////////////////////////////////////////////// Epoll //////////////////////////////////////////////

pub struct Epoll {
    epfd: RawFd,
    threads: AtomicU64,
}

impl Epoll {
    fn new() -> Result<Self, Error> {
        let epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epfd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self {
            epfd,
            threads: AtomicU64::new(0),
        })
    }
}

impl OsPoll for Epoll {
    fn new_thread(&self) -> ThreadState {
        let index: u64 = self.threads.fetch_add(1, Ordering::Relaxed);
        let ratio = if index == 0 {
            1
        } else if index == 1 {
            0
        } else {
            (index + 1).ilog2() as usize
        };
        NEW_THREAD.click();
        ThreadState {
            ratio,
            offset: 0,
        }
    }

    fn insert(&self, fd: RawFd) -> Result<(), Error> {
        let mut ev = libc::epoll_event {
            events: (libc::EPOLLET | libc::EPOLLIN | libc::EPOLLOUT) as u32,
            u64: fd as u64,
        };
        let ret = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut ev) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }

    fn poll(&self, _: &mut ThreadState, timeout_ms: i32) -> Result<Option<(RawFd, u32)>, Error> {
        let mut ev = libc::epoll_event { events: 0, u64: 0 };
        let ret = unsafe { libc::epoll_wait(self.epfd, &mut ev, 1, timeout_ms) };
        if ret < 0 {
            POLL_ERROR.click();
            Err(std::io::Error::last_os_error().into())
        } else if ret == 0 {
            POLL_TIMEOUT.click();
            Ok(None)
        } else if ev.u64 > i32::max_value() as u64 {
            FD_TRUNCATED.click();
            Ok(None)
        } else {
            POLL_RETURN.click();
            assert_eq!(1, ret);
            let fd = ev.u64 as RawFd;
            Ok(Some((fd, from_epoll_constants(ev.events as i32))))
        }
    }
}

///////////////////////////////////////// ConservingWrapper ////////////////////////////////////////

pub struct ConservingWrapper<P: OsPoll> {
    os_poll: P,
    conserved: Mutex<VecDeque<(RawFd, u32)>>,
}

impl<P: OsPoll> ConservingWrapper<P> {
    fn new(os_poll: P) -> Self {
        Self {
            os_poll,
            conserved: Mutex::new(VecDeque::new()),
        }
    }
}

impl<P: OsPoll> OsPoll for ConservingWrapper<P> {
    fn new_thread(&self) -> ThreadState {
        self.os_poll.new_thread()
    }

    fn insert(&self, fd: RawFd) -> Result<(), Error> {
        self.os_poll.insert(fd)
    }

    fn poll(&self, ts: &mut ThreadState, timeout_ms: i32) -> Result<Option<(RawFd, u32)>, Error> {
        if ts.offset < ts.ratio {
            let conserved = self.conserved.lock().unwrap().pop_front();
            if let Some((fd, events)) = conserved {
                if events & POLLIN != 0 {
                    RETURN_CONSERVED_POLLIN.click();
                }
                if events & POLLOUT != 0 {
                    RETURN_CONSERVED_POLLOUT.click();
                }
                ts.offset += 1;
                return Ok(Some((fd, events)))
            }
        } else {
            ts.offset = 0;
        }
        self.os_poll.poll(ts, timeout_ms)
    }
}

impl<P: OsPoll> Poll for ConservingWrapper<P> {
    fn conserve(&self, _: &mut ThreadState, fd: RawFd, events: u32) {
        assert_ne!(0, events);
        if events & POLLIN != 0 {
            CONSERVE_POLLIN.click();
        }
        if events & POLLOUT != 0 {
            CONSERVE_POLLOUT.click();
        }
        self.conserved.lock().unwrap().push_back((fd, events))
    }
}

/////////////////////////////////////////// default_poll ///////////////////////////////////////////

pub fn default_poll() -> Result<Box<dyn Poll>, Error> {
    Ok(Box::new(ConservingWrapper::new(Epoll::new()?)))
}
