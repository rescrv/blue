use std::os::fd::RawFd;

use biometrics::Collector;

#[cfg(target_os = "linux")]
mod epoll;

pub type Events = u8;

pub const POLLIN: Events = 0x01u8;
pub const POLLOUT: Events = 0x04u8;
pub const POLLERR: Events = 0x08u8;
pub const POLLHUP: Events = 0x10u8;

#[cfg(target_os = "linux")]
pub fn register_biometrics(collector: &mut Collector) {
    epoll::register_biometrics(collector);
}

pub trait Pollster: Send + Sync + 'static {
    fn poll(&self, millis: i32) -> Result<Option<(RawFd, Events)>, rpc_pb::Error>;
    fn arm(&self, fd: RawFd, send: bool) -> Result<(), rpc_pb::Error>;
}

#[cfg(target_os = "linux")]
pub fn default_pollster() -> Result<Box<dyn Pollster>, rpc_pb::Error> {
    let poll = epoll::Epollster::new()?;
    Ok(Box::new(poll))
}
