use std::cmp::Ordering;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;

use arrrg_derive::CommandLine;

use buffertk::{stack_pack, v64, Packable, Unpackable};

use prototk_derive::Message;

use zerror::Z;
use zerror_core::ErrorCore;

use super::{check_key_len, check_value_len, check_table_size, compare_key, Builder, Error, KeyValueDel, KeyValueEntry, KeyValuePair, KeyValuePut, KeyValueRef, TABLE_FULL_SIZE};
use super::setsum::Setsum;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

const BLOCK_BITS: u64 = 16;
const BLOCK_SIZE: u64 = 1 << BLOCK_BITS;
const BUFFER_SIZE: u64 = BLOCK_SIZE * 64;

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn block_offset(offset: u64) -> u64 {
    offset >> BLOCK_BITS
}

fn compute_true_up(offset: u64) -> u64 {
    if offset == block_offset(offset) << BLOCK_BITS {
        offset
    } else {
        next_boundary(offset)
    }
}

fn next_boundary(offset: u64) -> u64 {
    (block_offset(offset) + 1) << BLOCK_BITS
}

////////////////////////////////////////////// Header //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Header {
    #[prototk(10, uint64)]
    size: u64,
    #[prototk(11, uint32)]
    discriminant: u32,
    #[prototk(12, fixed32)]
    crc32c: u32,
}

pub const HEADER_MAX_SIZE: u64 = 1 // one byte for size of header
                               + 1 + 10 // size is a varint
                               + 1 + 1 // discriminant is a varint of one byte---always
                               + 1 + 4; // crc32c is fixed32.
const HEADER_WHOLE: u32 = 1;
const HEADER_FIRST: u32 = 2;
const HEADER_SECOND: u32 = 3;

//////////////////////////////////////////// LogOptions ////////////////////////////////////////////

#[derive(Clone, CommandLine, Debug, Eq, PartialEq)]
pub struct LogOptions {
    #[arrrg(optional, "Size of the write buffer.")]
    pub(crate) write_buffer: usize,
    #[arrrg(optional, "Size of the read buffer.")]
    pub(crate) read_buffer: usize,
    #[arrrg(optional, "Roll over logs that exceed this number of bytes.")]
    pub(crate) rollover_size: usize,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            write_buffer: BUFFER_SIZE as usize,
            read_buffer: BUFFER_SIZE as usize,
            rollover_size: 1<<22,
        }
    }
}

//////////////////////////////////////////// LogBuilder ////////////////////////////////////////////

pub struct LogBuilder<W: Write> {
    options: LogOptions,
    output: BufWriter<W>,
    bytes_written: u64,
    setsum: Setsum,
}

impl LogBuilder<File> {
    pub fn new<P: AsRef<Path>>(options: LogOptions, file_name: P) -> Result<Self, Error> {
        let file: File = OpenOptions::new().create_new(true).read(true).write(true).open(file_name)?;
        Self::from_write(options, file)
    }

    pub fn fsync(&mut self) -> Result<(), Error> {
        Ok(self.output.get_mut().sync_data()?)
    }
}

impl<W: Write> LogBuilder<W> {
    pub fn from_write(options: LogOptions, write: W) -> Result<Self, Error> {
        let output = BufWriter::with_capacity(options.write_buffer, write);
        Ok(Self {
            options,
            output,
            bytes_written: 0,
            setsum: Setsum::default(),
        })
    }

    fn append(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let header = Header {
            size: buffer.len() as u64,
            crc32c: crc32c::crc32c(buffer),
            discriminant: HEADER_WHOLE,
        };
        let header_sz: v64 = header.pack_sz().into();
        let header_pa = stack_pack(header_sz);
        let header_pa = header_pa.pack(header);
        let nb = next_boundary(self.bytes_written);
        let new_offset = self.bytes_written + (header_pa.pack_sz() + buffer.len()) as u64;
        check_table_size(new_offset as usize)?;
        if new_offset > self.options.rollover_size as u64 {
            return Err(Error::TableFull {
                core: ErrorCore::default(),
                size: new_offset as usize,
                limit: self.options.rollover_size,
            });
        }
        if new_offset > nb {
            self.append_split(buffer)
        } else {
            let header_buf = header_pa.to_vec();
            self.write(&header_buf)?;
            self.write(buffer)?;
            assert!(self.bytes_written <= nb);
            Ok(())
        }
    }

