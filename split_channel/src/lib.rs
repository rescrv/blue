//! split_channel provides a stream-of-messages abstraction with split send/recv channels.
//! Most calls that establish a channel return a tuple of ([RecvChannel], [SendChannel]).
//!
//! The key idea here is that an `&mut RecvChannel` and `&mut SendChannel` exist for the same
//! `SslStream<TcpStream>`, allowing parallel sending and processing of messages.  The general
//! pattern is to lock the send channel, send data, and then use [sync42::wait_list::WaitList] to
//! synchronize receivers.

use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use boring::ssl::{ErrorCode, SslAcceptor, SslConnector, SslFiletype, SslMethod, SslStream};

use arrrg_derive::CommandLine;

use biometrics::{Collector, Counter, Moments};

use buffertk::{stack_pack, Buffer, Packable, Unpackable};

use utilz::stopwatch::Stopwatch;

use zerror_core::ErrorCore;

use rpc_pb::{Error, Frame};

mod polling;

pub use polling::*;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static CONNECT: Counter = Counter::new("split_channel.connect");
static FROM_STREAM: Counter = Counter::new("split_channel.from_stream");

static MESSAGES_SENT: Counter = Counter::new("split_channel.messages_sent");
static MESSAGES_RECV: Counter = Counter::new("split_channel.messages_recv");

static POLL_ERRORS: Counter = Counter::new("split_channel.poll.errors");
static RECV_ERRORS: Counter = Counter::new("split_channel.recv.errors");
static SEND_ERRORS: Counter = Counter::new("split_channel.send.errors");

static SEND_POLL_LATENCY: Moments = Moments::new("split_channel.send.poll_latency");
static SEND_CALL_LATENCY: Moments = Moments::new("split_channel.send.call_latency");
static SEND_ERROR_LATENCY: Moments = Moments::new("split_channel.send.error_latency");
static RECV_POLL_LATENCY: Moments = Moments::new("split_channel.recv.poll_latency");
static RECV_CALL_LATENCY: Moments = Moments::new("split_channel.recv.call_latency");
static RECV_ERROR_LATENCY: Moments = Moments::new("split_channel.recv.error_latency");

static SEND_WANT_READ: Counter = Counter::new("split_channel.send.want_read");
static SEND_WANT_WRITE: Counter = Counter::new("split_channel.send.want_write");
static RECV_WANT_READ: Counter = Counter::new("split_channel.recv.want_read");
static RECV_WANT_WRITE: Counter = Counter::new("split_channel.recv.want_write");

static SEND_SHRINK_BUF: Counter = Counter::new("split_channel.send.shrink_buf");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&CONNECT);
    collector.register_counter(&FROM_STREAM);
    collector.register_counter(&MESSAGES_SENT);
    collector.register_counter(&MESSAGES_RECV);
    collector.register_counter(&POLL_ERRORS);
    collector.register_counter(&RECV_ERRORS);
    collector.register_counter(&SEND_ERRORS);
    collector.register_moments(&SEND_POLL_LATENCY);
    collector.register_moments(&SEND_CALL_LATENCY);
    collector.register_moments(&SEND_ERROR_LATENCY);
    collector.register_moments(&RECV_POLL_LATENCY);
    collector.register_moments(&RECV_CALL_LATENCY);
    collector.register_moments(&RECV_ERROR_LATENCY);
    collector.register_counter(&SEND_WANT_READ);
    collector.register_counter(&SEND_WANT_WRITE);
    collector.register_counter(&RECV_WANT_READ);
    collector.register_counter(&RECV_WANT_WRITE);
    polling::register_biometrics(collector);
}

/////////////////////////////////////////// ChannelState ///////////////////////////////////////////

#[derive(Debug)]
struct ChannelState {
    stream: SslStream<TcpStream>,
}

impl AsRawFd for ChannelState {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.get_ref().as_raw_fd()
    }
}

///////////////////////////////////////////// WorkDone /////////////////////////////////////////////

#[derive(Debug, Default)]
enum WorkDone {
    #[default]
    ReadRequisiteAmount,
    EncounteredEagain,
    Error(Error),
}

/////////////////////////////////////////// ProcessEvents //////////////////////////////////////////

