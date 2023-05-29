use std::collections::hash_map::HashMap;
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use buffertk::{stack_pack, Buffer, Unpackable};

use prototk::field_types::message;
use prototk_derive::Message;

use biometrics::{Counter, Collector};

use indicio::Trace;

use zerror_core::ErrorCore;

use rpc_pb::{Context, Error, Request, Response, Status};

use rivulet::{RecvChannel, SendChannel, ThreadState, Poll, ProcessEvents, POLLIN, POLLOUT, POLLERR, POLLHUP};

use boring::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static SERVER_NEW: Counter = Counter::new("busybee.server.new");
static SERVER_SERVE: Counter = Counter::new("busybee.server.serve");
static SERVE_THREAD_SERVING: Counter = Counter::new("busybee.server.serving");
static SERVE_THREAD_TIMEOUT: Counter = Counter::new("busybee.server.timeout");
static SERVE_THREAD_POLLIN: Counter = Counter::new("busybee.server.pollin");
static SERVE_THREAD_POLLOUT: Counter = Counter::new("busybee.server.pollout");
static SERVE_THREAD_CONSERVE: Counter = Counter::new("busybee.server.conserve");

static ADOPT_STREAM: Counter = Counter::new("busybee.adopt_stream");
static HANDLE_RPC: Counter = Counter::new("busybee.handle_rpc");
static HANDLE_RPC_CONSERVE: Counter = Counter::new("busybee.handle_rpc.conserve");
static HANDLE_RPC_DONE: Counter = Counter::new("busybee.handle_rpc.done");
static HANDLE_ERROR: Counter = Counter::new("busybee.handle_error");
static HANDLE_STATUS: Counter = Counter::new("busybee.handle_status");

static CHANNEL_NEW: Counter = Counter::new("busybee.channel.new");
static CHANNEL_FROM_PARTS: Counter = Counter::new("busybee.channel.from_parts");
static CHANNEL_POISON: Counter = Counter::new("busybee.channel.poison");

static UNKNOWN_SERVER: Counter = Counter::new("busybee.unknown_server");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&SERVER_NEW);
    collector.register_counter(&SERVER_SERVE);
    collector.register_counter(&SERVE_THREAD_SERVING);
    collector.register_counter(&SERVE_THREAD_TIMEOUT);
    collector.register_counter(&SERVE_THREAD_POLLIN);
    collector.register_counter(&SERVE_THREAD_POLLOUT);
    collector.register_counter(&SERVE_THREAD_CONSERVE);
    collector.register_counter(&ADOPT_STREAM);
    collector.register_counter(&HANDLE_RPC);
    collector.register_counter(&HANDLE_RPC_CONSERVE);
    collector.register_counter(&HANDLE_RPC_DONE);
    collector.register_counter(&HANDLE_ERROR);
    collector.register_counter(&HANDLE_STATUS);
    collector.register_counter(&CHANNEL_NEW);
    collector.register_counter(&CHANNEL_FROM_PARTS);
    collector.register_counter(&CHANNEL_POISON);
    collector.register_counter(&UNKNOWN_SERVER);
}

////////////////////////////////////////////// Channel /////////////////////////////////////////////

#[derive(Debug)]
struct Channel {
    recv: Mutex<RecvChannel>,
    send: Mutex<SendChannel>,
    poison: Mutex<Option<Error>>,
}

impl Channel {
    fn new(stream: SslStream<TcpStream>) -> Result<Channel, Error> {
        CHANNEL_NEW.click();
        let (r, s) = rivulet::from_stream(stream)?;
        Ok(Channel::from_parts(r, s))
    }

    fn from_parts(recv: RecvChannel, send: SendChannel) -> Channel {
        CHANNEL_FROM_PARTS.click();
        Channel {
            recv: Mutex::new(recv),
            send: Mutex::new(send),
            poison: Mutex::new(None),
        }
    }