    fn append_split(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let nb = next_boundary(self.bytes_written);
        let roundup = nb - self.bytes_written;
        if roundup <= HEADER_MAX_SIZE {
            self.true_up(nb)?;
            return self.append(buffer);
        }
        let first_bytes = (roundup - HEADER_MAX_SIZE) as usize;
        let first = &buffer[..first_bytes];
        let second = &buffer[first_bytes..];
        let first_header = Header {
            size: first.len() as u64,
            crc32c: crc32c::crc32c(first),
            discriminant: HEADER_FIRST,
        };
        let second_header = Header {
            size: second.len() as u64,
            crc32c: crc32c::crc32c(second),
            discriminant: HEADER_SECOND,
        };
        let first_header_sz: v64 = first_header.pack_sz().into();
        let second_header_sz: v64 = second_header.pack_sz().into();
        let first_header_buf = stack_pack(first_header_sz).pack(first_header).to_vec();
        let second_header_buf = stack_pack(second_header_sz).pack(second_header).to_vec();
        assert!(first_header_buf.len() as u64 <= HEADER_MAX_SIZE);
        assert!(second_header_buf.len() as u64 <= HEADER_MAX_SIZE);
        self.write(&first_header_buf)?;
        self.write(first)?;
        self.true_up(nb)?;
        self.write(&second_header_buf)?;
        self.write(second)?;
        Ok(())
    }

    fn true_up(&mut self, nb: u64) -> Result<(), Error> {
        assert!(nb >= self.bytes_written);
        let roundup = (nb - self.bytes_written) as usize;
        assert!(roundup as u64 <= HEADER_MAX_SIZE);
        let buf: &[u8] = &[0u8; HEADER_MAX_SIZE as usize][..roundup];
        if !buf.is_empty() {
            self.write(buf)
        } else {
            Ok(())
        }
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.output.flush()?;
        Ok(())
    }

    fn write(&mut self, buffer: &[u8]) -> Result<(), Error> {
        self.output.write_all(buffer)?;
        self.bytes_written += buffer.len() as u64;
        Ok(())
    }
}

impl<W: Write> Builder for LogBuilder<W> {
    type Sealed = Setsum;

    fn approximate_size(&self) -> usize {
        self.bytes_written as usize
    }

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        check_key_len(key)?;
        check_value_len(value)?;
        self.setsum.put(key, timestamp, value);
        let put = KeyValuePut {
            shared: 0,
            key_frag: key,
            timestamp,
            value,
        };
        let buf = stack_pack(KeyValueEntry::Put(put)).to_vec();
        self.append(&buf)?;
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        self.setsum.del(key, timestamp);
        let del = KeyValueDel {
            shared: 0,
            key_frag: key,
            timestamp,
        };
        let buf = stack_pack(KeyValueEntry::Del(del)).to_vec();
        self.append(&buf)?;
        Ok(())
    }

    fn seal(mut self) -> Result<Self::Sealed, Error> {
        self.flush()?;
        Ok(self.setsum)
    }
}

//////////////////////////////////////////// LogIterator ///////////////////////////////////////////

pub struct LogIterator<R: Read + Seek> {
    input: BufReader<R>,
    buffer: Vec<u8>,
}

impl LogIterator<File> {
    pub fn new<P: AsRef<Path>>(options: LogOptions, file_name: P) -> Result<Self, Error> {
        let file: File = OpenOptions::new().create(false).read(true).open(file_name)?;
        Self::from_reader(options, file)
    }
}

