use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex};

use biometrics::{Collector, Counter};
use boring::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use buffertk::{stack_pack, Unpackable};
use indicio::{clue, INFO};
use rpc_pb::{Context, Error, Host, Request, Response, Status};
use zerror_core::ErrorCore;

use super::builtins;
use super::channel::Channel;
use super::poll::{default_pollster, Pollster, POLLERR, POLLHUP, POLLIN, POLLOUT};
use super::LOGGING;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static DO_ACCEPT: Counter = Counter::new("busyrpc.server.accept");
static POLL_SUCCESS: Counter = Counter::new("busyrpc.server.poll");
static POLL_ERROR: Counter = Counter::new("busyrpc.server.poll.error");
static GET_CHANNEL: Counter = Counter::new("busyrpc.server.channel.get");
static GET_CHANNEL_FAILED: Counter = Counter::new("busyrpc.server.channel.get.error");
static ADD_CHANNEL_ARM_FAILED: Counter = Counter::new("busyrpc.server.channel.arm.error");
static ADD_CHANNEL_RECV_FAILED: Counter = Counter::new("busyrpc.server.channel.recv.error");
static CANCEL_CHANNEL: Counter = Counter::new("busyrpc.server.channel.cancel");
static SEND_FAILED: Counter = Counter::new("busyrpc.server.send_failed");
static RECV_FAILED: Counter = Counter::new("busyrpc.server.recv_failed");
static NEEDS_WRITE: Counter = Counter::new("busyrpc.server.needs_write");
static UNKNOWN_SERVER_NAME: Counter = Counter::new("busyrpc.server.needs_write");
static HANDLE_RPC: Counter = Counter::new("busyrpc.server.handle_rpc");
static HANDLE_RPC_FAILED: Counter = Counter::new("busyrpc.server.handle_rpc.error");
static SAW_POLLIN: Counter = Counter::new("busyrpc.server.pollin");
static SAW_POLLOUT: Counter = Counter::new("busyrpc.server.pollout");
static SAW_POLLERRHUP: Counter = Counter::new("busyrpc.server.pollerrhup");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&DO_ACCEPT);
    collector.register_counter(&POLL_SUCCESS);
    collector.register_counter(&POLL_ERROR);
    collector.register_counter(&GET_CHANNEL);
    collector.register_counter(&GET_CHANNEL_FAILED);
    collector.register_counter(&ADD_CHANNEL_ARM_FAILED);
    collector.register_counter(&ADD_CHANNEL_RECV_FAILED);
    collector.register_counter(&CANCEL_CHANNEL);
    collector.register_counter(&SEND_FAILED);
    collector.register_counter(&RECV_FAILED);
    collector.register_counter(&NEEDS_WRITE);
    collector.register_counter(&UNKNOWN_SERVER_NAME);
    collector.register_counter(&HANDLE_RPC);
    collector.register_counter(&HANDLE_RPC_FAILED);
    collector.register_counter(&SAW_POLLIN);
    collector.register_counter(&SAW_POLLOUT);
    collector.register_counter(&SAW_POLLERRHUP);
}

/////////////////////////////////////////// ServerOptions //////////////////////////////////////////

/// RPC Server options.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "binaries", derive(arrrg_derive::CommandLine))]
pub struct ServerOptions {
    /// SSL/TLS ca_file.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the CA certificate."))]
    pub ca_file: String,
    /// SSL/TLS private key.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the private key file."))]
    pub private_key_file: String,
    /// SSL/TLS certificate.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the certificate file."))]
    pub certificate_file: String,
    /// Bind-to this host.
    #[cfg_attr(
        feature = "binaries",
        arrrg(required, "Host to bind to in host:ID=host:port format.")
    )]
    pub bind_to: Host,
    /// Number of threads to spawn.
    #[cfg_attr(feature = "binaries", arrrg(optional, "Number of threads to spawn."))]
    pub thread_pool_size: u16,
    /// Send-buffer size.
    // Buffering preferences.
    #[cfg_attr(feature = "binaries", arrrg(optional, "Userspace send buffer size."))]
    pub user_send_buffer_size: usize,
}

impl ServerOptions {
    /// Build the SSL acceptor or die trying.
    pub fn must_build_acceptor(&self) -> SslAcceptor {
        // Setup our SSL preferences.
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor
            .set_ca_file(&self.ca_file)
            .expect("invalid ca file");
        acceptor
            .set_private_key_file(&self.private_key_file, SslFiletype::PEM)
            .expect("invalid private key");
        acceptor
            .set_certificate_file(&self.certificate_file, SslFiletype::PEM)
            .expect("invalid certificate");
        acceptor.check_private_key().expect("invalid private key");
        acceptor.set_verify(
            boring::ssl::SslVerifyMode::PEER | boring::ssl::SslVerifyMode::FAIL_IF_NO_PEER_CERT,
        );
        acceptor.build()
    }