    fn poison(&self, err: Error) -> Result<(), Error> {
        CHANNEL_POISON.click();
        let mut guard = self.poison.lock().unwrap();
        if guard.is_none() {
            *guard = Some(err.clone())
        }
        Err(err)
    }
}

impl AsRawFd for Channel {
    fn as_raw_fd(&self) -> i32 {
        self.recv.lock().unwrap().as_raw_fd()
    }
}

/////////////////////////////////////////// ServerOptions //////////////////////////////////////////

pub struct ServerOptions {
    // SSL/TLS preferences.
    ca_file: String,
    private_key_file: String,
    certificate_file: String,
    // Server-side preferences.
    bind_to_host: String,
    bind_to_port: u16,
    thread_pool_size: u16,
    // Servers that are registered.
    // NOTE(rescrv):  I initially prototyped having options be separate.  What manifests looks
    // strictly uglier than having servers be part of the ServerOptions.
    servers: HashMap<String, Box<dyn rpc_pb::Server + Send + Sync + 'static>>,
}

impl ServerOptions {
    fn must_build_acceptor(&self) -> SslAcceptor {
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
        // TODO(rescrv):  Production blocker.
        acceptor.set_verify(boring::ssl::SslVerifyMode::NONE);
        acceptor.build()
    }

    pub fn with_ca_file(mut self, ca_file: &str) -> Self {
        self.ca_file = ca_file.to_owned();
        self
    }

    pub fn with_private_key_file(mut self, private_key_file: &str) -> Self {
        self.private_key_file = private_key_file.to_owned();
        self
    }

    pub fn with_certificate_file(mut self, certificate_file: &str) -> Self {
        self.certificate_file = certificate_file.to_owned();
        self
    }

    pub fn with_bind_to_host(mut self, bind_to_host: &str) -> Self {
        self.bind_to_host = bind_to_host.to_owned();
        self
    }

    pub fn with_bind_to_port(mut self, bind_to_port: u16) -> Self {
        self.bind_to_port = bind_to_port;
        self
    }

    pub fn with_thread_pool_size(mut self, thread_pool_size: u16) -> Self {
        self.thread_pool_size = thread_pool_size;
        self
    }

    pub fn with_server(mut self, name: &str, server: Box<dyn rpc_pb::Server + Send + Sync + 'static>) {
        assert!(!self.servers.contains_key(name));
        assert!(name.starts_with("__"));
        self.servers.insert(name.to_owned(), server);
    }
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            ca_file: "".to_owned(),
            private_key_file: "".to_owned(),
            certificate_file: "".to_owned(),
            bind_to_host: "localhost".to_owned(),
            bind_to_port: 2049,
            thread_pool_size: 64,
            servers: HashMap::new(),
        }
    }
}

///////////////////////////////////////////// BuiltIns /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Empty {}

rpc_pb::service! {
    name = Builtin;
    server = BuiltinServer;
    client = BuiltinClient;
    error = Error;

    rpc nop(Empty) -> Empty;
}

#[derive(Debug, Default)]
struct BuiltinService {
}

impl Builtin for BuiltinService {
    fn nop(&self, _: &Context, _: Empty) -> Result<Empty, Error> {
        Ok(Empty {})
    }
}

///////////////////////////////////////////// Internals ////////////////////////////////////////////

struct Internals {
    servers: HashMap<String, Box<dyn rpc_pb::Server + Send + Sync + 'static>>,
    streams: Mutex<HashMap<RawFd, Arc<Channel>>>,
    poll: Box<dyn Poll>,
}

impl Internals {
    fn new(poll: Box<dyn Poll>) -> Self {
        let mut servers: HashMap<String, _> = HashMap::default();
        let builtins: Box<dyn rpc_pb::Server + Send + Sync + 'static> =
            Box::new(BuiltinServer::bind(BuiltinService::default()));
        servers.insert("__builtins__".to_owned(), builtins);
        Self {
            servers,
            streams: Mutex::new(HashMap::default()),
            poll,
        }
    }

