//! A log is an unordered table.

use std::cmp::Ordering;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write as IoWrite};
use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;
use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;

use biometrics::{Collector, Counter};
use buffertk::{stack_pack, v64, Packable, Unpackable};
use keyvalint::{KeyValuePair, KeyValueRef};
use prototk_derive::Message;
use sync42::work_coalescing_queue::{WorkCoalescingCore, WorkCoalescingQueue};
use zerror::Z;
use zerror_core::ErrorCore;

use super::setsum::Setsum;
use super::{
    check_key_len, check_table_size, check_value_len, compare_key, Builder, Error, KeyValueDel,
    KeyValueEntry, KeyValuePut, TABLE_FULL_SIZE,
};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static APPEND: Counter = Counter::new("sst.log.append");
static FSYNC: Counter = Counter::new("sst.log.fsync");

/// Register the biometrics for the log.
pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&APPEND);
    collector.register_counter(&FSYNC);
}

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The maximum batch size that can be written to a log.
pub const MAX_BATCH_SIZE: u64 = BLOCK_SIZE - 2 * HEADER_MAX_SIZE;

const BLOCK_BITS: u64 = 20;
const BLOCK_SIZE: u64 = 1 << BLOCK_BITS;
const DEFAULT_BUFFER_SIZE: u64 = BLOCK_SIZE * 2;

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

fn check_batch_size(size: usize) -> Result<(), Error> {
    if size as u64 > BLOCK_SIZE {
        let err = Error::TableFull {
            core: ErrorCore::default(),
            size,
            limit: BLOCK_SIZE as usize,
        };
        Err(err)
    } else {
        Ok(())
    }
}

fn check_batch_size_plus<P: Packable>(buffer: &[u8], pa: P) -> Result<(), Error> {
    let size = buffer.len() + pa.pack_sz();
    check_batch_size(size)
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

/// The maximum header size for a log header.
pub const HEADER_MAX_SIZE: u64 = 1 // one byte for size of header
                               + 1 + 10 // size is a varint
                               + 1 + 1 // discriminant is a varint of one byte---always
                               + 1 + 4; // crc32c is fixed32.
const HEADER_WHOLE: u32 = 1;
const HEADER_FIRST: u32 = 2;
const HEADER_SECOND: u32 = 3;

//////////////////////////////////////////// LogOptions ////////////////////////////////////////////

/// Options used for creating and reading logs.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct LogOptions {
    /// The number of bytes to use for a write buffer.
    #[cfg_attr(feature = "command_line", arrrg(optional, "Size of the write buffer."))]
    pub(crate) write_buffer: usize,
    /// The number of bytes to use for a read buffer.
    #[cfg_attr(feature = "command_line", arrrg(optional, "Size of the read buffer."))]
    pub(crate) read_buffer: usize,
    /// The size at which to rollover the log for multi-file log builders.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Roll over logs that exceed this number of bytes.")
    )]
    pub(crate) rollover_size: usize,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            write_buffer: DEFAULT_BUFFER_SIZE as usize,
            read_buffer: DEFAULT_BUFFER_SIZE as usize,
            rollover_size: 1 << 30,
        }
    }
}

/////////////////////////////////////////////// Write //////////////////////////////////////////////

/// An extension of std::io::Write that does fsync.
pub trait Write: std::io::Write {
    /// Return when the data is known to be durable.
    fn fsync(&mut self) -> Result<(), Error>;
}

impl Write for File {
    fn fsync(&mut self) -> Result<(), Error> {
        Ok(self.sync_data()?)
    }
}

impl Write for &mut Vec<u8> {
    fn fsync(&mut self) -> Result<(), Error> {
        // pass
        Ok(())
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn fsync(&mut self) -> Result<(), Error> {
        self.get_mut().fsync()
    }
}

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

/// A WriteBatch for appending to a log.
#[derive(Clone, Debug, Default)]
pub struct WriteBatch {
    buffer: Vec<u8>,
    setsum: Setsum,
}

impl WriteBatch {
    /// Insert the key-value pair into the write batch.
    pub fn insert(&mut self, kvr: KeyValueRef<'_>) -> Result<(), Error> {
        if let Some(value) = kvr.value {
            self.put(kvr.key, kvr.timestamp, value)
        } else {
            self.del(kvr.key, kvr.timestamp)
        }
    }

