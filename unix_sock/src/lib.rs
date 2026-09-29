use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

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
        // Keep the pipe out of every process the embedding program spawns.
        for fd in fds {
            // SAFETY(rescrv): fcntl observes only the fd we just created and integer flags.
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::close(rx);
                    libc::close(tx);
                }
                return Err(err);
            }
        }
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
        self.wait_timeout(other, None).map(|_| ())
    }

    /// Wait until `other` is readable, the context is canceled, or `timeout` passes.  False only
    /// on timeout.
    fn wait_timeout(
        &self,
        other: &impl AsRawFd,
        timeout: Option<Duration>,
    ) -> Result<bool, std::io::Error> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
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
            let millis = match deadline {
                None => -1,
                Some(deadline) => deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(libc::c_int::MAX as u128) as libc::c_int,
            };
            // SAFETY(rescrv): pfd is a valid array of two pollfds.
            let rc = unsafe { libc::poll(pfd.as_mut_ptr(), 2, millis) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                return Err(err);
            }
            return Ok(rc > 0);
        }
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

    /// Like [Context::wait], but gives up after `timeout`.  False on timeout.
    pub fn wait_timeout(
        &self,
        other: &impl AsRawFd,
        timeout: Duration,
    ) -> Result<bool, std::io::Error> {
        self.state.wait_timeout(other, Some(timeout))
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

/////////////////////////////////////////// ServerOptions //////////////////////////////////////////

/// How a [Server] guards its socket.
#[derive(Clone, Debug)]
pub struct ServerOptions {
    /// Permission bits for the socket file.  Set before the socket is reachable at its path.
    pub mode: u32,
    /// Refuse peers whose uid is neither ours nor root, where the platform reports peer uids.
    pub same_uid_only: bool,
    /// Connections served at once; more are refused with an error.
    pub max_connections: usize,
    /// How long a client gets to send its whole request, and to accept the response.
    pub request_timeout: Duration,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            mode: 0o600,
            same_uid_only: true,
            max_connections: 64,
            request_timeout: Duration::from_secs(10),
        }
    }
}

////////////////////////////////////////////// Server //////////////////////////////////////////////

pub struct Server {
    path: Path<'static>,
    listener: UnixListener,
    invokable: Arc<dyn Invokable>,
    options: ServerOptions,
    active: Arc<AtomicUsize>,
}

impl Server {
    /// Listen at `path` with [ServerOptions::default].  See [Server::with_options].
    pub fn new<'a, I: Invokable + 'static>(
        path: impl Into<Path<'a>>,
        invoke: I,
    ) -> Result<Self, std::io::Error> {
        Self::with_options(path, invoke, ServerOptions::default())
    }

    /// Listen at `path`.
    ///
    /// A socket left at `path` by a server that is gone is replaced.  If a server is listening
    /// there, this fails with `AddrInUse`; if `path` is anything but a socket, with
    /// `AlreadyExists`.
    pub fn with_options<'a, I: Invokable + 'static>(
        path: impl Into<Path<'a>>,
        invoke: I,
        options: ServerOptions,
    ) -> Result<Self, std::io::Error> {
        let path = path.into().into_owned();
        let listener = bind(&path, options.mode)?;
        listener.set_nonblocking(true)?;
        let invokable = Arc::new(invoke);
        let active = Arc::new(AtomicUsize::new(0));
        Ok(Server {
            path,
            listener,
            invokable,
            options,
            active,
        })
    }