pub trait ProcessEvents {
    fn process_events(&mut self, events: &mut u32) -> Result<Option<Buffer>, Error>;
}

//////////////////////////////////////////// RecvChannel ///////////////////////////////////////////

#[derive(Debug)]
pub struct RecvChannel {
    state: Arc<Mutex<ChannelState>>,
    recv_buf: Vec<u8>,
    recv_idx: usize,
}

impl RecvChannel {
    pub fn recv(&mut self) -> Result<Buffer, Error> {
        loop {
            let mut events = POLLIN;
            if let Some(buf) = self.process_events(&mut events)? {
                return Ok(buf);
            }
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let sw = Stopwatch::default();
            unsafe {
                if libc::poll(&mut pfd, 1, 30_000) < 0 {
                    POLL_ERRORS.click();
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            RECV_POLL_LATENCY.add(sw.since());
        }
    }

    fn work_recv(&mut self, require: usize, events: &mut u32) -> WorkDone {
        loop {
            let amt = std::cmp::max(require, 4096);
            if self.recv_buf.len() - self.recv_idx < amt {
                self.recv_buf.resize(self.recv_idx + amt, 0);
            }
            let sw = Stopwatch::default();
            let mut state = self.state.lock().unwrap();
            match state.stream.ssl_read(&mut self.recv_buf[self.recv_idx..]) {
                Ok(sz) => {
                    RECV_CALL_LATENCY.add(sw.since());
                    self.recv_idx += sz;
                    if self.recv_idx >= require {
                        return WorkDone::ReadRequisiteAmount;
                    }
                },
                Err(err) => {
                    match err.code() {
                        ErrorCode::WANT_READ => {
                            RECV_WANT_READ.click();
                            *events &= !POLLIN;
                            return WorkDone::EncounteredEagain;
                        },
                        ErrorCode::WANT_WRITE => {
                            RECV_WANT_WRITE.click();
                            return WorkDone::EncounteredEagain;
                        },
                        _ => {
                            RECV_ERROR_LATENCY.add(sw.since());
                            RECV_ERRORS.click();
                            let err = Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: err.to_string(),
                            };
                            return WorkDone::Error(err);
                        },
                    }
                },
            }
        }
    }
}

impl ProcessEvents for RecvChannel {
    fn process_events(&mut self, events: &mut u32) -> Result<Option<Buffer>, Error> {
        if self.recv_idx == 0 {
            match self.work_recv(1, events) {
                WorkDone::ReadRequisiteAmount => {},
                WorkDone::EncounteredEagain => {
                    return Ok(None);
                },
                WorkDone::Error(err) => {
                    return Err(err);
                }
            }
        }
        assert!(!self.recv_buf.is_empty());
        assert!(self.recv_idx > 0);
        let frame_sz = self.recv_buf[0] as usize;
        if frame_sz + 1 > self.recv_idx {
            match self.work_recv(frame_sz + 1, events) {
                WorkDone::ReadRequisiteAmount => {},
                WorkDone::EncounteredEagain => {
                    return Ok(None);
                },
                WorkDone::Error(err) => {
                    return Err(err);
                }
            }
        }
        let (frame, _): (Frame, &[u8]) = Frame::unpack(&self.recv_buf[1..1 + frame_sz])?;
        let start = 1 + frame_sz;
        let limit = start + frame.size as usize;
        if self.recv_idx < limit {
            match self.work_recv(limit, events) {
                WorkDone::ReadRequisiteAmount => {},
                WorkDone::EncounteredEagain => {
                    return Ok(None);
                },
                WorkDone::Error(err) => {
                    return Err(err);
                }
            }
        }
        let buf = Buffer::from(&self.recv_buf[start..limit]);
        if crc32c::crc32c(buf.as_bytes()) != frame.crc32c {
            RECV_ERRORS.click();
            return Err(Error::TransportFailure {
                core: ErrorCore::default(),
                what: "crc32c failed".to_string(),
            });
        }
        self.recv_buf.rotate_left(limit);
        self.recv_buf.shrink_to(self.recv_buf.len() - limit);
        self.recv_idx -= limit;
        MESSAGES_RECV.click();
        Ok(Some(buf))
    }
}

impl AsRawFd for RecvChannel {
    fn as_raw_fd(&self) -> RawFd {
        self.state.lock().unwrap().as_raw_fd()
    }
}

//////////////////////////////////////////// SendChannel ///////////////////////////////////////////

#[derive(Debug)]
pub struct SendChannel {
    state: Arc<Mutex<ChannelState>>,
    send_buf: Vec<u8>,
    send_idx: usize,
}

impl SendChannel {
    pub fn send(&mut self, body: &[u8]) -> Result<(), Error> {
        // send piggy backs on enqueues counters
        self.enqueue(body)?;
        self.blocking_drain()?;
        Ok(())
    }

