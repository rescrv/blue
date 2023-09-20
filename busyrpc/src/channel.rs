use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};

use boring::ssl::{ErrorCode, SslStream};

use biometrics::{Collector, Counter, Moments};

use buffertk::{stack_pack, v64, Packable};

use zerror_core::ErrorCore;

use super::buffers::{RecvBuffer, SendBuffer};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static NEW_CHANNEL: Counter = Counter::new("busyrpc.channel.new");
static SEND: Counter = Counter::new("busyrpc.channel.send");
static WANT_SEND: Counter = Counter::new("busyrpc.channel.want_send");
static WANT_RECV: Counter = Counter::new("busyrpc.channel.want_recv");
static DO_SEND_WORK: Counter = Counter::new("busyrpc.channel.send_work");
static DO_RECV_WORK: Counter = Counter::new("busyrpc.channel.recv_work");
static SEND_BUF_EMPTIED: Counter = Counter::new("busyrpc.channel.send_buf_emptied");

static WRITE_SIZE: Moments = Moments::new("busyrpc.channel.write_size");
static READ_SIZE: Moments = Moments::new("busyrpc.channel.read_size");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&NEW_CHANNEL);
    collector.register_counter(&SEND);
    collector.register_counter(&WANT_SEND);
    collector.register_counter(&WANT_RECV);
    collector.register_counter(&DO_SEND_WORK);
    collector.register_counter(&DO_RECV_WORK);
    collector.register_counter(&SEND_BUF_EMPTIED);

    collector.register_moments(&WRITE_SIZE);
    collector.register_moments(&READ_SIZE);
}

////////////////////////////////////////////// Channel /////////////////////////////////////////////

/// A channel is a bidirectional wrapper around an ssl stream/socket.
pub struct Channel {
    stream: SslStream<TcpStream>,
    send_buf: SendBuffer,
    recv_buf: RecvBuffer,
}

impl Channel {
    /// Create a new Channel from an established SSL-wrapped TcpStream.
    pub fn new(stream: SslStream<TcpStream>, mut send_buf_sz: usize) -> Result<Self, rpc_pb::Error> {
        NEW_CHANNEL.click();
        if send_buf_sz < 64 {
            send_buf_sz = 64;
        }
        assert!(send_buf_sz < u32::max_value() as usize);
        stream.get_ref().set_nodelay(true)?;
        stream.get_ref().set_nonblocking(true)?;
        Ok(Channel {
            stream,
            send_buf: SendBuffer::new(send_buf_sz),
            recv_buf: RecvBuffer::new(),
        })
    }

    /// Send on this channel, buffering the message in userspace if the socket would block.
    ///
    /// This will resize the userspace buffer to hold the full message.
    pub fn send(&mut self, msg: &[u8]) -> Result<(), rpc_pb::Error> {
        SEND.click();
        assert!(msg.len() <= rpc_pb::MAX_REQUEST_SIZE);
        assert!(msg.len() <= rpc_pb::MAX_RESPONSE_SIZE);
        let frame = rpc_pb::Frame::from_buffer(msg);
        let frame_sz: v64 = frame.pack_sz().into();
        self.send_buf.append_pa(stack_pack(frame_sz).pack(frame));
        self.send_buf.append_bytes(msg);
        self.do_send_work()?;
        Ok(())
    }

    /// Try sending on this channel, but only if the message can be immediately buffered.
    /// The message is outright dropped if it won't fit in the send buffer.
    /// Returns true if the message was buffered.
    ///
    /// Used for heartbeats.
    pub fn try_send(&mut self, msg: &[u8]) -> Result<bool, rpc_pb::Error> {
        assert!(msg.len() <= rpc_pb::MAX_REQUEST_SIZE);
        assert!(msg.len() <= rpc_pb::MAX_RESPONSE_SIZE);
        let frame = rpc_pb::Frame::from_buffer(msg);
        let frame_sz: usize = frame.pack_sz();
        assert!(frame_sz < 128);
        if self.send_buf.free() >= 1 + frame_sz + msg.len() {
            let frame_sz: v64 = frame_sz.into();
            self.send_buf.append_pa(stack_pack(frame_sz).pack(frame));
            self.send_buf.append_bytes(msg);
            self.do_send_work()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn needs_write(&mut self) -> bool {
        !self.send_buf.is_empty()
    }

    /// Do work to flush the send buffer until the kernel reports that it cannot flush more.
    ///
    /// Returns true if the kernel would block, false if the send_buffer is empty.
    pub fn do_send_work(&mut self) -> Result<bool, rpc_pb::Error> {
        DO_SEND_WORK.click();
        loop {
            if self.send_buf.bytes().is_empty() {
                SEND_BUF_EMPTIED.click();
                return Ok(false);
            }
            let amt = match self.stream.ssl_write(self.send_buf.bytes()) {
                Ok(amt) => amt,
                Err(err) => {
                    match err.code() {
                        ErrorCode::WANT_WRITE => {
                            WANT_SEND.click();
                            return Ok(true);
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
            WRITE_SIZE.add(amt as f64);
            if amt != 0 {
                self.send_buf.consume(amt);
            } else {
                return Err(rpc_pb::Error::TransportFailure {
                    core: ErrorCore::default(),
                    what: "socket closed".to_string(),
                });
            }
        }
    }

    /// Try to receive a message from the channel.
    ///
    /// Returns true if reads were processed to the point of blocking.
    pub fn do_recv_work<F: FnMut(Vec<u8>)>(&mut self, mut f: F) -> Result<bool, rpc_pb::Error> {
        DO_RECV_WORK.click();
        let mut local_buffer = [0u8; 4096];
        loop {
            match self.stream.ssl_read(&mut local_buffer) {
                Ok(sz) if sz == 0 => {
                    return Err(rpc_pb::Error::TransportFailure {
                        core: ErrorCore::default(),
                        what: "socket closed".to_string(),
                    });
                },
                Ok(sz) => {
                    READ_SIZE.add(sz as f64);
                    if self.recv_buf.read_bytes(&local_buffer[..sz], &mut f)? > 0 {
                        return Ok(false);
                    }
                },
                Err(err) => {
                    return match err.code() {
                        ErrorCode::WANT_READ => {
                            WANT_RECV.click();
                            Ok(true)
                        },
                        _ => {
                            Err(rpc_pb::Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: err.to_string(),
                            })
                        },
                    };
                },
            };
        }
    }
}

impl AsRawFd for Channel {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.get_ref().as_raw_fd()
    }
}