    /// Merge one batch into an other.  Will only error if the resulting batch size is too large.
    pub fn merge(&mut self, wb: &WriteBatch) -> Result<(), Error> {
        check_batch_size(self.buffer.len() + wb.buffer.len())?;
        self.buffer.extend_from_slice(&wb.buffer);
        self.setsum += wb.setsum;
        Ok(())
    }
}

impl Builder for WriteBatch {
    type Sealed = Self;

    fn approximate_size(&self) -> usize {
        self.buffer.len()
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
        let pa = stack_pack(KeyValueEntry::Put(put));
        check_batch_size_plus(&self.buffer, &pa)?;
        pa.append_to_vec(&mut self.buffer);
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
        let pa = stack_pack(KeyValueEntry::Del(del));
        check_batch_size_plus(&self.buffer, &pa)?;
        pa.append_to_vec(&mut self.buffer);
        Ok(())
    }

    fn seal(self) -> Result<Self::Sealed, Error> {
        Ok(self)
    }
}

//////////////////////////////////////////// LogBuilder ////////////////////////////////////////////

/// A LogBuilder is a non-concurrent log writer.
pub struct LogBuilder<W: Write> {
    options: LogOptions,
    output: BufWriter<W>,
    bytes_written: u64,
    setsum: Setsum,
}

impl LogBuilder<File> {
    /// Create a new log builder with the provided options at the prescribed path.
    pub fn new<P: AsRef<Path>>(options: LogOptions, file_name: P) -> Result<Self, Error> {
        let file: File = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(file_name)?;
        Self::from_write(options, file)
    }

    /// fsync the log builder.
    pub fn fsync(&mut self) -> Result<(), Error> {
        FSYNC.click();
        self.output.flush()?;
        Ok(self.output.get_mut().sync_data()?)
    }
}

impl<W: Write> LogBuilder<W> {
    /// Create a new LogBuilder from options and a write.
    pub fn from_write(options: LogOptions, write: W) -> Result<Self, Error> {
        let output = BufWriter::with_capacity(options.write_buffer, write);
        Ok(Self {
            options,
            output,
            bytes_written: 0,
            setsum: Setsum::default(),
        })
    }

    /// Flush the log to the OS.  This does not call fsync.
    pub fn flush(&mut self) -> Result<(), Error> {
        self.output.flush()?;
        Ok(())
    }

    /// Append a write batch to the log.
    pub fn append(&mut self, write_batch: &WriteBatch) -> Result<(), Error> {
        if write_batch.buffer.is_empty() {
            return Err(Error::EmptyBatch {
                core: ErrorCore::default(),
            });
        }
        assert_ne!(write_batch.setsum, Setsum::default());
        self.setsum += write_batch.setsum;
        self._append(&write_batch.buffer)
    }

    fn _append(&mut self, buffer: &[u8]) -> Result<(), Error> {
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
            return self._append(buffer);
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
        let mut wb = WriteBatch::default();
        wb.put(key, timestamp, value)?;
        self.append(&wb)?;
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        let mut wb = WriteBatch::default();
        wb.del(key, timestamp)?;
        self.append(&wb)?;
        Ok(())
    }

    fn seal(mut self) -> Result<Self::Sealed, Error> {
        self.flush()?;
        Ok(self.setsum)
    }
}

/////////////////////////////////////// ConcurrentLogBuilder ///////////////////////////////////////

struct WriteCoalescingCore<W: Write> {
    builder: LogBuilder<W>,
    written: u64,
}

impl<W: Write> WorkCoalescingCore<Arc<WriteBatch>, Result<u64, Error>> for WriteCoalescingCore<W> {
    type InputAccumulator = WriteBatch;
    type OutputIterator<'a> = std::vec::IntoIter<Result<u64, Error>> where W: 'a;