    pub fn enqueue(&mut self, body: &[u8]) -> Result<(), Error> {
        let frame = Frame {
            size: body.len() as u64,
            crc32c: crc32c::crc32c(body),
        };
        assert!(frame.pack_sz() < 256);
        self.send_buf.push(frame.pack_sz() as u8);
        stack_pack(frame).append_to_vec(&mut self.send_buf);
        self.send_buf.extend_from_slice(body);
        MESSAGES_SENT.click();
        Ok(())
    }

    // Flush until either the flushing would block or the buffer is entirely flushed.
    pub fn flush(&mut self, events: &mut u32) -> Result<(), Error> {
        while self.send_idx < self.send_buf.len() {
            let buf = &self.send_buf[self.send_idx..];
            let sw = Stopwatch::default();
            let mut state = self.state.lock().unwrap();
            match state.stream.ssl_write(buf) {
                Ok(sz) => {
                    SEND_CALL_LATENCY.add(sw.since());
                    self.send_idx += sz;
                },
                Err(err) => {
                    SEND_ERROR_LATENCY.add(sw.since());
                    match err.code() {
                        ErrorCode::WANT_READ => {
                            SEND_WANT_READ.click();
                            *events |= POLLIN;
                            return Ok(());
                        },
                        ErrorCode::WANT_WRITE => {
                            SEND_WANT_WRITE.click();
                            *events &= !POLLOUT;
                            return Ok(());
                        },
                        _ => {
                            SEND_ERRORS.click();
                            return Err(Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: err.to_string(),
                            });
                        },
                    }
                },
            };
        }
        *events &= !POLLOUT;
        self.send_buf.clear();
        self.send_idx = 0;
        Ok(())
    }

    pub fn blocking_drain(&mut self) -> Result<(), Error> {
        'draining:
        while self.send_idx < self.send_buf.len() {
            let mut events = POLLOUT;
            self.process_events(&mut events)?;
            if self.send_idx >= self.send_buf.len() {
                continue 'draining;
            }
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: to_poll_constants(events),
                revents: 0,
            };
            let sw = Stopwatch::default();
            unsafe {
                if libc::poll(&mut pfd, 1, 5_000) < 0 {
                    SEND_ERRORS.click();
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            SEND_POLL_LATENCY.add(sw.since());
            if self.send_idx > 64 && self.send_idx >= self.send_buf.len() / 2 {
                SEND_SHRINK_BUF.click();
                let size = self.send_buf.len() - self.send_idx;
                self.send_buf.rotate_left(self.send_idx);
                self.send_buf.shrink_to(size);
                self.send_idx = 0;
            }
        }
        self.send_buf.clear();
        self.send_idx = 0;
        Ok(())
    }
}

impl ProcessEvents for SendChannel {
    fn process_events(&mut self, events: &mut u32) -> Result<Option<Buffer>, Error> {
        self.flush(events)?;
        Ok(None)
    }
}

///////////////////////////////////////////// Listener /////////////////////////////////////////////

pub struct Listener {
    acceptor: Arc<SslAcceptor>,
    listener: TcpListener,
}

impl Iterator for Listener {
    type Item = Result<(RecvChannel, SendChannel), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let (stream, addr) = match self.listener.accept() {
            Ok((stream, addr)) => (stream, addr),
            Err(err) => {
                return Some(Err(err.into()));
            },
        };
        // TODO(rescrv): Log to indicio with the accepted addr.
        _ = addr;
        let stream = match self.acceptor.accept(stream) {
            Ok(stream) => stream,
            Err(err) => {
                return Some(Err(Error::TransportFailure {
                    core: ErrorCore::default(),
                    what: err.to_string(),
                }));
            },
        };
        let (recv_chan, send_chan) = from_stream(stream).expect("channel from stream");
        Some(Ok((recv_chan, send_chan)))
    }
}