    /// Get the pollster.
    pub fn pollster(&self) -> Result<Box<dyn Pollster>, rpc_pb::Error> {
        default_pollster()
    }

    /// Set the ca_file.
    pub fn with_ca_file(mut self, ca_file: &str) -> Self {
        ca_file.clone_into(&mut self.ca_file);
        self
    }

    /// Set the private_key_file.
    pub fn with_private_key_file(mut self, private_key_file: &str) -> Self {
        private_key_file.clone_into(&mut self.private_key_file);
        self
    }

    /// Set the certificate_file.
    pub fn with_certificate_file(mut self, certificate_file: &str) -> Self {
        certificate_file.clone_into(&mut self.certificate_file);
        self
    }

    /// Set the bind_to_host.
    pub fn with_bind_to(mut self, bind_to: &Host) -> Self {
        bind_to.clone_into(&mut self.bind_to);
        self
    }

    /// Set the thread_pool_size.
    pub fn with_thread_pool_size(mut self, thread_pool_size: u16) -> Self {
        self.thread_pool_size = thread_pool_size;
        self
    }

    /// Set the user_send_buffer_size.
    pub fn with_user_send_buffer_size(mut self, user_send_buffer_size: usize) -> Self {
        self.user_send_buffer_size = user_send_buffer_size;
        self
    }
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            ca_file: "UNSET".to_string(),
            private_key_file: "UNSET".to_string(),
            certificate_file: "UNSET".to_string(),
            bind_to: Host::default(),
            thread_pool_size: 64,
            user_send_buffer_size: 65536,
        }
    }
}

///////////////////////////////////////////// Internals ////////////////////////////////////////////

struct Internals {
    channels: Mutex<HashMap<RawFd, Arc<Mutex<Channel>>>>,
    services: ServiceRegistry,
    pollster: Box<dyn Pollster>,
    canceled: OwnedFd,
}

impl Internals {
    fn new(pollster: Box<dyn Pollster>, services: ServiceRegistry, canceled: OwnedFd) -> Arc<Self> {
        Arc::new(Self {
            channels: Mutex::default(),
            services,
            pollster,
            canceled,
        })
    }