    fn can_batch(&self, acc: &WriteBatch, other: &Arc<WriteBatch>) -> bool {
        check_batch_size(acc.buffer.len().saturating_add(other.buffer.len())).is_ok()
    }

    fn batch(&mut self, mut acc: WriteBatch, other: Arc<WriteBatch>) -> Self::InputAccumulator {
        acc.merge(&other)
            .expect("can_batch should ensure this is impossible");
        acc
    }

    fn work(&mut self, taken: usize, acc: Self::InputAccumulator) -> Self::OutputIterator<'_> {
        let mut ret = Vec::with_capacity(taken);
        self.written += acc.buffer.len() as u64;
        if let Err(err) = self.builder.append(&acc) {
            for _ in 0..taken {
                ret.push(Err(err.clone()))
            }
        } else if let Err(err) = self.builder.flush() {
            for _ in 0..taken {
                ret.push(Err(err.clone()))
            }
        } else {
            for _ in 0..taken {
                ret.push(Ok(self.written))
            }
        }
        ret.into_iter()
    }
}

struct FsyncCoalescingCore {
    raw_builder: RawFd,
    synced: u64,
}

impl WorkCoalescingCore<u64, bool> for FsyncCoalescingCore {
    type InputAccumulator = u64;
    type OutputIterator<'a> = std::vec::IntoIter<bool>;

    fn can_batch(&self, acc: &u64, input: &u64) -> bool {
        if *acc > 0 && *acc <= self.synced && *input <= self.synced {
            true
        } else if *acc > 0 && *acc <= self.synced {
            false
        } else {
            true
        }
    }

    fn batch(&mut self, acc: u64, seen: u64) -> Self::InputAccumulator {
        std::cmp::max(acc, seen)
    }

    fn work(&mut self, taken: usize, acc: Self::InputAccumulator) -> Self::OutputIterator<'_> {
        FSYNC.click();
        if self.synced >= acc {
            vec![true; taken].into_iter()
        } else {
            // SAFETY(rescrv):  The worst thing that can happen is we fsync on a fd that's not ours.
            let ret = unsafe { libc::fdatasync(self.raw_builder) } >= 0;
            if ret {
                self.synced = acc;
            }
            vec![ret; taken].into_iter()
        }
    }
}

/// A ConcurrentLogBuilder provides a non-standard builder interface that is internally
/// synchronized.  This will be orders of magnitude faster than standard LogBuilder.
pub struct ConcurrentLogBuilder<W: Write> {
    write_cq: WorkCoalescingQueue<Arc<WriteBatch>, Result<u64, Error>, WriteCoalescingCore<W>>,
    // TODO(rescrv): Make this return Option<Error> too.
    fsync_cq: WorkCoalescingQueue<u64, bool, FsyncCoalescingCore>,
    poison: AtomicBool,
    _phantom_w: std::marker::PhantomData<W>,
}

impl ConcurrentLogBuilder<File> {
    /// Create a new concurrent log builder.
    pub fn new<P: AsRef<Path>>(options: LogOptions, file_name: P) -> Result<Self, Error> {
        let builder = LogBuilder::new(options, file_name)?;
        Self::from_builder(builder)
    }