//////////////////////////////////////// SplitChannelOptions ///////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Default, Eq, PartialEq)]
pub struct SplitChannelOptions {
    #[arrrg(required, "Hostname to use.", "HOST")]
    host: String,
    #[arrrg(required, "Port to use.", "PORT")]
    port: u16,
    #[arrrg(optional, "Set CA file.", "PEM")]
    ca_file: Option<String>,
    #[arrrg(optional, "Set private-key file.", "PEM")]
    private_key_file: Option<String>,
    #[arrrg(optional, "Set certificate file.", "PEM")]
    certificate_file: Option<String>,
    #[arrrg(flag, "Set SSL verify mode to None.  Useful when certificates don't match.")]
    verify_none: bool,
}

impl SplitChannelOptions {
    pub fn connect(&self) -> Result<(RecvChannel, SendChannel), Error> {
        CONNECT.click();
        let mut builder =
            SslConnector::builder(SslMethod::tls()).map_err(|err| Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("could not build connector builder: {}", err),
            })?;
        if let Some(ca_file) = &self.ca_file {
            builder.set_ca_file(ca_file).expect("set_ca_file");
        }
        if let Some(private_key_file) = &self.private_key_file {
            builder.set_private_key_file(private_key_file, SslFiletype::PEM).expect("private_key_file");
        }
        if let Some(certificate_file) = &self.certificate_file {
            builder.set_certificate_file(certificate_file, SslFiletype::PEM).expect("private_key_file");
        }
        if self.private_key_file.is_some() && self.certificate_file.is_some() {
            builder.check_private_key().expect("invalid private key");
        }
        if self.verify_none {
            builder.set_verify(boring::ssl::SslVerifyMode::NONE);
        }
        let connector = builder.build();
        let stream = TcpStream::connect(format!("{}:{}", self.host, self.port))?;
        let stream =
            connector
                .connect(&self.host, stream)
                .map_err(|err| Error::TransportFailure {
                    core: ErrorCore::default(),
                    what: format!("{}", err),
                })?;
        from_stream(stream)
    }

    pub fn bind_to(&self) -> Result<Listener, Error> {
        // Create the SSL acceptor state.
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        if let Some(ca_file) = &self.ca_file {
            acceptor.set_ca_file(ca_file).expect("set_ca_file");
        }
        if let Some(private_key_file) = &self.private_key_file {
            acceptor.set_private_key_file(private_key_file, SslFiletype::PEM).expect("private_key_file");
        }
        if let Some(certificate_file) = &self.certificate_file {
            acceptor.set_certificate_file(certificate_file, SslFiletype::PEM).expect("private_key_file");
        }
        if self.private_key_file.is_some() && self.certificate_file.is_some() {
            acceptor.check_private_key().expect("invalid private key");
        }
        if self.verify_none {
            acceptor.set_verify(boring::ssl::SslVerifyMode::NONE);
        }
        let acceptor = Arc::new(acceptor.build());
        // Establish a listener.
        let listener = TcpListener::bind(format!("{}:{}", self.host, self.port))?;
        Ok(Listener {
            acceptor,
            listener,
        })
    }
}

//////////////////////////////////////////// from_stream ///////////////////////////////////////////

pub fn from_stream(stream: SslStream<TcpStream>) -> Result<(RecvChannel, SendChannel), Error> {
    FROM_STREAM.click();
    stream.get_ref().set_nonblocking(true)?;
    stream.get_ref().set_nodelay(true)?;
    let state = Arc::new(Mutex::new(ChannelState { stream }));
    let recv = RecvChannel {
        state: Arc::clone(&state),
        recv_buf: Vec::new(),
        recv_idx: 0,
    };
    let send = SendChannel {
        state,
        send_buf: Vec::new(),
        send_idx: 0,
    };
    Ok((recv, send))
}