    fn serve_thread(self: Arc<Internals>) {
        let mut ts = self.poll.new_thread();
        'serving:
        loop {
            SERVE_THREAD_SERVING.click();
            let (fd, mut events) = match self.poll.poll(&mut ts, 10_000) {
                Ok(Some((fd, ev))) => (fd, ev),
                Ok(None) => {
                    SERVE_THREAD_TIMEOUT.click();
                    continue 'serving;
                }
                Err(err) => {
                    Trace::new("busybee.poll.error")
                        .with_value::<message<Error>, 1>(err)
                        .finish();
                    todo!();
                },
            };
            let chan = match self.get_channel(fd) {
                Some(chan) => chan,
                None => {
                    Trace::new("busybee.poll.get_channel_failed").finish();
                    continue 'serving;
                }
            };
            if events & POLLOUT != 0 {
                SERVE_THREAD_POLLOUT.click();
                match chan.send.lock().unwrap().process_events(&mut events) {
                    Ok(Some(msg)) => {
                        let c = Arc::clone(&chan);
                        if let Err(err) = self.handle_rpc(&mut ts, c, msg) {
                            Trace::new("busybee.channel.error")
                                .with_value::<message<Error>, 1>(err.clone())
                                .finish();
                            _ = chan.poison(err);
                        }
                    },
                    Ok(None) => {},
                    Err(err) => {
                        Trace::new("busybee.channel.error")
                            .with_value::<message<Error>, 1>(err.clone())
                            .finish();
                        _ = chan.poison(err);
                    }
                };
            }
            if events & POLLIN != 0 {
                SERVE_THREAD_POLLIN.click();
                match chan.recv.lock().unwrap().process_events(&mut events) {
                    Ok(Some(msg)) => {
                        let c = Arc::clone(&chan);
                        if let Err(err) = self.handle_rpc(&mut ts, c, msg) {
                            Trace::new("busybee.channel.error")
                                .with_value::<message<Error>, 1>(err.clone())
                                .finish();
                            _ = chan.poison(err);
                        }
                    },
                    Ok(None) => {},
                    Err(err) => {
                        Trace::new("busybee.channel.error")
                            .with_value::<message<Error>, 1>(err.clone())
                            .finish();
                        _ = chan.poison(err);
                    }
                };
            }
            if events & (POLLERR|POLLHUP) != 0 {
                self.cancel_channel(chan);
                events = 0;
            }
            if events != 0 {
                SERVE_THREAD_CONSERVE.click();
                self.poll.conserve(&mut ts, fd, events);
            }
        }
    }

    fn adopt_stream(self: Arc<Internals>, stream: SslStream<TcpStream>) {
        ADOPT_STREAM.click();
        let fd = stream.get_ref().as_raw_fd();
        let chan = match Channel::new(stream) {
            Ok(chan) => chan,
            Err(err) => {
                Trace::new("busybee.adopt_stream.error")
                    .with_value::<message<Error>, 1>(err)
                    .finish();
                return;
            }
        };
        self.streams.lock().unwrap().insert(fd, Arc::new(chan));
        if let Err(err) = self.poll.insert(fd) {
            Trace::new("busybee.poll.error")
                .with_value::<message<Error>, 1>(err)
                .finish();
        }
    }

    fn get_channel(&self, fd: RawFd) -> Option<Arc<Channel>> {
        self.streams.lock().unwrap().get(&fd).map(Arc::clone)
    }

    fn cancel_channel(&self, chan: Arc<Channel>) {
        let fd = chan.as_raw_fd();
        let mut streams = self.streams.lock().unwrap();
        if let Some(fetched) = streams.get(&fd) {
            if Arc::as_ptr(fetched) == Arc::as_ptr(&chan) {
                streams.remove(&fd);
            }
        }
    }