    /// fsync the data to disk.  This will return when all previously written data is durable.
    pub fn fsync(&self) -> Result<(), Error> {
        if !self.fsync_cq.do_work(0) {
            Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "log is poisoned".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

impl<W: Write + AsRawFd> ConcurrentLogBuilder<W> {
    /// Create a new ConcurrentLogBuilder from the given writer.
    pub fn from_write(options: LogOptions, write: W) -> Result<Self, Error> {
        let builder = LogBuilder::from_write(options, write)?;
        Self::from_builder(builder)
    }

    /// Create a new ConcurrentLogBuilder from the provided builder.
    pub fn from_builder(builder: LogBuilder<W>) -> Result<Self, Error> {
        let raw_builder = builder.output.get_ref().as_raw_fd();
        let write_cq = WorkCoalescingQueue::new(WriteCoalescingCore { builder, written: 0 });
        let fsync_cq = WorkCoalescingQueue::new(FsyncCoalescingCore { raw_builder, synced: 0 });
        let poison = AtomicBool::new(false);
        let _phantom_w = std::marker::PhantomData;
        Ok(Self {
            write_cq,
            fsync_cq,
            poison,
            _phantom_w,
        })
    }

    /// Flush data to the OS.
    pub fn flush(&self) -> Result<(), Error> {
        let mut write = self.write_cq.get_core();
        write.builder.flush()
    }

    /// Return the approximate number of bytes written.
    pub fn approximate_size(&self) -> usize {
        let write = self.write_cq.get_core();
        write.builder.approximate_size()
    }

    /// Append `write_batch` to the log.
    ///
    /// Returns after fsync is done.
    pub fn append(&self, write_batch: WriteBatch) -> Result<(), Error> {
        if write_batch.buffer.is_empty() {
            return Err(Error::EmptyBatch {
                core: ErrorCore::default(),
            });
        }
        let written = match self.write_cq.do_work(Arc::new(write_batch)) {
            Ok(written) => written,
            Err(err) => {
                self.poison.store(true, atomic::Ordering::Relaxed);
                return Err(err);
            }
        };
        if !self.fsync_cq.do_work(written) {
            self.poison.store(true, atomic::Ordering::Relaxed);
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "fsync failed".to_string(),
            };
            return Err(err);
        }
        Ok(())
    }

    /// Put a key-value pair in the log.
    ///
    /// Returns after fsync is done.
    pub fn put(&self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        let mut wb = WriteBatch::default();
        wb.put(key, timestamp, value)?;
        self.append(wb)?;
        Ok(())
    }

    /// Put a key-value tombstone in the log.
    ///
    /// Returns after fsync is done.
    pub fn del(&self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        let mut wb = WriteBatch::default();
        wb.del(key, timestamp)?;
        self.append(wb)?;
        Ok(())
    }

    /// Seal the log and return its setsum.
    pub fn seal(self) -> Result<Setsum, Error> {
        let core = self.write_cq.into_inner();
        core.builder.seal()
    }
}

//////////////////////////////////////////// LogIterator ///////////////////////////////////////////

/// An iterator over logs.
pub struct LogIterator<R: Read + Seek> {
    input: BufReader<R>,
    buffer: Vec<u8>,
    buffer_idx: usize,
}

impl LogIterator<File> {
    /// Open `file_name` using `options` as a guide and return a [LogIterator].
    pub fn new<P: AsRef<Path>>(options: LogOptions, file_name: P) -> Result<Self, Error> {
        let file: File = OpenOptions::new()
            .create(false)
            .read(true)
            .open(file_name)?;
        Self::from_reader(options, file)
    }
}

impl<R: Read + Seek> LogIterator<R> {
    /// Create a new [LogIterator] from options and a reader.
    pub fn from_reader(options: LogOptions, reader: R) -> Result<Self, Error> {
        let input = BufReader::with_capacity(options.read_buffer, reader);
        Ok(Self {
            input,
            buffer: vec![],
            buffer_idx: 0,
        })
    }

    /// Return the next item in the log, or None when the log has been traversed.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<KeyValueRef>, Error> {
        if self.buffer_idx < self.buffer.len() {
            return self.next_from_buffer();
        }
        self.buffer_idx = 0;
        self.buffer.clear();
        let header = match self.next_frame()? {
            Some(header) => header,
            None => {
                return Ok(None);
            }
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
                    .with_variable("header1", header)
                    .with_variable("offset", self.input.stream_position().unwrap_or(0)));
                }
            };
            if header2.discriminant != HEADER_SECOND {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "invalid discriminant in second header".to_owned(),
                }
                .with_variable("discriminant", header2.discriminant)
                .with_variable("offset", self.input.stream_position().unwrap_or(0)));
            }
        } else {
            return Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "invalid discriminant in header".to_owned(),
            }
            .with_variable("discriminant", header.discriminant)
            .with_variable("offset", self.input.stream_position().unwrap_or(0)));
        }
        self.next_from_buffer()
    }

    fn next_from_buffer(&mut self) -> Result<Option<KeyValueRef>, Error> {
        if self.buffer_idx >= self.buffer.len() {
            return Err(Error::EmptyBatch {
                core: ErrorCore::default(),
            });
        }
        let (kve, rem) = <KeyValueEntry as Unpackable>::unpack(&self.buffer[self.buffer_idx..])?;
        self.buffer_idx = self.buffer.len() - rem.len();
        fn check_shared(shared: u64) -> Result<(), Error> {
            if shared != 0 {
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "shared was not 0".to_string(),
                })
            } else {
                Ok(())
            }
        }
        match &kve {
            KeyValueEntry::Put(KeyValuePut {
                shared,
                key_frag,
                timestamp,
                value,
            }) => {
                check_shared(*shared)?;
                Ok(Some(KeyValueRef {
                    key: key_frag,
                    timestamp: *timestamp,
                    value: Some(value),
                }))
            }
            KeyValueEntry::Del(KeyValueDel {
                shared,
                key_frag,
                timestamp,
            }) => {
                check_shared(*shared)?;
                Ok(Some(KeyValueRef {
                    key: key_frag,
                    timestamp: *timestamp,
                    value: None,
                }))
            }
        }
    }

    fn next_frame(&mut self) -> Result<Option<Header>, Error> {
        let header = match self.next_header()? {
            Some(header) => header,
            None => {
                return Ok(None);
            }
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
            .with_variable("returned", crc)
            .with_variable("offset", self.input.stream_position().unwrap_or(0)));
        }
        Ok(Some(header))
    }

    fn next_header(&mut self) -> Result<Option<Header>, Error> {
        'looping: loop {
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
                }
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
                .with_variable("header_sz", header_sz)
                .with_variable("offset", self.input.stream_position().unwrap_or(0)));
            }
            let header = &mut header[..header_sz];
            self.input.read_exact(header)?;
            let header: Header = <Header as Unpackable>::unpack(header)?.0;
            if header.size > TABLE_FULL_SIZE as u64 {
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "entry size exceeds TABLE_FULL_SIZE".to_owned(),
                }
                .with_variable("size", header.size)
                .with_variable("offset", self.input.stream_position().unwrap_or(0)));
            }
            return Ok(Some(header));
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
            .with_variable("trued_up", trued_up)
            .with_variable("offset", self.input.stream_position().unwrap_or(0)));
        }
        self.input.seek(SeekFrom::Start(trued_up))?;
        Ok(())
    }
}

