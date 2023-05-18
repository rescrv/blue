use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use boring::ssl::{ErrorCode, SslConnector, SslMethod, SslStream};

use crc32c;

use biometrics::{Collector, Counter, Moments};

use buffertk::{stack_pack, Buffer, Packable, Unpackable};

use util::stopwatch::Stopwatch;

use zerror_core::ErrorCore;

use rpc_pb::{Error, Frame};

///////////////////////////////////////////// constants ////////////////////////////////////////////

// These match Linux definitions.
pub const POLLIN: i16 = 0x0001;
pub const POLLOUT: i16 = 0x0004;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static CONNECT: Counter = Counter::new("rivulet.connect");
static FROM_STREAM: Counter = Counter::new("rivulet.from_stream");

static MESSAGES_SENT: Counter = Counter::new("rivulet.messages_sent");
static MESSAGES_RECV: Counter = Counter::new("rivulet.messages_recv");

static POLL_ERRORS: Counter = Counter::new("rivulet.poll.errors");
static RECV_ERRORS: Counter = Counter::new("rivulet.recv.errors");
static SEND_ERRORS: Counter = Counter::new("rivulet.send.errors");

static SEND_POLL_LATENCY: Moments = Moments::new("rivulet.send.poll_latency");
static SEND_CALL_LATENCY: Moments = Moments::new("rivulet.send.call_latency");
static SEND_ERROR_LATENCY: Moments = Moments::new("rivulet.send.error_latency");
static RECV_POLL_LATENCY: Moments = Moments::new("rivulet.recv.poll_latency");
static RECV_CALL_LATENCY: Moments = Moments::new("rivulet.recv.call_latency");
static RECV_ERROR_LATENCY: Moments = Moments::new("rivulet.recv.error_latency");

static SEND_WANT_READ: Counter = Counter::new("rivulet.send.want_read");
static SEND_WANT_WRITE: Counter = Counter::new("rivulet.send.want_write");
static RECV_WANT_READ: Counter = Counter::new("rivulet.recv.want_read");
static RECV_WANT_WRITE: Counter = Counter::new("rivulet.recv.want_write");

static SEND_SHRINK_BUF: Counter = Counter::new("rivulet.send.shrink_buf");

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
            if let Some(buf) = self.try_recv()? {
                return Ok(buf);
            }
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: POLLIN,
                revents: 0,
            };
            let sw = Stopwatch::default();
            unsafe {
                if libc::poll(&mut pfd, 1, 5_000) < 0 {
                    POLL_ERRORS.click();
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            RECV_POLL_LATENCY.add(sw.since());
        }
    }

    /// Attempt to recv a buffer until either it succeeds or it hits EAGAIN.
    pub fn try_recv(&mut self) -> Result<Option<Buffer>, Error> {
        if self.recv_idx == 0 {
            match self.work_recv(1) {
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
            match self.work_recv(frame_sz + 1) {
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
            match self.work_recv(limit) {
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

    fn work_recv(&mut self, require: usize) -> WorkDone {
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
                    RECV_ERROR_LATENCY.add(sw.since());
                    match err.code() {
                        ErrorCode::WANT_READ => {
                            RECV_WANT_READ.click();
                            return WorkDone::EncounteredEagain;
                        },
                        ErrorCode::WANT_WRITE => {
                            RECV_WANT_WRITE.click();
                            return WorkDone::EncounteredEagain;
                        },
                        _ => {
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

//////////////////////////////////////////// SendChannel ///////////////////////////////////////////

#[derive(Debug)]
pub struct SendChannel {
    state: Arc<Mutex<ChannelState>>,
    send_buf: Vec<u8>,
    send_idx: usize,
}

impl SendChannel {
    pub fn send(&mut self, body: &[u8]) -> Result<(), Error> {
        self.enqueue(body)?;
        self.blocking_drain()?;
        MESSAGES_SENT.click();
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
        Ok(())
    }

    pub fn blocking_drain(&mut self) -> Result<(), Error> {
        'draining:
        while self.send_idx < self.send_buf.len() {
            let events = self.try_drain()?;
            if self.send_idx >= self.send_buf.len() {
                continue 'draining;
            }
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: events,
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

    /// Call try_drain when it's OK to call into SSL_write with the given send_buf.
    pub fn try_drain(&mut self) -> Result<i16, Error> {
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
                            return Ok(POLLIN|POLLOUT);
                        },
                        ErrorCode::WANT_WRITE => {
                            SEND_WANT_WRITE.click();
                            return Ok(POLLOUT);
                        },
                        _ => {
                            return Err(Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: err.to_string(),
                            });
                        },
                    }
                },
            };
        }
        Ok(0)
    }
}

////////////////////////////////////////////// connect /////////////////////////////////////////////

pub fn connect(hostname: &str, port: u16) -> Result<(RecvChannel, SendChannel), Error> {
    CONNECT.click();
    let mut builder =
        SslConnector::builder(SslMethod::tls()).map_err(|err| Error::TransportFailure {
            core: ErrorCore::default(),
            what: format!("could not build connector builder: {}", err),
        })?;
    // TODO(rescrv): Production blocker.  Need to sort out certs, etc.
    builder.set_verify(boring::ssl::SslVerifyMode::NONE);
    let connector = builder.build();
    let stream = TcpStream::connect(format!("{hostname}:{port}"))?;
    let stream =
        connector
            .connect(hostname, stream)
            .map_err(|err| Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("{}", err),
            })?;
    from_stream(stream)
}

//////////////////////////////////////////// from_stream ///////////////////////////////////////////

pub fn from_stream(
    stream: SslStream<TcpStream>,
) -> Result<(RecvChannel, SendChannel), Error> {
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
        state: Arc::clone(&state),
        send_buf: Vec::new(),
        send_idx: 0,
    };
    Ok((recv, send))
}