impl<R: Read + Seek> LogIterator<R> {
    pub fn from_reader(options: LogOptions, reader: R) -> Result<Self, Error> {
        let input = BufReader::with_capacity(options.read_buffer, reader);
        Ok(Self {
            input,
            buffer: Vec::new(),
        })
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<KeyValueRef>, Error> {
        self.buffer.clear();
        let header = match self.next_frame()? {
            Some(header) => header,
            None => {
                return Ok(None);
            },
        };
        if header.discriminant == HEADER_WHOLE {
            // pass
        } else if header.discriminant == HEADER_FIRST {
            self.true_up()?;
            let header2 = match self.next_frame()? {
                Some(header) => header,
                None => {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        context: "truncation: no second header".to_owned(),
                    }
                    .with_variable("header1", header));
                },
            };
            if header2.discriminant != HEADER_SECOND {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "invalid discriminant in second header".to_owned(),
                }
                .with_variable("discriminant", header2.discriminant));
            }
        } else {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "invalid discriminant in header".to_owned(),
            }
            .with_variable("discriminant", header.discriminant));
        }
        let kve: KeyValueEntry = <KeyValueEntry as Unpackable>::unpack(&self.buffer)?.0;
        let kvr = KeyValueRef {
            key: kve.key_frag(),
            timestamp: kve.timestamp(),
            value: kve.value(),
        };
        Ok(Some(kvr))
    }

    fn next_frame(&mut self) -> Result<Option<Header>, Error> {
        let header = match self.next_header()? {
            Some(header) => header,
            None => {
                return Ok(None);
            },
        };
        let buffer_start_sz = self.buffer.len();
        let buffer_new_sz = buffer_start_sz + header.size as usize;
        self.buffer.resize(buffer_new_sz, 0);
        let buffer = &mut self.buffer[buffer_start_sz..];
        self.input.read_exact(buffer)?;
        let crc = crc32c::crc32c(buffer);
        if crc != header.crc32c {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "crc checksum failed".to_owned(),
            }
            .with_variable("expected", header.crc32c)
            .with_variable("returned", crc));
        }
        Ok(Some(header))
    }

    fn next_header(&mut self) -> Result<Option<Header>, Error> {
        'looping:
        loop {
            let header_sz: &mut [u8] = &mut [0; 1];
            let header: &mut [u8] = &mut [0; HEADER_MAX_SIZE as usize];
            match self.input.read_exact(header_sz) {
                Ok(_) => (),
                Err(err) => {
                    if err.kind() == ErrorKind::UnexpectedEof {
                        return Ok(None);
                    } else {
                        return Err(err.into());
                    }
                },
            };
            let header_sz: usize = header_sz[0] as usize;
            if header_sz == 0 {
                self.true_up()?;
                continue 'looping;
            }
            if header_sz as u64 > HEADER_MAX_SIZE {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "header size exceeds HEADER_MAX_SIZE".to_owned(),
                }
                .with_variable("header_sz", header_sz));
            }
            let header = &mut header[..header_sz];
            self.input.read_exact(header)?;
            let header: Header = <Header as Unpackable>::unpack(header)?.0;
            if header.size > TABLE_FULL_SIZE as u64 {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "entry size exceeds TABLE_FULL_SIZE".to_owned(),
                }
                .with_variable("size", header.size));
            }
            return Ok(Some(header))
        }
    }

    fn true_up(&mut self) -> Result<(), Error> {
        let offset = self.input.stream_position()?;
        let trued_up = compute_true_up(offset);
        if trued_up - offset > HEADER_MAX_SIZE {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "true-up exceeds HEADER_MAX_SIZE".to_owned(),
            }
            .with_variable("offset", offset)
            .with_variable("trued_up", trued_up));
        }
        self.input.seek(SeekFrom::Start(trued_up))?;
        Ok(())
    }
}