    fn serve_rpc_thread(self: Arc<Internals>) {
        'serving: loop {
            let (fd, mut events) = match self.pollster.poll(10_000) {
                Ok(Some((fd, ev))) => (fd, ev),
                Ok(None) => {
                    continue 'serving;
                }
                Err(_err) => {
                    POLL_ERROR.click();
                    //Trace::new("busyrpc.poll.error")
                    //    .with_value::<message<Error>, 1>(err)
                    //    .finish();
                    continue 'serving;
                }
            };
            if fd == self.canceled.as_raw_fd() {
                break 'serving;
            }
            POLL_SUCCESS.click();
            let chan = match self.get_channel(fd) {
                Some(chan) => chan,
                None => {
                    GET_CHANNEL_FAILED.click();
                    //Trace::new("busyrpc.poll.get_channel_failed").finish();
                    continue 'serving;
                }
            };
            let mut chan_guard = chan.lock().unwrap();
            if events & POLLOUT != 0 {
                SAW_POLLOUT.click();
                events &= !POLLOUT;
                if let Err(_err) = chan_guard.do_send_work() {
                    SEND_FAILED.click();
                    //Trace::new("busyrpc.send.error")
                    //    .with_value::<message<Error>, 1>(err)
                    //    .finish();
                    events = POLLERR;
                } else if chan_guard.needs_write() {
                    NEEDS_WRITE.click();
                    events |= POLLOUT
                }
            }
            if events & POLLIN != 0 {
                SAW_POLLIN.click();
                if self.do_recv_work(&mut chan_guard) {
                    events = POLLERR;
                }
            }
            events |= POLLIN;
            if events & (POLLERR | POLLHUP) != 0 {
                SAW_POLLERRHUP.click();
                let fd = chan_guard.as_raw_fd();
                drop(chan_guard);
                self.cancel_channel(fd);
            } else if let Err(_err) = self.pollster.arm(fd, events & POLLOUT != 0) {
                POLL_ERROR.click();
                //Trace::new("busyrpc.poll.error")
                //    .with_value::<message<Error>, 1>(err)
                //    .finish();
            }
        }
    }

    fn add_channel(&self, channel: Channel) {
        let fd = channel.as_raw_fd();
        let channel = Arc::new(Mutex::new(channel));
        let channel_p = Arc::clone(&channel);
        self.channels.lock().unwrap().insert(fd, channel);
        let mut chan_guard = channel_p.lock().unwrap();
        if let Err(_err) = self.pollster.arm(fd, true) {
            ADD_CHANNEL_ARM_FAILED.click();
            //Trace::new("busyrpc.poll.error")
            //    .with_value::<message<rpc_pb::Error>, 1>(err)
            //    .finish();
        };
        if self.do_recv_work(&mut chan_guard) {
            ADD_CHANNEL_RECV_FAILED.click();
            self.channels.lock().unwrap().remove(&fd);
        } else if let Err(_err) = self.pollster.arm(fd, true) {
            POLL_ERROR.click();
            //Trace::new("busyrpc.poll.error")
            //    .with_value::<message<Error>, 1>(err)
            //    .finish();
        }
    }

    fn get_channel(&self, fd: RawFd) -> Option<Arc<Mutex<Channel>>> {
        GET_CHANNEL.click();
        self.channels.lock().unwrap().get(&fd).cloned()
    }

    fn cancel_channel(&self, fd: RawFd) {
        CANCEL_CHANNEL.click();
        self.channels.lock().unwrap().remove(&fd);
    }

    // Returns true on failure.
    fn do_recv_work(&self, chan: &mut Channel) -> bool {
        let mut buffers: Vec<Vec<u8>> = Vec::new();
        let buffers_mut = &mut buffers;
        let f = |buf| buffers_mut.push(buf);
        let mut error = false;
        match chan.do_recv_work(f) {
            Ok(_) => {}
            Err(_err) => {
                RECV_FAILED.click();
                //Trace::new("busyrpc.recv.error")
                //    .with_value::<message<Error>, 1>(err)
                //    .finish();
                error = true;
            }
        };
        for buffer in buffers.into_iter() {
            if let Err(_err) = self.handle_rpc(chan, buffer) {
                HANDLE_RPC_FAILED.click();
                //Trace::new("busyrpc.rpc.error")
                //    .with_value::<message<Error>, 1>(err)
                //    .finish();
                error = true;
            }
        }
        error
    }

    fn handle_rpc(&self, chan: &mut Channel, msg: Vec<u8>) -> Result<(), Error> {
        HANDLE_RPC.click();
        let req = Request::unpack(&msg)?.0;
        let ctx = Context::from(&req);
        let server = match self.services.get_server(req.service) {
            Some(server) => server,
            None => {
                UNKNOWN_SERVER_NAME.click();
                let err = Error::UnknownServerName {
                    core: ErrorCore::default(),
                    name: req.service.to_string(),
                };
                return self.handle_error(chan, req, err);
            }
        };
        let resp: Status = server.call(&ctx, req.method, req.body);
        self.handle_status(chan, req, resp)
    }

    fn handle_error(&self, chan: &mut Channel, req: Request, err: Error) -> Result<(), Error> {
        let status = Err(err);
        self.handle_status(chan, req, status)
    }

    fn handle_status(&self, chan: &mut Channel, req: Request, status: Status) -> Result<(), Error> {
        let err_buf: Vec<u8>;
        let (body, service_error, rpc_error) = match &status {
            Ok(Ok(body)) => {
                let body: &[u8] = body;
                (Some(body), None, None)
            }
            Ok(Err(err)) => {
                let err: &[u8] = err;
                (None, Some(err), None)
            }
            Err(err) => {
                err_buf = stack_pack(err).to_vec();
                let err_buf: &[u8] = &err_buf;
                (None, None, Some(err_buf))
            }
        };
        let resp = Response {
            seq_no: req.seq_no,
            trace: req.trace,
            body,
            service_error,
            rpc_error,
        };
        let resp_buf = stack_pack(resp).to_vec();
        chan.send(&resp_buf)
    }
}

////////////////////////////////////////// ServiceRegistry /////////////////////////////////////////

/// ServiceRegistry maps servers by name.
// TODO(rescrv):  Dedupe with rpc_pb's similar struct.
pub struct ServiceRegistry {
    services: HashMap<&'static str, Box<dyn rpc_pb::Server + Send + Sync + 'static>>,
}

impl ServiceRegistry {
    /// Create a new ServiceRegistry.
    pub fn new() -> Self {
        let mut services = Self {
            services: HashMap::new(),
        };
        let builtins = builtins::BuiltinService::new();
        services.register("__builtins__", builtins::BuiltinServer::bind(builtins));
        services
    }

