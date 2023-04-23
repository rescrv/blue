use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

use boring::ssl::{ErrorCode, SslConnector, SslFiletype, SslMethod, SslStream};

use crc32c;

use biometrics::{Collector, Counter, Moments};

use buffertk::{stack_pack, Buffer, Packable, Unpackable};

use util::stopwatch::Stopwatch;

use zerror_core::ErrorCore;

use rpc_pb::Frame;

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
    pub fn send(&mut self, buf: &[u8]) -> Result<(), rpc_pb::Error> {
        let frame = Frame {
            size: buf.len() as u64,
            crc32c: crc32c::crc32c(buf),
        };
        assert!(frame.pack_sz() < 256);
        let mut hdr: Vec<u8> = vec![frame.pack_sz() as u8];
        stack_pack(frame).append_to_vec(&mut hdr);
        self.send_raw(&hdr)?;
        self.send_raw(buf)?;
        MESSAGES_SENT.click();
        Ok(())
    }

    fn send_raw(&mut self, mut buf: &[u8]) -> Result<(), rpc_pb::Error> {
        while !buf.is_empty() {
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
            let sw = Stopwatch::default();
            let mut state = self.state.lock().unwrap();
            match state.stream.ssl_write(buf) {
                Ok(sz) => buf = &buf[sz..],
                Err(err) => {
                    if err.code() != ErrorCode::WANT_READ && err.code() != ErrorCode::WANT_WRITE {
                        return Err(rpc_pb::Error::TransportFailure {
                            core: ErrorCore::default(),
                            what: err.to_string(),
                        });
                    }
                }
            }
            SEND_CALL_LATENCY.add(sw.since());
        }
        Ok(())
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