pub fn log_to_builder<P: AsRef<Path>, B: Builder>(log_options: LogOptions, log_path: P, mut builder: B) -> Result<B::Sealed, Error> {
    let mut log_iter = LogIterator::new(log_options, log_path)?;
    let mut kvrs = Vec::new();
    while let Some(kvr) = log_iter.next().unwrap() {
        kvrs.push(KeyValuePair::from(kvr));
    }
    fn sort_key(lhs: &KeyValuePair, rhs: &KeyValuePair) -> Ordering {
        compare_key(&lhs.key, lhs.timestamp, &rhs.key, rhs.timestamp)
    }
    kvrs.sort_by(sort_key);
    for kvr in kvrs.into_iter() {
        match kvr.value {
            Some(v) => { builder.put(&kvr.key, kvr.timestamp, &v)?; },
            None => { builder.del(&kvr.key, kvr.timestamp)?; },
        }
    }
    builder.seal()
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod offsets {
    use super::*;

    #[test]
    fn offsets() {
        assert_eq!(0, block_offset(0));
        assert_eq!(0, block_offset(65535));
        assert_eq!(1, block_offset(65536));
        assert_eq!(1, block_offset(65537));
    }

    #[test]
    fn boundaries() {
        assert_eq!(65536, next_boundary(0));
        assert_eq!(65536, next_boundary(65535));
        assert_eq!(131072, next_boundary(65536));
        assert_eq!(131072, next_boundary(65537));
    }

    #[test]
    fn true_ups() {
        assert_eq!(0, compute_true_up(0));
        assert_eq!(65536, compute_true_up(1));
        assert_eq!(65536, compute_true_up(65536));
        assert_eq!(131072, compute_true_up(65537));
        assert_eq!(131072, compute_true_up(131072));
    }
}

#[cfg(test)]
mod builder {
    use super::*;

    #[test]
    fn empty() {
        let mut write = Vec::new();
        let log = LogBuilder::from_write(LogOptions::default(), &mut write).expect("should not fail");
        drop(log);
        let exp: &[u8] = &[];
        let got: &[u8] = &write;
        assert_eq!(exp, got);
    }

    #[test]
    fn header_one() {
        let header = Header {
            size: 19,
            discriminant: 1,
            crc32c: 0x5475cb53,
        };
        let buf = stack_pack(header).to_vec();
        let exp: &[u8] = &[
            80, 19, // size: uint64
            88, 1, // discriminant: uint32,
            101, 83, 203, 117, 84, // crc32c: fixed32
        ];
        let got: &[u8] = &buf;
        assert_eq!(exp, got);
    }

    #[test]
    fn crc32c_one() {
        let buf: &[u8] = &[
            66, 19, // tag, length of KeyValueEntry::Put

            8, 0,
            18, 3, 101, 102, 103,
            24, 42,
            34, 8, 1, 2, 3, 4, 5, 6, 7, 8
        ];
        assert_eq!(0x5acc2712, crc32c::crc32c(buf));
    }

    #[test]
    fn insert_one() {
        let mut write = Vec::new();
        let mut log = LogBuilder::from_write(LogOptions::default(), &mut write).expect("should not fail");
        log.put(&[101, 102, 103], 42, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        log.flush().unwrap();
        drop(log);
        let exp: &[u8] = &[
            9, // There are nine bytes in the header.
            80, 21, // size: uint64
            88, 1, // discriminant: uint32,
            101, 18, 39, 204, 90, // crc32c: fixed32

            66, 19, // tag, length of KeyValueEntry::Put

            8, 0, // shared
            18, 3, 101, 102, 103, // key_frag
            24, 42, // timestamp
            34, 8, 1, 2, 3, 4, 5, 6, 7, 8 // value
        ];
        let got: &[u8] = &write;
        assert_eq!(exp, got);
    }

    #[test]
    fn insert_across_boundary() {
        let mut write = Vec::new();
        let mut log = LogBuilder::from_write(LogOptions::default(), &mut write).expect("should not fail");
        let key1 = vec!['A' as u8; 64];
        let value1 = vec!['B' as u8; 32768];
        let key2 = vec!['C' as u8; 64];
        let value2 = vec!['D' as u8; 32768];
        log.put(&key1, 42, &value1).unwrap();
        log.put(&key2, 99, &value2).unwrap();
        log.flush().unwrap();
        drop(log);
        // The first put.
        let exp: &[u8] = &[
            11, // There are 11 bytes in this header.
            80, 206, 128, 2, // size: uint64
            88, 1, // discriminant: uint32
            101, 151, 232, 6, 121, // crc32c: fixed32
        ];
        assert_eq!(exp, &write[..12]);
        let exp: &[u8] = &[
            66, 202, 128, 2, // tag, length of KeyValueEntry::Put
            8, 0, // shared
            18, 64,  // key1_frag tag + sz
        ];
        assert_eq!(exp, &write[12..20]);
        assert_eq!(&key1, &write[20..84]);
        let exp: &[u8] = &[
            24, 42, // timestamp
            34, 128, 128, 2, // tag + size of value1
        ];
        assert_eq!(exp, &write[84..90]);
        assert_eq!(&value1, &write[90..32858]);
        // The first half of the second put.
        let exp: &[u8] = &[
            11, // size of header
            80, 147, 255, 1, // size: uint64
            88, 2, // discriminant: uint32
            101, 237, 98, 210, 156, // crc3c: fixed32
        ];
        assert_eq!(exp, &write[32858..32870]);
        let exp: &[u8] = &[
            66, 202, 128, 2, // tag, length of KeyValueEntry::Put
            8, 0, // shared
            18, 64 // key2_frag tag + sz
        ];
        assert_eq!(exp, &write[32870..32878]);
        assert_eq!(key2, &write[32878..32942]);
        let exp: &[u8] = &[
            24, 99, // timestamp
            34, 128, 128, 2, // tag + size of value2
        ];
        assert_eq!(exp, &write[32942..32948]);
        assert_eq!(&value2[..32581], &write[32948..65529]);
        let exp: &[u8] = &[0, 0, 0, 0, 0, 0, 0]; // true up
        assert_eq!(exp, &write[65529..65536]);
        // The second half of the second put.
        let exp: &[u8] = &[
            10, // size of header
            80, 187, 1, // size: uint64
            88, 3, // discriminant: uint32
            101, 61, 120, 150, 79, // crc3c: fixed32
        ];
        assert_eq!(exp, &write[65536..65547]);
        assert_eq!(&value2[32581..], &write[65547..65734]);
        let exp: &[u8] = &[];
        assert_eq!(exp, &write[65734..]);
    }
}
