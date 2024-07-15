use std::os::fd::RawFd;

use biometrics::{Collector, Counter};

use super::{Events, Pollster, POLLERR, POLLHUP, POLLIN, POLLOUT};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static POLL_ADD: Counter = Counter::new("busybee.poll.epoll.add");
static POLL_ADD_ERR: Counter = Counter::new("busybee.poll.epoll.add.error");
static POLL_MOD: Counter = Counter::new("busybee.poll.epoll.mod");
static POLL_MOD_ERR: Counter = Counter::new("busybee.poll.epoll.mod.error");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&POLL_ADD);
    collector.register_counter(&POLL_ADD_ERR);
    collector.register_counter(&POLL_MOD);
    collector.register_counter(&POLL_MOD_ERR);
}

///////////////////////////////////////// Event Conversion /////////////////////////////////////////

fn map_events_to_ep(ev: Events) -> u32 {
    let mut out = 0u32;
    if (ev & POLLIN) != 0 {
        out |= libc::EPOLLIN as u32;
    }
    if (ev & POLLOUT) != 0 {
        out |= libc::EPOLLOUT as u32;
    }
    if (ev & POLLERR) != 0 {
        out |= libc::EPOLLERR as u32;
    }
    if (ev & POLLHUP) != 0 {
        out |= libc::EPOLLHUP as u32;
    }
    out
}

fn map_ep_to_events(x: u32) -> Events {
    let mut out = 0;
    if (x & libc::EPOLLIN as u32) != 0 {
        out |= POLLIN;
    }
    if (x & libc::EPOLLOUT as u32) != 0 {
        out |= POLLOUT;
    }
    if (x & libc::EPOLLERR as u32) != 0 {
        out |= POLLERR;
    }
    if (x & libc::EPOLLHUP as u32) != 0 {
        out |= POLLHUP;
    }
    out
}

///////////////////////////////////////////// Epollster ////////////////////////////////////////////

pub struct Epollster {
    epoll_inst: RawFd,
}

impl Epollster {
    pub fn new() -> Result<Self, rpc_pb::Error> {
        let ret = unsafe { libc::epoll_create(1) };
        if ret < 1 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self { epoll_inst: ret })
    }
}

impl Pollster for Epollster {
    fn poll(&self, millis: i32) -> Result<Option<(RawFd, Events)>, rpc_pb::Error> {
        let mut ep_event = libc::epoll_event { events: 0, u64: 0 };
        let ret = unsafe { libc::epoll_wait(self.epoll_inst, &mut ep_event, 1, millis) };
        if ret < 0 && unsafe { *libc::__errno_location() } == libc::EINTR {
            Ok(None)
        } else if ret < 0 {
            Err(std::io::Error::last_os_error().into())
        } else if ret == 0 {
            Ok(None)
        } else {
            Ok(Some((
                ep_event.u64 as RawFd,
                map_ep_to_events(ep_event.events),
            )))
        }
    }

    fn arm(&self, fd: RawFd, send: bool) -> Result<(), rpc_pb::Error> {
        let mut ev = POLLIN | POLLERR | POLLHUP;
        if send {
            ev |= POLLOUT;
        }
        let mut ep_event = libc::epoll_event {
            events: map_events_to_ep(ev) | libc::EPOLLONESHOT as u32,
            u64: fd as u64,
        };
        POLL_MOD.click();
        let ret =
            unsafe { libc::epoll_ctl(self.epoll_inst, libc::EPOLL_CTL_MOD, fd, &mut ep_event) };
        let ret = if ret < 0 && unsafe { *libc::__errno_location() } == libc::ENOENT {
            POLL_ADD.click();
            unsafe { libc::epoll_ctl(self.epoll_inst, libc::EPOLL_CTL_ADD, fd, &mut ep_event) }
        } else if ret < 0 {
            POLL_MOD_ERR.click();
            return Err(std::io::Error::last_os_error().into());
        } else {
            ret
        };
        if ret < 0 {
            POLL_ADD_ERR.click();
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
}
