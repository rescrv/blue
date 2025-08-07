use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use libc::c_int;
use utf8path::Path;

/////////////////////////////////////////// ContextState ///////////////////////////////////////////

#[derive(Debug)]
struct ContextState {
    cancel: AtomicBool,
    rx: c_int,
    tx: c_int,
}

impl ContextState {
    fn new() -> Result<Self, std::io::Error> {
        let cancel = AtomicBool::new(false);
        let mut fds: [c_int; 2] = [-1; 2];
        unsafe {
            if libc::pipe(&mut fds as *mut c_int) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        let rx = fds[0];
        let tx = fds[1];
        Ok(ContextState { cancel, rx, tx })
    }

    fn cancel(&self) {
        if self.cancel.swap(true, Ordering::AcqRel) {
            return;
        }
        unsafe {
            libc::close(self.tx);
        }
    }

    fn canceled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn wait(&self, other: &impl AsRawFd) -> Result<(), std::io::Error> {
        let mut pfd = [
            libc::pollfd {
                fd: self.rx,
                events: libc::POLLERR,
                revents: 0,
            },
            libc::pollfd {
                fd: other.as_raw_fd(),
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            },
        ];
        unsafe {
            if libc::poll(pfd.as_mut_ptr(), 2, -1) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

impl Drop for ContextState {
    fn drop(&mut self) {
        self.cancel();
        unsafe {
            libc::close(self.rx);
        }
    }
}

////////////////////////////////////////////// Context /////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct Context {
    state: Arc<ContextState>,
}

impl Context {
    pub fn new() -> Result<Self, std::io::Error> {
        let state = Arc::new(ContextState::new()?);
        Ok(Self { state })
    }

    pub fn cancel(&self) {
        self.state.cancel();
    }

    pub fn canceled(&self) -> bool {
        self.state.canceled()
    }

    pub fn wait(&self, other: &impl AsRawFd) -> Result<(), std::io::Error> {
        self.state.wait(other)
    }
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

pub struct Client {
    path: Path<'static>,
}

impl Client {
    pub fn new<'a>(path: impl Into<Path<'a>>) -> Result<Self, std::io::Error> {
        let path = path.into().into_owned();
        Ok(Client { path })
    }

    pub fn invoke(&mut self, command: &str) -> Result<String, std::io::Error> {
        let mut stream = UnixStream::connect(self.path.as_str())?;
        stream.write_all(command.as_ref())?;
        stream.shutdown(Shutdown::Write)?;
        let mut response = vec![];
        loop {
            let mut buf = [0u8; 4096];
            let amt = stream.read(&mut buf)?;
            if amt == 0 {
                break;
            }
            response.extend(buf[..amt].iter());
        }
        String::from_utf8(response).map_err(|_| std::io::Error::other("expected utf8 in response"))
    }
}

///////////////////////////////////////////// Invokable ////////////////////////////////////////////

pub trait Invokable: Send + Sync {
    fn invoke(&self, command: &str) -> String;
}

////////////////////////////////////////////// Server //////////////////////////////////////////////

pub struct Server {
    path: Path<'static>,
    listener: UnixListener,
    invokable: Arc<dyn Invokable>,
}

impl Server {
    pub fn new<'a, I: Invokable + 'static>(
        path: impl Into<Path<'a>>,
        invoke: I,
    ) -> Result<Self, std::io::Error> {
        let path = path.into().into_owned();
        let listener = UnixListener::bind(path.as_str())?;
        let invokable = Arc::new(invoke);
        Ok(Server {
            path,
            listener,
            invokable,
        })
    }

    pub fn serve(&mut self, context: &Context) -> Result<(), std::io::Error> {
        loop {
            context.wait(&self.listener)?;
            if context.canceled() {
                break;
            }
            let context = context.clone();
            let invokable = self.invokable.clone();
            let (socket, addr) = self.listener.accept()?;
            let context = context.clone();
            let _handle = std::thread::spawn(move || {
                Self::serve_one(&context, invokable.as_ref(), socket, addr);
            });
            // NOTE(rescrv):  We leak handle here and rely upon context being canceled and the
            // thread exiting quickly to clean things up.  If it takes time, that's not a problem.
            // If it doesn't happen, that's not a problem.  The lingering thread can only return an
            // error on its socket---which is what we want.
        }
        Ok(())
    }

    fn serve_one(
        context: &Context,
        invokable: &dyn Invokable,
        mut socket: UnixStream,
        _: SocketAddr,
    ) {
        let mut request = vec![];
        loop {
            if let Err(err) = context.wait(&socket) {
                _ = socket.write_all(format!("error: {err:?}").as_ref());
                return;
            }
            if context.canceled() {
                _ = socket.write_all("error: server shut down".as_ref());
                return;
            }
            let mut buf = [0u8; 4096];
            let amt = match socket.read(&mut buf) {
                Ok(amt) => amt,
                Err(err) => {
                    _ = socket.write_all(format!("error: could not read: {err:?}").as_ref());
                    return;
                }
            };
            if amt == 0 {
                break;
            }
            request.extend(buf[..amt].iter());
            if request.len() >= 65536 {
                _ = socket.write_all("error: request exceeds 65536 bytes".as_ref());
                return;
            }
        }
        let request = match String::from_utf8(request) {
            Ok(request) => request,
            Err(err) => {
                _ = socket
                    .write_all(format!("error: could not interpret as utf8: {err:?}").as_ref());
                return;
            }
        };
        let response = invokable.invoke(&request);
        _ = socket.write_all(response.as_ref());
    }
}

impl std::fmt::Debug for Server {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(fmt, "Server({:?})", self.path)
    }
}
