use buffertk::{Packable, StackPacker, Unpacker};

use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const HEADER_MAX_SIZE: usize = 1 + 1 + 10 + 1 + 4;

//////////////////////////////////////////// SendBuffer ////////////////////////////////////////////

pub struct SendBuffer {
    buffer: Vec<u8>,
    target: usize,
    lower: usize,
    upper: usize,
}

impl SendBuffer {
    pub fn new(size: usize) -> SendBuffer {
        Self {
            buffer: vec![0u8; size],
            target: size,
            lower: 0,
            upper: 0,
        }
    }

    #[allow(dead_code)]
    pub fn free(&self) -> usize {
        self.buffer.len() - self.upper + self.lower
    }

    pub fn is_empty(&self) -> bool {
        self.lower == self.upper
    }

    pub fn append_pa<P: Packable, T: Packable>(&mut self, pa: StackPacker<'_, P, T>) {
        let pa_sz = pa.pack_sz();
        self.make_room(pa_sz);
        assert!(self.buffer.len() - self.upper >= pa_sz);
        let amt = pa.into_slice(&mut self.buffer[self.upper..]).len();
        assert_eq!(amt, pa_sz);
        self.upper += amt;
    }

    pub fn append_bytes(&mut self, bytes: &[u8]) {
        self.make_room(bytes.len());
        assert!(self.buffer.len() - self.upper >= bytes.len());
        for (i, j) in std::iter::zip(self.upper..self.buffer.len(), 0..bytes.len()) {
            self.buffer[i] = bytes[j];
        }
        self.upper += bytes.len();
    }

    pub fn consume(&mut self, bytes: usize) {
        assert!(bytes <= self.upper - self.lower);
        self.lower += bytes;
        if self.buffer.len() > self.target {
            self.shift_down();
            let sz = std::cmp::max(self.target, self.upper);
            self.buffer.resize(sz, 0);
            self.buffer.shrink_to_fit();
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buffer[self.lower..self.upper]
    }

    fn shift_down(&mut self) {
        if self.lower > 0 {
            let bytes: &mut [u8] = self.buffer.as_mut();
            for (i, j) in std::iter::zip(0..self.upper, self.lower..self.upper) {
                bytes[i] = bytes[j];
            }
            self.upper -= self.lower;
            self.lower -= self.lower;
        }
    }

    fn make_room(&mut self, how_much: usize) {
        if self.buffer.len() - self.upper < how_much {
            self.shift_down();
        }
        let delta = self.buffer.len() - self.upper;
        if delta < how_much {
            self.buffer.resize(self.buffer.len() + how_much - delta, 0);
        }
    }
}

//////////////////////////////////////////// RecvBuffer ////////////////////////////////////////////

pub enum RecvBuffer {
    Frame {
        hdr: [u8; HEADER_MAX_SIZE],
        idx: usize,
    },
    Body {
        crc: u32,
        buf: Option<Vec<u8>>,
        idx: usize,
    },
}

impl RecvBuffer {
    pub fn new() -> Self {
        Self::Frame {
            hdr: [0u8; HEADER_MAX_SIZE],
            idx: 0,
        }
    }