    fn handle_rpc(&self, ts: &mut ThreadState, chan: Arc<Channel>, msg: Buffer) -> Result<(), Error> {
        HANDLE_RPC.click();
        let req = Request::unpack(msg.as_bytes())?.0;
        let ctx = Context::from(&req);
        let server: &(dyn rpc_pb::Server + Send + Sync + 'static) = match self.servers.get(req.server) {
            Some(server) => server.as_ref(),
            None => {
                UNKNOWN_SERVER.click();
                let err = Error::UnknownServerName {
                    core: ErrorCore::default(),
                    name: req.server.to_string(),
                };
                return self.handle_error(ts, chan, req, err);
            }
        };
        let resp: Status = server.call(&ctx, req.method, req.body);
        self.handle_status(ts, chan, req, resp)
    }

    fn handle_error(&self, ts: &mut ThreadState, chan: Arc<Channel>, req: Request, err: Error) -> Result<(), Error> {
        HANDLE_ERROR.click();
        let status = Err(err);
        self.handle_status(ts, chan, req, status)
    }

    fn handle_status(&self, ts: &mut ThreadState, chan: Arc<Channel>, req: Request, status: Status) -> Result<(), Error> {
        HANDLE_STATUS.click();
        #[allow(unused_assignments)]
        let mut err_buf = Buffer::default();
        let (body, service_error, rpc_error) = match &status {
            Ok(Ok(body)) => { (Some(body.as_bytes()), None, None) },
            Ok(Err(err)) => { (None, Some(err.as_bytes()), None) },
            Err(err) => {
                err_buf = stack_pack(err).to_buffer();
                (None, None, Some(err_buf.as_bytes()))
            },
        };
        let resp = Response {
            seq_no: req.seq_no,
            trace: req.trace,
            body,
            service_error,
            rpc_error,
        };
        let resp_buf = stack_pack(resp).to_buffer();
        let mut send = chan.send.lock().unwrap();
        send.enqueue(resp_buf.as_bytes())?;
        let mut events: u32 = POLLOUT;
        send.flush(&mut events)?;
        if events != 0 {
            HANDLE_RPC_CONSERVE.click();
            self.poll.conserve(ts, chan.as_raw_fd(), events);
        }
        HANDLE_RPC_DONE.click();
        Ok(())
    }
}

////////////////////////////////////////////// Server //////////////////////////////////////////////

pub struct Server {
    options: ServerOptions,
    internals: Mutex<Arc<Internals>>,
}

impl Server {
    pub fn new(options: ServerOptions, poll: Box<dyn Poll>) -> Self {
        SERVER_NEW.click();
        Self {
            options,
            internals: Mutex::new(Arc::new(Internals::new(poll))),
        }
    }

    pub fn serve(&self) -> Result<(), Error> {
        SERVER_SERVE.click();
        // Spawn threads to serve the thread pool.
        let mut threads = Vec::new();
        for _ in 0..self.options.thread_pool_size {
            let internals = self.get_internals();
            threads.push(std::thread::spawn(|| Internals::serve_thread(internals)));
        }
        // SSL/TLS acceptor
        let acceptor = Arc::new(self.options.must_build_acceptor());
        // Listen for incoming connections.
        let bind_to = format!(
            "{}:{}",
            self.options.bind_to_host, self.options.bind_to_port
        );
        let listener =
            TcpListener::bind(bind_to).map_err(|err| rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: err.to_string(),
            })?;
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let acceptor = acceptor.clone();
                    let stream = acceptor.accept(stream).unwrap();
                    self.get_internals().adopt_stream(stream);
                },
                Err(err) => {
                    Trace::new("busybee.listen.error")
                        .with_value::<message<Error>, 1>(err.into())
                        .finish();
                },
            }
        }
        for thread in threads.into_iter() {
            _ = thread.join();
        }
        Ok(())
    }

    fn get_internals(&self) -> Arc<Internals> {
        Arc::clone(&self.internals.lock().unwrap())
    }
}