    pub fn serve(&mut self, context: &Context) -> Result<(), std::io::Error> {
        loop {
            context.wait(&self.listener)?;
            if context.canceled() {
                break;
            }
            let (mut socket, addr) = match self.listener.accept() {
                Ok(accepted) => accepted,
                Err(err) => match err.raw_os_error() {
                    // The connection went away between poll and accept, or we were interrupted.
                    Some(libc::EAGAIN)
                    | Some(libc::EINTR)
                    | Some(libc::ECONNABORTED)
                    | Some(libc::EPROTO) => continue,
                    // Out of descriptors or memory:  back off and let in-flight requests finish.
                    Some(libc::EMFILE) | Some(libc::ENFILE) | Some(libc::ENOBUFS)
                    | Some(libc::ENOMEM) => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    _ => return Err(err),
                },
            };
            // BSDs hand back a socket that inherits the listener's O_NONBLOCK.
            if socket.set_nonblocking(false).is_err()
                || socket
                    .set_write_timeout(Some(self.options.request_timeout))
                    .is_err()
            {
                continue;
            }
            if self.options.same_uid_only && !peer_allowed(&socket) {
                refuse(&mut socket, "error: permission denied");
                continue;
            }
            if self.active.fetch_add(1, Ordering::AcqRel) >= self.options.max_connections {
                self.active.fetch_sub(1, Ordering::AcqRel);
                refuse(&mut socket, "error: too many connections");
                continue;
            }
            let context = context.clone();
            let invokable = self.invokable.clone();
            let active = Arc::clone(&self.active);
            let timeout = self.options.request_timeout;
            let spawned = std::thread::Builder::new().spawn(move || {
                Self::serve_one(&context, invokable.as_ref(), socket, addr, timeout);
                active.fetch_sub(1, Ordering::AcqRel);
            });
            // NOTE(rescrv):  We leak handle here and rely upon context being canceled and the
            // thread exiting quickly to clean things up.  If it takes time, that's not a problem.
            // If it doesn't happen, that's not a problem.  The lingering thread can only return an
            // error on its socket---which is what we want.
            if spawned.is_err() {
                self.active.fetch_sub(1, Ordering::AcqRel);
            }
        }
        Ok(())
    }

    fn serve_one(
        context: &Context,
        invokable: &dyn Invokable,
        mut socket: UnixStream,
        _: SocketAddr,
        timeout: Duration,
    ) {
        let deadline = Instant::now() + timeout;
        let mut request = vec![];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match context.wait_timeout(&socket, remaining) {
                Ok(true) => {}
                Ok(false) => {
                    _ = socket.write_all(b"error: request timed out");
                    return;
                }
                Err(err) => {
                    _ = socket.write_all(format!("error: {err:?}").as_ref());
                    return;
                }
            }
            if context.canceled() {
                _ = socket.write_all("error: server shut down".as_ref());
                return;
            }
            let mut buf = [0u8; 4096];
            let amt = match socket.read(&mut buf) {
                Ok(amt) => amt,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
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

/// Answer a connection we will not serve.  Consume what the client sent first (briefly, since this
/// runs on the accept loop):  closing with unread data resets the connection and the client would
/// never see the message.
fn refuse(socket: &mut UnixStream, message: &str) {
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut buf = [0u8; 4096];
    let mut total = 0;
    while total < 65536 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || socket.set_read_timeout(Some(remaining)).is_err() {
            break;
        }
        match socket.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
    _ = socket.write_all(message.as_bytes());
}

/// Bind `path`, replacing a stale socket but never a live one, with `mode` set before the socket is
/// reachable at `path`.
fn bind(path: &Path, mode: u32) -> Result<UnixListener, std::io::Error> {
    match std::fs::symlink_metadata(path.as_str()) {
        Ok(metadata) if !metadata.file_type().is_socket() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{path} exists and is not a socket"),
            ));
        }
        Ok(_) => match UnixStream::connect(path.as_str()) {
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("a server is already listening on {path}"),
                ));
            }
            Err(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(path.as_str())?;
            }
            Err(err) => return Err(err),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    // Bind under a private name, fix the mode, then rename into place:  the socket is never
    // reachable at `path` with the umask's permissions.
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let std_path = std::path::Path::new(path.as_str());
    let name = std_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = std_path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let listener = UnixListener::bind(&tmp)?;
    let placed = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))
        .and_then(|()| std::fs::rename(&tmp, path.as_str()));
    if let Err(err) = placed {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(listener)
}