////////////////////////////////////////// log_to_builder //////////////////////////////////////////

/// Given a log, write it out to the provided builder.
pub fn log_to_builder<P: AsRef<Path>, B: Builder>(
    log_options: LogOptions,
    log_path: P,
    mut builder: B,
) -> Result<Option<B::Sealed>, Error> {
    let mut log_iter = LogIterator::new(log_options, log_path)?;
    let mut kvrs = Vec::new();
    while let Some(kvr) = log_iter.next().unwrap() {
        kvrs.push(KeyValuePair::from(kvr));
    }
    fn sort_key(lhs: &KeyValuePair, rhs: &KeyValuePair) -> Ordering {
        compare_key(&lhs.key, lhs.timestamp, &rhs.key, rhs.timestamp)
    }
    kvrs.sort_by(sort_key);
    if kvrs.is_empty() {
        return Ok(None);
    }
    for kvr in kvrs.into_iter() {
        match kvr.value {
            Some(v) => {
                builder.put(&kvr.key, kvr.timestamp, &v)?;
            }
            None => {
                builder.del(&kvr.key, kvr.timestamp)?;
            }
        }
    }
    builder.seal().map(Some)
}

/////////////////////////////////////////// log_to_setsum //////////////////////////////////////////

/// Given a log, read and compute its setsum.
pub fn log_to_setsum<P: AsRef<Path>>(
    log_options: LogOptions,
    log_path: P,
) -> Result<Setsum, Error> {
    let mut log_iter = LogIterator::new(log_options, log_path)?;
    let mut acc = Setsum::default();
    while let Some(kvr) = log_iter.next().unwrap() {
        if let Some(value) = kvr.value.as_ref() {
            acc.put(kvr.key, kvr.timestamp, value);
        } else {
            acc.del(kvr.key, kvr.timestamp);
        }
    }
    Ok(acc)
}