    pub fn read_bytes<F: FnMut(Vec<u8>)>(
        &mut self,
        mut bytes: &[u8],
        mut receiver: F,
    ) -> Result<usize, rpc_pb::Error> {
        let mut returned = 0;
        'reading_bytes: while !bytes.is_empty() {
            match self {
                RecvBuffer::Frame { hdr, idx } => {
                    if *idx < 1 {
                        hdr[0] = bytes[0];
                        *idx += 1;
                        bytes = &bytes[1..];
                    }
                    assert!(*idx > 0);
                    if bytes.is_empty() {
                        continue 'reading_bytes;
                    }
                    if hdr[0] as usize > HEADER_MAX_SIZE - 1 {
                        return Err(rpc_pb::Error::SerializationError {
                            core: ErrorCore::default(),
                            err: prototk::Error::BufferTooShort {
                                required: hdr[*idx] as usize,
                                had: *idx,
                            },
                            context: "frame header size invalid".to_owned(),
                        });
                    }
                    let amt = std::cmp::min(hdr[0] as usize + 1 - *idx, bytes.len());
                    if amt > 0 {
                        hdr[*idx..*idx + amt].copy_from_slice(&bytes[..amt]);
                        *idx += amt;
                        bytes = &bytes[amt..];
                    }
                    if *idx > hdr[0] as usize {
                        let hdr_sz = hdr[0] as usize;
                        assert_eq!(*idx, hdr_sz + 1);
                        let mut up = Unpacker::new(&hdr[1..1 + hdr_sz]);
                        let frame: rpc_pb::Frame = up.unpack()?;
                        if frame.size > rpc_pb::MAX_BODY_SIZE as u64 {
                            return Err(rpc_pb::Error::RequestTooLarge {
                                core: ErrorCore::default(),
                                size: frame.size,
                            });
                        }
                        let buf = vec![0u8; frame.size as usize];
                        *self = RecvBuffer::Body {
                            crc: frame.crc32c,
                            buf: Some(buf),
                            idx: 0,
                        }
                    }
                }
                RecvBuffer::Body { crc: _, buf, idx } => {
                    let buf = buf.as_mut().unwrap();
                    let amt = std::cmp::min(buf.len() - *idx, bytes.len());
                    if amt > 0 {
                        buf[*idx..*idx + amt].copy_from_slice(&bytes[..amt]);
                        *idx += amt;
                        bytes = &bytes[amt..];
                    }
                    if *idx == buf.len() {
                        let mut next = RecvBuffer::Frame {
                            hdr: [0u8; HEADER_MAX_SIZE],
                            idx: 0,
                        };
                        std::mem::swap(self, &mut next);
                        if let RecvBuffer::Body {
                            crc,
                            mut buf,
                            idx: _,
                        } = next
                        {
                            let buf = buf.take().unwrap();
                            if crc32c::crc32c(buf.as_ref()) != crc {
                                return Err(rpc_pb::Error::TransportFailure {
                                    core: ErrorCore::default(),
                                    what: format!("crc32c checksum failed: {crc}"),
                                });
                            }
                            receiver(buf);
                            returned += 1;
                        } else {
                            panic!("swap failed");
                        }
                    }
                }
            };
        }
        Ok(returned)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod send_buffer {
    use buffertk::{stack_pack, v64};

    use super::*;

    #[test]
    fn empty() {
        let buf = SendBuffer::new(64);
        assert_eq!(0, buf.lower);
        assert_eq!(0, buf.upper);
    }

    #[test]
    fn free() {
        let buf = SendBuffer::new(64);
        assert_eq!(64, buf.free())
    }

    #[test]
    fn append_pa() {
        let mut buf = SendBuffer::new(64);
        buf.append_pa(stack_pack(v64::from(42)).pack(v64::from(256)));
        assert_eq!(buf.lower, 0);
        assert_eq!(buf.upper, 3);
        assert_eq!(&[42u8, 128u8, 2u8], buf.bytes());
    }

    #[test]
    fn append_bytes() {
        let mut buf = SendBuffer::new(64);
        buf.append_bytes(&[42u8, 128u8, 2u8]);
        assert_eq!(buf.lower, 0);
        assert_eq!(buf.upper, 3);
        assert_eq!(&[42u8, 128u8, 2u8], buf.bytes());
    }

    #[test]
    fn consume() {
        let mut buf = SendBuffer::new(64);
        buf.append_bytes(&[42u8, 128u8, 2u8]);
        buf.consume(1);
        assert_eq!(buf.lower, 1);
        assert_eq!(buf.upper, 3);
        assert_eq!(&[128u8, 2u8], buf.bytes());
    }

    #[test]
    fn resizing() {
        let mut buf = SendBuffer::new(64);
        let bytes = [0u8; 1024];
        buf.append_bytes(&bytes);
        assert_eq!(1024, buf.buffer.len());
        buf.consume(1024);
        assert_eq!(64, buf.buffer.len());
    }
}

#[cfg(test)]
mod recv_buffer {
    use buffertk::{stack_pack, v64};

    use super::*;

    #[test]
    fn empty() {
        let _buf = RecvBuffer::Frame {
            hdr: [0u8; HEADER_MAX_SIZE],
            idx: 0,
        };
    }

    #[test]
    fn read_bytes_frame() {
        let mut recv_buf = RecvBuffer::Frame {
            hdr: [0u8; HEADER_MAX_SIZE],
            idx: 0,
        };
        let mut bytes_body = vec![0; 16];
        for (b, i) in std::iter::zip(bytes_body.iter_mut(), 0..16) {
            *b = i;
        }
        let frame: rpc_pb::Frame = rpc_pb::Frame::from_buffer(bytes_body.as_ref());
        let frame_sz: v64 = frame.pack_sz().into();
        let mut bytes_hdr = Vec::new();
        stack_pack(frame_sz)
            .pack(frame)
            .append_to_vec(&mut bytes_hdr);
        let received = &mut Vec::new();
        let mut receiver = |buf| {
            received.push(buf);
        };
        recv_buf
            .read_bytes(bytes_hdr.as_ref(), &mut receiver)
            .unwrap();
        if let RecvBuffer::Body { crc, buf, idx } = &recv_buf {
            assert_eq!(3653830891, *crc);
            let buf: &[u8] = buf.as_ref().unwrap();
            assert_eq!(&[0u8; 16], buf);
            assert_eq!(0, *idx);
        } else {
            panic!("test failed");
        }
        recv_buf
            .read_bytes(bytes_body.as_ref(), &mut receiver)
            .unwrap();
        if let RecvBuffer::Frame { hdr, idx } = &recv_buf {
            assert_eq!([0u8; HEADER_MAX_SIZE], *hdr);
            assert_eq!(0, *idx);
        } else {
            panic!("test failed");
        }
    }
}
