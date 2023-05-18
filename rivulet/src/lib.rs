use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use boring::ssl::{ErrorCode, SslConnector, SslMethod, SslStream};

use crc32c;

use biometrics::{Collector, Counter, Moments};

use buffertk::{stack_pack, Buffer, Packable, Unpackable};

use util::stopwatch::Stopwatch;

use zerror_core::ErrorCore;

use rpc_pb::Frame;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const FRAME_SIZE_HINT: usize = 128;

// These match Linux definitions.
pub const POLLIN: i16 = 0x0001;
pub const POLLOUT: i16 = 0x0004;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static CONNECT: Counter = Counter::new("rivulet.connect");
static FROM_STREAM: Counter = Counter::new("rivulet.from_stream");

static MESSAGES_SENT: Counter = Counter::new("rivulet.messages_sent");
static MESSAGES_RECV: Counter = Counter::new("rivulet.messages_recv");

static ERROR_POLL: Counter = Counter::new("rivulet.poll.errors");

static SEND_POLL_LATENCY: Moments = Moments::new("rivulet.send.poll_latency");
static SEND_CALL_LATENCY: Moments = Moments::new("rivulet.send.call_latency");
static RECV_POLL_LATENCY: Moments = Moments::new("rivulet.recv.poll_latency");
static RECV_CALL_LATENCY: Moments = Moments::new("rivulet.recv.call_latency");

static SEND_SHRINK_BUF: Counter = Counter::new("rivulet.send.shrink_buf");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&CONNECT);
    collector.register_counter(&FROM_STREAM);
    collector.register_counter(&MESSAGES_SENT);
    collector.register_counter(&MESSAGES_RECV);
    collector.register_moments(&SEND_POLL_LATENCY);
    collector.register_moments(&SEND_CALL_LATENCY);
    collector.register_moments(&RECV_POLL_LATENCY);
    collector.register_moments(&RECV_CALL_LATENCY);
}

/////////////////////////////////////////// ChannelState ///////////////////////////////////////////

struct ChannelState {
    stream: SslStream<TcpStream>,
}

impl AsRawFd for ChannelState {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.get_ref().as_raw_fd()
    }
}

//////////////////////////////////////////// RecvChannel ///////////////////////////////////////////

pub struct RecvChannel {
    state: Arc<Mutex<ChannelState>>,
    recv_buf: Vec<u8>,
}

impl RecvChannel {
    pub fn recv(&mut self) -> Result<Buffer, rpc_pb::Error> {
        while self.recv_buf.is_empty() || self.recv_buf[0] as usize + 1 < self.recv_buf.len() {
            self.do_work_recv(16)?;
        }
        assert!(!self.recv_buf.is_empty());
        let frame_sz = self.recv_buf[0] as usize;
        let (frame, _): (Frame, &[u8]) = Frame::unpack(&self.recv_buf[1..1 + frame_sz])?;
        let start = 1 + frame_sz;
        let limit = start + frame.size as usize;
        while self.recv_buf.len() < limit {
            self.do_work_recv(limit)?;
        }
        let buf = Buffer::from(&self.recv_buf[start..limit]);
        if crc32c::crc32c(buf.as_bytes()) != frame.crc32c {
            return Err(rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: "crc32c failed".to_string(),
            });
        }
        self.recv_buf.rotate_left(limit);
        self.recv_buf.resize(self.recv_buf.len() - limit, 0u8);
        MESSAGES_RECV.click();
        Ok(buf)
    }

    fn do_work_recv(&mut self, size_hint: usize) -> Result<usize, rpc_pb::Error> {
        loop {
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            unsafe {
                let sw = Stopwatch::default();
                if libc::poll(&mut pfd, 1, 50) < 0 {
                    ERROR_POLL.click();
                    return Err(std::io::Error::last_os_error().into());
                }
                RECV_POLL_LATENCY.add(sw.since());
            }
            let sw = Stopwatch::default();
            let recv_buf_starting_sz = self.recv_buf.len();
            self.recv_buf.resize(recv_buf_starting_sz + size_hint, 0u8);
            let mut state = self.state.lock().unwrap();
            match state
                .stream
                .ssl_read(&mut self.recv_buf[recv_buf_starting_sz..])
            {
                Ok(sz) => {
                    self.recv_buf.resize(recv_buf_starting_sz + sz, 0u8);
                    RECV_CALL_LATENCY.add(sw.since());
                    return Ok(sz);
                }
                Err(err) => {
                    if err.code() != ErrorCode::WANT_READ && err.code() != ErrorCode::WANT_WRITE {
                        return Err(rpc_pb::Error::TransportFailure {
                            core: ErrorCore::default(),
                            what: err.to_string(),
                        });
                    }
                    RECV_CALL_LATENCY.add(sw.since());
                }
            }
        }
    }
}

//////////////////////////////////////////// SendChannel ///////////////////////////////////////////

pub struct SendChannel {
    state: Arc<Mutex<ChannelState>>,
}

impl SendChannel {
    pub fn send(&mut self, body: &[u8]) -> Result<(), rpc_pb::Error> {
        self.enqueue(body)?;
        self.blocking_drain()?;
        MESSAGES_SENT.click();
        Ok(())
    }

    pub fn enqueue(&mut self, body: &[u8]) -> Result<(), rpc_pb::Error> {
        let frame = Frame {
            size: buf.len() as u64,
            crc32c: crc32c::crc32c(buf),
        };
        assert!(frame.pack_sz() < 256);
        self.send_buf.push(frame.pack_sz() as u8);
        stack_pack(frame).append_to_vec(&mut self.send_buf);
        self.send_buf.extend_from_slice(body);
        Ok(())
    }

    pub fn blocking_drain(&mut self) -> Result<(), rpc_pb::Error> {
        'draining:
        while self.send_idx < self.send_buf.len() {
            let events = self.try_drain()?;
            if self.send_idx >= self.send_buf.len() {
                continue 'draining;
            }
            let mut pfd = libc::pollfd {
                fd: self.state.lock().unwrap().as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            unsafe {
                let sw = Stopwatch::default();
                if libc::poll(&mut pfd, 1, 50) < 0 {
                    ERROR_POLL.click();
                    return Err(std::io::Error::last_os_error().into());
                }
                SEND_POLL_LATENCY.add(sw.since());
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

    /// Call blocking_send when it's OK to call into SSL_write with the given send_buf.
    pub fn try_drain(&mut self) -> Result<i16, rpc_pb::Error> {
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
                            return Err(rpc_pb::Error::TransportFailure {
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

pub fn connect(hostname: &str, port: u16) -> Result<(RecvChannel, SendChannel), rpc_pb::Error> {
    CONNECT.click();
    let mut builder =
        SslConnector::builder(SslMethod::tls()).map_err(|err| rpc_pb::Error::TransportFailure {
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
            .map_err(|err| rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("{}", err),
            })?;
    from_stream(stream)
}

//////////////////////////////////////////// from_stream ///////////////////////////////////////////

pub fn from_stream(
    stream: SslStream<TcpStream>,
) -> Result<(RecvChannel, SendChannel), rpc_pb::Error> {
    FROM_STREAM.click();
    stream.get_ref().set_nonblocking(true)?;
    stream.get_ref().set_nodelay(true)?;
    let state = Arc::new(Mutex::new(ChannelState { stream }));
    let recv = RecvChannel {
        state: Arc::clone(&state),
        recv_buf: Vec::new(),
    };
    let send = SendChannel {
        state: Arc::clone(&state),
    };
    Ok((recv, send))
}