/////////////////////////////////// truncate_final_partial_frame ///////////////////////////////////

/// Return the truncation point for a log that is corrupt with the final_partial_frame corruption.
///
/// This can happen if a process exits between write calls in append_split.
pub fn truncate_final_partial_frame<P: AsRef<Path>>(
    log_options: LogOptions,
    log_path: P,
) -> Result<Option<u64>, Error> {
    let mut iter = LogIterator::new(log_options, log_path)?;
    let mut offset = 0;
    let mut last_was_valid = true;
    while let Some(header) = iter.next_frame()? {
        if header.discriminant == HEADER_SECOND || header.discriminant == HEADER_WHOLE {
            offset = iter.input.stream_position()?;
            last_was_valid = true;
        } else {
            last_was_valid = false;
        }
    }
    if !last_was_valid {
        Ok(Some(offset))
    } else {
        Ok(None)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod offsets {
    use super::*;

    #[test]
    fn offsets() {
        assert_eq!(0, block_offset(0));
        assert_eq!(0, block_offset(1048575));
        assert_eq!(1, block_offset(1048576));
        assert_eq!(1, block_offset(2097151));
    }

    #[test]
    fn boundaries() {
        assert_eq!(1048576, next_boundary(0));
        assert_eq!(1048576, next_boundary(1048575));
        assert_eq!(2097152, next_boundary(1048576));
        assert_eq!(2097152, next_boundary(2097151));
    }

    #[test]
    fn true_ups() {
        assert_eq!(0, compute_true_up(0));
        assert_eq!(1048576, compute_true_up(1));
        assert_eq!(1048576, compute_true_up(1048576));
        assert_eq!(2097152, compute_true_up(1048577));
        assert_eq!(2097152, compute_true_up(2097152));
    }
}

#[cfg(test)]
mod builder {
    use super::*;

    #[test]
    fn empty() {
        let mut write = Vec::new();
        let log =
            LogBuilder::from_write(LogOptions::default(), &mut write).expect("should not fail");
        log.seal().expect("seal should not fail");
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
            8, 0, 18, 3, 101, 102, 103, 24, 42, 34, 8, 1, 2, 3, 4, 5, 6, 7, 8,
        ];
        assert_eq!(0x5acc2712, crc32c::crc32c(buf));
    }

    #[test]
    fn insert_one() {
        let mut write = Vec::new();
        let mut log =
            LogBuilder::from_write(LogOptions::default(), &mut write).expect("should not fail");
        log.put(&[101, 102, 103], 42, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
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
            34, 8, 1, 2, 3, 4, 5, 6, 7, 8, // value
        ];
        let got: &[u8] = &write;
        assert_eq!(exp, got);
    }

    #[test]
    fn insert_across_boundary() {
        let mut buffer = Vec::new();
        let mut log =
            LogBuilder::from_write(LogOptions::default(), &mut buffer).expect("should not fail");
        let key = vec![b'A'; 64];
        let value = vec![b'B'; 32768];
        for _ in 0..33 {
            log.put(&key, 42, &value).unwrap();
        }
        log.flush().unwrap();
        drop(log);
        let block_size = BLOCK_SIZE as usize;
        assert_eq!(
            &[
                66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 0, 0, 0, 0, 0, 0, 0, 10, 80,
                199, 22, 88, 3, 101, 44, 249, 80, 107, 66, 66, 66, 66, 66, 66, 66, 66, 66
            ],
            &buffer[block_size - 20..block_size + 20]
        );
    }
}