/// True if the peer is us or root.  Where the platform cannot say, the socket's mode is the guard.
fn peer_allowed(socket: &UnixStream) -> bool {
    // SAFETY(rescrv): geteuid cannot fail.
    let me = unsafe { libc::geteuid() };
    match peer_uid(socket) {
        Some(Ok(uid)) => uid == me || uid == 0,
        Some(Err(_)) => false,
        None => true,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(socket: &UnixStream) -> Option<Result<libc::uid_t, std::io::Error>> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY(rescrv): getsockopt writes at most len bytes into cred.
    let rc = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        )
    };
    if rc < 0 {
        return Some(Err(std::io::Error::last_os_error()));
    }
    Some(Ok(cred.uid))
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn peer_uid(socket: &UnixStream) -> Option<Result<libc::uid_t, std::io::Error>> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY(rescrv): getpeereid writes only into uid and gid.
    if unsafe { libc::getpeereid(socket.as_raw_fd(), &mut uid, &mut gid) } < 0 {
        return Some(Err(std::io::Error::last_os_error()));
    }
    Some(Ok(uid))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn peer_uid(_: &UnixStream) -> Option<Result<libc::uid_t, std::io::Error>> {
    None
}

impl std::fmt::Debug for Server {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(fmt, "Server({:?})", self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Invokable for Echo {
        fn invoke(&self, command: &str) -> String {
            format!("echo: {command}")
        }
    }

    fn sock_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("unix-sock-{name}-{}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    /// Serve on a thread; returns the context to cancel and the join handle.
    fn serve(mut server: Server) -> (Context, std::thread::JoinHandle<()>) {
        let context = Context::new().unwrap();
        let ctx = context.clone();
        let handle = std::thread::spawn(move || server.serve(&ctx).unwrap());
        (context, handle)
    }

    #[test]
    fn a_stale_socket_is_reclaimed() {
        let path = sock_path("stale");
        let _ = std::fs::remove_file(&path);
        drop(UnixListener::bind(&path).unwrap());
        assert!(std::fs::symlink_metadata(&path).is_ok());
        let (context, handle) = serve(Server::new(path.as_str(), Echo).unwrap());
        let response = Client::new(path.as_str()).unwrap().invoke("hi").unwrap();
        assert_eq!("echo: hi", response);
        context.cancel();
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_live_socket_is_not_stolen() {
        let path = sock_path("live");
        let _ = std::fs::remove_file(&path);
        let (context, handle) = serve(Server::new(path.as_str(), Echo).unwrap());
        let err = Server::new(path.as_str(), Echo).err().unwrap();
        assert_eq!(std::io::ErrorKind::AddrInUse, err.kind());
        // The first server still answers.
        let response = Client::new(path.as_str()).unwrap().invoke("x").unwrap();
        assert_eq!("echo: x", response);
        context.cancel();
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_non_socket_is_left_alone() {
        let path = sock_path("file");
        std::fs::write(&path, "precious").unwrap();
        let err = Server::new(path.as_str(), Echo).err().unwrap();
        assert_eq!(std::io::ErrorKind::AlreadyExists, err.kind());
        assert_eq!("precious", std::fs::read_to_string(&path).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_socket_is_private() {
        let path = sock_path("mode");
        let _ = std::fs::remove_file(&path);
        let server = Server::new(path.as_str(), Echo).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(0o600, mode & 0o777);
        drop(server);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_silent_client_times_out() {
        let path = sock_path("timeout");
        let _ = std::fs::remove_file(&path);
        let options = ServerOptions {
            request_timeout: Duration::from_millis(200),
            ..ServerOptions::default()
        };
        let (context, handle) = serve(Server::with_options(path.as_str(), Echo, options).unwrap());
        let mut stream = UnixStream::connect(&path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert_eq!("error: request timed out", response);
        context.cancel();
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn connections_are_bounded() {
        let path = sock_path("bounded");
        let _ = std::fs::remove_file(&path);
        let options = ServerOptions {
            max_connections: 1,
            ..ServerOptions::default()
        };
        let (context, handle) = serve(Server::with_options(path.as_str(), Echo, options).unwrap());
        let idle = UnixStream::connect(&path).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let response = Client::new(path.as_str()).unwrap().invoke("x").unwrap();
        assert_eq!("error: too many connections", response);
        drop(idle);
        std::thread::sleep(Duration::from_millis(100));
        context.cancel();
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn context_pipes_do_not_leak_into_children() {
        let context = Context::new().unwrap();
        for fd in [context.state.rx, context.state.tx] {
            // SAFETY(rescrv): fcntl observes only the fd.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            assert_ne!(0, flags & libc::FD_CLOEXEC);
        }
    }
}