    /// Register the server with this ServiceRegistry.
    pub fn register<S: rpc_pb::Server + Send + Sync + 'static>(
        &mut self,
        service: &'static str,
        server: S,
    ) {
        if self.services.contains_key(service) {
            panic!("cannot add the same service twice");
        }
        self.services.insert(service, Box::new(server));
    }

    fn get_server(&self, service: &str) -> Option<&(dyn rpc_pb::Server + Send + Sync + 'static)> {
        match self.services.get(service) {
            Some(server) => Some(server.as_ref()),
            None => None,
        }
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

////////////////////////////////////////////// Server //////////////////////////////////////////////

/// An RPC Server hosts multiple server instances so they may be called.
pub struct Server {
    options: ServerOptions,
    internals: Arc<Internals>,
}

impl Server {
    /// Create a new server from the options and service registry.
    pub fn new(
        options: ServerOptions,
        services: ServiceRegistry,
    ) -> Result<(Self, impl FnOnce()), rpc_pb::Error> {
        let pollster = options.pollster()?;
        let mut fds: [libc::c_int; 2] = [-1; 2];
        // SAFETY(rescrv):  We are passing a suitably-sized array of ints.
        unsafe {
            if libc::pipe(&mut fds as *mut libc::c_int) < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        // SAFETY(rescrv):  We were just handed this file descriptor.
        let watch = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        pollster.arm_forever(watch.as_raw_fd())?;
        let cancel = move || {
            // SAFETY(rescrv): We will be the only ones to call close, and the function is returned
            // as a FnOnce.  Trust in the type system from there.
            unsafe {
                libc::close(fds[1]);
            }
        };
        let internals = Internals::new(pollster, services, watch);
        Ok((Self { options, internals }, cancel))
    }

    /// Return the Host to which this server is bound.
    pub fn host(&self) -> Host {
        self.options.bind_to.clone()
    }

    /// Serve the server forever.
    pub fn serve(&self) -> Result<(), Error> {
        // Spawn threads to serve the thread pool.
        let mut threads = Vec::new();
        for _ in 0..self.options.thread_pool_size {
            let internals = Arc::clone(&self.internals);
            threads.push(std::thread::spawn(move || {
                internals.serve_rpc_thread();
            }));
        }
        // SSL/TLS acceptor
        let acceptor = Arc::new(self.options.must_build_acceptor());
        // Listen for incoming connections.
        let listener = TcpListener::bind(self.options.bind_to.connect()).map_err(|err| {
            rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: err.to_string(),
            }
        })?;
        'listening: loop {
            let break_fd = self.internals.canceled.as_raw_fd();
            let mut pfd = [
                libc::pollfd {
                    fd: break_fd,
                    events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                    revents: 0,
                },
                libc::pollfd {
                    fd: listener.as_raw_fd(),
                    events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                    revents: 0,
                },
            ];
            let ret = unsafe { libc::poll(pfd.as_mut_ptr(), 2, -1) };
            if pfd[0].revents != 0 {
                clue!(LOGGING, INFO, {
                    serve: {
                        canceled: true,
                    },
                });
                break;
            }
            if ret < 0 {
                clue!(LOGGING, INFO, {
                    serve: {
                        error: true,
                    },
                });
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let acceptor = acceptor.clone();
                    let stream = match acceptor.accept(stream) {
                        Ok(stream) => stream,
                        Err(err) => {
                            let _err = rpc_pb::Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: err.to_string(),
                            };
                            //Trace::new("busyrpc.accept.error")
                            //    .with_value::<message<Error>, 1>(err)
                            //    .finish();
                            continue 'listening;
                        }
                    };
                    DO_ACCEPT.click();
                    match self.add_channel(stream) {
                        Ok(_) => {}
                        Err(_err) => {
                            //Trace::new("busyrpc.add_channel.error")
                            //    .with_value::<message<Error>, 1>(err)
                            //    .finish();
                            continue 'listening;
                        }
                    };
                }
                Err(_err) => {
                    //Trace::new("busyrpc.listen.error")
                    //    .with_value::<message<Error>, 1>(err.into())
                    //    .finish();
                }
            }
        }
        for thread in threads.into_iter() {
            _ = thread.join();
        }
        Ok(())
    }

    fn add_channel(&self, stream: SslStream<TcpStream>) -> Result<(), rpc_pb::Error> {
        let channel = Channel::new(stream, self.options.user_send_buffer_size)?;
        let internals = Arc::clone(&self.internals);
        internals.add_channel(channel);
        Ok(())
    }
}
