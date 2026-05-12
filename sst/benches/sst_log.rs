use std::fs::{File, remove_file};
use std::io::{
    BufReader, BufWriter, Cursor as IoCursor, ErrorKind, Read, Seek, SeekFrom, Write as IoWrite,
};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use buffertk::{Packable, Unpacker, stack_pack, v64};
use statslicer::{Bencher, Parameter, Parameters, benchmark, black_box, statslicer_main};

use sst::block::{Block, BlockBuilderOptions, BlockCursor};
use sst::log::{LogBuilder, LogIterator, LogOptions, WriteBatch};
use sst::{Builder, Cursor, Sst, SstBuilder, SstOptions};

const SST_NUM_KEYS: &[usize] = &[4096, 16384];
const SST_TARGET_BLOCK_SIZES: &[usize] = &[4096, 16384, 65536];
const SST_RESTART_INTERVALS: &[usize] = &[16, 128, 512];
const VALUE_BYTES: &[usize] = &[16, 256];
const SST_LOOKUP_KEYS: usize = 8192;

const LOG_NUM_ENTRIES: &[usize] = &[1024, 4096, 16384];
const LOG_BATCH_ENTRIES: &[usize] = &[1, 64];

const LOG_BLOCK_BITS: u64 = 20;
const LOG_BLOCK_SIZE: u64 = 1 << LOG_BLOCK_BITS;
const LOG_DEFAULT_BUFFER_SIZE: usize = (LOG_BLOCK_SIZE * 2) as usize;
const LOG_HEADER_MAX_SIZE: usize = 1 // one byte for size of header
                                + 1 + 10 // size is a varint
                                + 1 + 1 // discriminant is a varint of one byte
                                + 1 + 4; // crc32c is fixed32.
const LOG_HEADER_WHOLE: u32 = 1;
const LOG_HEADER_FIRST: u32 = 2;
const LOG_HEADER_SECOND: u32 = 3;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SstScanParameters {
    num_keys: usize,
    target_block_size: usize,
    restart_interval: usize,
    value_bytes: usize,
}

impl Parameters for SstScanParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("num_keys", Parameter::Integer(self.num_keys as u64)),
            (
                "target_block_size",
                Parameter::Integer(self.target_block_size as u64),
            ),
            (
                "restart_interval",
                Parameter::Integer(self.restart_interval as u64),
            ),
            ("value_bytes", Parameter::Integer(self.value_bytes as u64)),
        ]
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LogParameters {
    num_entries: usize,
    batch_entries: usize,
    value_bytes: usize,
}

impl Parameters for LogParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("num_entries", Parameter::Integer(self.num_entries as u64)),
            (
                "batch_entries",
                Parameter::Integer(self.batch_entries as u64),
            ),
            ("value_bytes", Parameter::Integer(self.value_bytes as u64)),
        ]
    }
}

fn key(index: usize) -> Vec<u8> {
    format!("{index:016x}").into_bytes()
}

fn temp_path(name: &str) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, AtomicOrdering::Relaxed);
    std::env::temp_dir().join(format!(
        "sst-log-bench-{name}-{}-{id}.sst",
        std::process::id()
    ))
}

fn build_sst(params: &SstScanParameters) -> (Sst, PathBuf) {
    let block_options = BlockBuilderOptions::default()
        .bytes_restart_interval(u32::MAX)
        .key_value_pairs_restart_interval(params.restart_interval as u32);
    let options = SstOptions::default()
        .block(block_options)
        .target_block_size(params.target_block_size as u32);
    let path = temp_path("table");
    let _ = remove_file(&path);
    let mut builder = SstBuilder::new(options, &path).unwrap();
    let value = vec![0x42; params.value_bytes];
    for index in 0..params.num_keys {
        builder.put(&key(index), 0, &value).unwrap();
    }
    let table = builder.seal().unwrap();
    (table, path)
}

#[derive(Clone)]
struct BaselineSst {
    file: Arc<File>,
    index_block: Block,
}

impl BaselineSst {
    fn new(path: &PathBuf) -> Self {
        let mut file = File::open(path).unwrap();
        let file_size = file.seek(SeekFrom::End(0)).unwrap();
        let final_block_offset_position = file_size - 8;
        let mut final_block_offset_buf = [0u8; 8];
        file.read_exact_at(&mut final_block_offset_buf, final_block_offset_position)
            .unwrap();
        let mut up = Unpacker::new(&final_block_offset_buf);
        let final_block_offset: u64 = up.unpack().unwrap();
        let final_block_size = final_block_offset_position + 8 - final_block_offset;
        let mut final_block_buf = vec![0u8; final_block_size as usize];
        file.read_exact_at(&mut final_block_buf, final_block_offset)
            .unwrap();
        let mut up = Unpacker::new(&final_block_buf);
        let final_block: BenchFinalBlock = up.unpack().unwrap();
        let file = Arc::new(file);
        let index_block = Self::load_block_from_file(&file, &final_block.index_block);
        Self { file, index_block }
    }

    fn cursor(&self) -> BaselineSstCursor {
        BaselineSstCursor {
            table: self.clone(),
            meta_cursor: self.index_block.cursor(),
            block_cursor: None,
        }
    }

    fn load_block(&self, metadata: &BenchBlockMetadata) -> Block {
        Self::load_block_from_file(&self.file, metadata)
    }

    fn load_block_from_file(file: &File, metadata: &BenchBlockMetadata) -> Block {
        let len = (metadata.limit - metadata.start) as usize;
        let mut buf = vec![0u8; len];
        file.read_exact_at(&mut buf, metadata.start).unwrap();
        let mut up = Unpacker::new(&buf);
        let entry: BenchSstEntry = up.unpack().unwrap();
        match entry {
            BenchSstEntry::PlainBlock(bytes) => Block::new(bytes.to_vec()).unwrap(),
            BenchSstEntry::FilterBlock(_) | BenchSstEntry::FinalBlock(_) => {
                panic!("expected a plain block")
            }
        }
    }
}

struct BaselineSstCursor {
    table: BaselineSst,
    meta_cursor: BlockCursor,
    block_cursor: Option<BlockCursor>,
}

impl BaselineSstCursor {
    fn metadata_from_current(&self) -> Option<BenchBlockMetadata> {
        let kvr = self.meta_cursor.key_value()?;
        let value = kvr.value.unwrap();
        let mut up = Unpacker::new(value);
        Some(up.unpack().unwrap())
    }

    fn load_block_cursor_from_current_meta(&self) -> Option<BlockCursor> {
        let metadata = self.metadata_from_current()?;
        Some(self.table.load_block(&metadata).cursor())
    }
}

impl Cursor for BaselineSstCursor {
    fn seek_to_first(&mut self) -> Result<(), sst::SError> {
        self.meta_cursor.seek_to_first()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), sst::SError> {
        self.meta_cursor.seek_to_last()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), sst::SError> {
        self.meta_cursor.seek(key)?;
        let Some(mut block_cursor) = self.load_block_cursor_from_current_meta() else {
            return self.seek_to_last();
        };
        block_cursor.seek(key)?;
        if block_cursor.key().is_none() {
            self.meta_cursor.next()?;
            let Some(mut next_block_cursor) = self.load_block_cursor_from_current_meta() else {
                return self.seek_to_last();
            };
            next_block_cursor.seek(key)?;
            block_cursor = next_block_cursor;
        }
        self.block_cursor = Some(block_cursor);
        Ok(())
    }

    fn prev(&mut self) -> Result<(), sst::SError> {
        loop {
            if self.block_cursor.is_none() {
                self.meta_cursor.prev()?;
                let Some(mut block_cursor) = self.load_block_cursor_from_current_meta() else {
                    return self.seek_to_first();
                };
                block_cursor.seek_to_last()?;
                self.block_cursor = Some(block_cursor);
            }
            let block_cursor = self.block_cursor.as_mut().unwrap();
            block_cursor.prev()?;
            if block_cursor.key_value().is_some() {
                return Ok(());
            }
            self.block_cursor = None;
        }
    }

    fn next(&mut self) -> Result<(), sst::SError> {
        loop {
            if self.block_cursor.is_none() {
                self.meta_cursor.next()?;
                let Some(mut block_cursor) = self.load_block_cursor_from_current_meta() else {
                    return self.seek_to_last();
                };
                block_cursor.seek_to_first()?;
                self.block_cursor = Some(block_cursor);
            }
            let block_cursor = self.block_cursor.as_mut().unwrap();
            block_cursor.next()?;
            if block_cursor.key_value().is_some() {
                return Ok(());
            }
            self.block_cursor = None;
        }
    }

    fn key(&self) -> Option<sst::KeyRef<'_>> {
        self.block_cursor.as_ref().and_then(|cursor| cursor.key())
    }

    fn value(&self) -> Option<&'_ [u8]> {
        self.block_cursor.as_ref().and_then(|cursor| cursor.value())
    }
}

fn lookup_keys(params: &SstScanParameters) -> Vec<Vec<u8>> {
    let mut state = 0x9e37_79b9_7f4a_7c15u64
        ^ params.num_keys as u64
        ^ ((params.target_block_size as u64) << 17)
        ^ ((params.restart_interval as u64) << 33)
        ^ ((params.value_bytes as u64) << 49);
    let high = params.num_keys.saturating_sub(1).max(1);
    (0..SST_LOOKUP_KEYS)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            key(1 + (state as usize % high))
        })
        .collect()
}

fn observe_key_value(kvr: Option<sst::KeyValueRef<'_>>) -> usize {
    if let Some(kvr) = kvr {
        black_box(kvr.key);
        black_box(kvr.timestamp);
        black_box(kvr.value);
        1
    } else {
        0
    }
}

fn bench_sst_seek_forward_optimized(params: &SstScanParameters, b: &mut Bencher) {
    let (table, path) = build_sst(params);
    let lookup_keys = lookup_keys(params);
    let size = b.size();
    b.run(|| {
        let mut observed = 0;
        for idx in 0..size {
            let mut cursor = table.cursor();
            cursor.seek(&lookup_keys[idx % lookup_keys.len()]).unwrap();
            observed += observe_key_value(cursor.key_value());
        }
        black_box(observed);
    });
    let _ = remove_file(path);
}

fn bench_sst_seek_forward_baseline(params: &SstScanParameters, b: &mut Bencher) {
    let (table, path) = build_sst(params);
    let baseline = BaselineSst::new(&path);
    drop(table);
    let lookup_keys = lookup_keys(params);
    let size = b.size();
    b.run(|| {
        let mut observed = 0;
        for idx in 0..size {
            let mut cursor = baseline.cursor();
            cursor.seek(&lookup_keys[idx % lookup_keys.len()]).unwrap();
            observed += observe_key_value(cursor.key_value());
        }
        black_box(observed);
    });
    let _ = remove_file(path);
}

fn bench_sst_seek_reverse_optimized(params: &SstScanParameters, b: &mut Bencher) {
    let (table, path) = build_sst(params);
    let lookup_keys = lookup_keys(params);
    let size = b.size();
    b.run(|| {
        let mut observed = 0;
        for idx in 0..size {
            let mut cursor = table.cursor();
            cursor.seek(&lookup_keys[idx % lookup_keys.len()]).unwrap();
            cursor.prev().unwrap();
            observed += observe_key_value(cursor.key_value());
        }
        black_box(observed);
    });
    let _ = remove_file(path);
}

fn bench_sst_seek_reverse_baseline(params: &SstScanParameters, b: &mut Bencher) {
    let (table, path) = build_sst(params);
    let baseline = BaselineSst::new(&path);
    drop(table);
    let lookup_keys = lookup_keys(params);
    let size = b.size();
    b.run(|| {
        let mut observed = 0;
        for idx in 0..size {
            let mut cursor = baseline.cursor();
            cursor.seek(&lookup_keys[idx % lookup_keys.len()]).unwrap();
            cursor.prev().unwrap();
            observed += observe_key_value(cursor.key_value());
        }
        black_box(observed);
    });
    let _ = remove_file(path);
}

#[derive(Clone, Debug)]
struct BenchBatch {
    write_batch: WriteBatch,
    bytes: Vec<u8>,
    setsum: sst::Setsum,
}

fn build_batches(params: &LogParameters) -> Vec<BenchBatch> {
    let value = vec![0x42; params.value_bytes];
    let mut batches = Vec::new();
    let mut index = 0;
    while index < params.num_entries {
        let limit = params.num_entries.min(index + params.batch_entries);
        let mut batch = WriteBatch::default();
        let mut bytes = Vec::new();
        let mut setsum = sst::Setsum::default();
        while index < limit {
            let key = key(index);
            batch.put(&key, 0, &value).unwrap();
            setsum.put(&key, 0, &value);
            let put = BenchKeyValueEntry::Put(BenchKeyValuePut {
                shared: 0,
                key_frag: &key,
                timestamp: 0,
                value: &value,
            });
            stack_pack(put).append_to_vec(&mut bytes);
            index += 1;
        }
        batches.push(BenchBatch {
            write_batch: batch,
            bytes,
            setsum,
        });
    }
    batches
}

fn build_log(params: &LogParameters) -> Vec<u8> {
    let batches = build_batches(params);
    let mut bytes = Vec::new();
    {
        let mut builder = LogBuilder::from_write(LogOptions::default(), &mut bytes).unwrap();
        for batch in &batches {
            builder.append(&batch.write_batch).unwrap();
        }
        builder.flush().unwrap();
    }
    bytes
}

fn bench_log_write_optimized(params: &LogParameters, b: &mut Bencher) {
    let batches = build_batches(params);
    let size = b.size();
    b.run(|| {
        let mut total = 0;
        for _ in 0..size {
            let mut bytes = Vec::new();
            let mut builder = LogBuilder::from_write(LogOptions::default(), &mut bytes).unwrap();
            for batch in &batches {
                builder.append(&batch.write_batch).unwrap();
            }
            builder.flush().unwrap();
            drop(builder);
            total += bytes.len();
        }
        black_box(total);
    });
}

fn bench_log_write_baseline(params: &LogParameters, b: &mut Bencher) {
    let batches = build_batches(params);
    let size = b.size();
    b.run(|| {
        let mut total = 0;
        for _ in 0..size {
            let mut bytes = Vec::new();
            let mut builder = BaselineLogBuilder::new(&mut bytes);
            for batch in &batches {
                builder.append(batch);
            }
            builder.flush();
            drop(builder);
            total += bytes.len();
        }
        black_box(total);
    });
}

fn bench_log_iterate_optimized(params: &LogParameters, b: &mut Bencher) {
    let bytes = build_log(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let mut iter =
                LogIterator::from_reader(LogOptions::default(), IoCursor::new(&bytes)).unwrap();
            while let Some(kvr) = iter.next().unwrap() {
                black_box(kvr.key);
                black_box(kvr.timestamp);
                black_box(kvr.value);
                visited += 1;
            }
        }
        black_box(visited);
    });
}

fn bench_log_iterate_baseline(params: &LogParameters, b: &mut Bencher) {
    let bytes = build_log(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let mut iter = BaselineLogIterator::new(IoCursor::new(&bytes));
            while let Some(kvr) = iter.next() {
                black_box(kvr.key);
                black_box(kvr.timestamp);
                black_box(kvr.value);
                visited += 1;
            }
        }
        black_box(visited);
    });
}

struct BaselineLogBuilder<W: sst::log::Write> {
    output: BufWriter<W>,
    bytes_written: u64,
    setsum: sst::Setsum,
}

impl<W: sst::log::Write> BaselineLogBuilder<W> {
    fn new(write: W) -> Self {
        Self {
            output: BufWriter::with_capacity(LOG_DEFAULT_BUFFER_SIZE, write),
            bytes_written: 0,
            setsum: sst::Setsum::default(),
        }
    }

    fn append(&mut self, batch: &BenchBatch) {
        assert!(!batch.bytes.is_empty());
        assert_ne!(batch.setsum, sst::Setsum::default());
        self.setsum += batch.setsum;
        self.append_bytes(&batch.bytes);
    }

    fn append_bytes(&mut self, buffer: &[u8]) {
        let header = BenchHeader {
            size: buffer.len() as u64,
            discriminant: LOG_HEADER_WHOLE,
            crc32c: crc32c::crc32c(buffer),
        };
        let header = pack_log_header_to_vec(&header);
        let new_offset = self.bytes_written + (header.len() + buffer.len()) as u64;
        if new_offset > next_log_boundary(self.bytes_written) {
            self.append_split(buffer);
        } else {
            self.write(&header);
            self.write(buffer);
        }
    }

    fn append_split(&mut self, buffer: &[u8]) {
        let nb = next_log_boundary(self.bytes_written);
        let roundup = nb - self.bytes_written;
        if roundup <= LOG_HEADER_MAX_SIZE as u64 {
            self.true_up(nb);
            self.append_bytes(buffer);
            return;
        }
        let first_bytes = (roundup - LOG_HEADER_MAX_SIZE as u64) as usize;
        let first = &buffer[..first_bytes];
        let second = &buffer[first_bytes..];
        let first_header = pack_log_header_to_vec(&BenchHeader {
            size: first.len() as u64,
            discriminant: LOG_HEADER_FIRST,
            crc32c: crc32c::crc32c(first),
        });
        let second_header = pack_log_header_to_vec(&BenchHeader {
            size: second.len() as u64,
            discriminant: LOG_HEADER_SECOND,
            crc32c: crc32c::crc32c(second),
        });
        self.write(&first_header);
        self.write(first);
        self.true_up(nb);
        self.write(&second_header);
        self.write(second);
    }

    fn true_up(&mut self, nb: u64) {
        let roundup = (nb - self.bytes_written) as usize;
        self.write(&[0u8; LOG_HEADER_MAX_SIZE][..roundup]);
    }

    fn write(&mut self, buffer: &[u8]) {
        self.output.write_all(buffer).unwrap();
        self.bytes_written += buffer.len() as u64;
    }

    fn flush(&mut self) {
        self.output.flush().unwrap();
    }
}

struct BaselineLogIterator<R: Read + Seek> {
    input: BufReader<R>,
    buffer: Vec<u8>,
    buffer_idx: usize,
}

impl<R: Read + Seek> BaselineLogIterator<R> {
    fn new(reader: R) -> Self {
        Self {
            input: BufReader::with_capacity(LOG_DEFAULT_BUFFER_SIZE, reader),
            buffer: Vec::new(),
            buffer_idx: 0,
        }
    }

    fn next(&mut self) -> Option<BenchKeyValueRef<'_>> {
        if self.buffer_idx < self.buffer.len() {
            return self.next_from_buffer();
        }
        self.buffer_idx = 0;
        self.buffer.clear();
        let header = self.next_frame()?;
        if header.discriminant == LOG_HEADER_WHOLE {
        } else if header.discriminant == LOG_HEADER_FIRST {
            self.true_up();
            let header = self.next_frame().unwrap();
            assert_eq!(header.discriminant, LOG_HEADER_SECOND);
        } else {
            panic!("invalid log frame discriminant");
        }
        self.next_from_buffer()
    }

    fn next_from_buffer(&mut self) -> Option<BenchKeyValueRef<'_>> {
        if self.buffer_idx >= self.buffer.len() {
            return None;
        }
        let mut up = Unpacker::new(&self.buffer[self.buffer_idx..]);
        let entry: BenchKeyValueEntry = up.unpack().unwrap();
        self.buffer_idx = self.buffer.len() - up.remain().len();
        match entry {
            BenchKeyValueEntry::Put(put) => {
                assert_eq!(put.shared, 0);
                Some(BenchKeyValueRef {
                    key: put.key_frag,
                    timestamp: put.timestamp,
                    value: Some(put.value),
                })
            }
            BenchKeyValueEntry::Del(del) => {
                assert_eq!(del.shared, 0);
                Some(BenchKeyValueRef {
                    key: del.key_frag,
                    timestamp: del.timestamp,
                    value: None,
                })
            }
        }
    }

    fn next_frame(&mut self) -> Option<BenchHeader> {
        let header = self.next_header()?;
        let buffer_start = self.buffer.len();
        let buffer_limit = buffer_start + header.size as usize;
        self.buffer.resize(buffer_limit, 0);
        self.input
            .read_exact(&mut self.buffer[buffer_start..buffer_limit])
            .unwrap();
        assert_eq!(
            header.crc32c,
            crc32c::crc32c(&self.buffer[buffer_start..buffer_limit])
        );
        Some(header)
    }

    fn next_header(&mut self) -> Option<BenchHeader> {
        loop {
            let mut header_sz = [0u8; 1];
            match self.input.read_exact(&mut header_sz) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::UnexpectedEof => return None,
                Err(err) => panic!("{err}"),
            }
            let header_sz = header_sz[0] as usize;
            if header_sz == 0 {
                self.true_up();
                continue;
            }
            assert!(header_sz <= LOG_HEADER_MAX_SIZE);
            let mut header = [0u8; LOG_HEADER_MAX_SIZE];
            self.input.read_exact(&mut header[..header_sz]).unwrap();
            let mut up = Unpacker::new(&header[..header_sz]);
            return Some(up.unpack().unwrap());
        }
    }

    fn true_up(&mut self) {
        let offset = self.input.stream_position().unwrap();
        let trued_up = compute_log_true_up(offset);
        self.input.seek(SeekFrom::Start(trued_up)).unwrap();
    }
}

fn pack_log_header_to_vec(header: &BenchHeader) -> Vec<u8> {
    let header_sz: v64 = header.pack_sz().into();
    let header_sz_pa = stack_pack(header_sz);
    let pa = header_sz_pa.pack(header);
    let mut bytes = Vec::with_capacity(pa.pack_sz());
    pa.append_to_vec(&mut bytes);
    bytes
}

fn next_log_boundary(offset: u64) -> u64 {
    ((offset >> LOG_BLOCK_BITS) + 1) << LOG_BLOCK_BITS
}

fn compute_log_true_up(offset: u64) -> u64 {
    if offset == (offset >> LOG_BLOCK_BITS) << LOG_BLOCK_BITS {
        offset
    } else {
        next_log_boundary(offset)
    }
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BenchHeader {
    #[prototk(10, uint64)]
    size: u64,
    #[prototk(11, uint32)]
    discriminant: u32,
    #[prototk(12, fixed32)]
    crc32c: u32,
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BenchBlockMetadata {
    #[prototk(13, uint64)]
    start: u64,
    #[prototk(14, uint64)]
    limit: u64,
    #[prototk(15, fixed32)]
    crc32c: u32,
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BenchFinalBlock {
    #[prototk(16, message)]
    index_block: BenchBlockMetadata,
    #[prototk(17, message)]
    filter_block: BenchBlockMetadata,
    #[prototk(19, bytes32)]
    setsum: [u8; 32],
    #[prototk(20, uint64)]
    smallest_timestamp: u64,
    #[prototk(21, uint64)]
    biggest_timestamp: u64,
    #[prototk(18, fixed64)]
    final_block_offset: u64,
}

#[derive(Clone, Debug, prototk_derive::Message)]
#[allow(clippy::enum_variant_names)]
enum BenchSstEntry<'a> {
    #[prototk(10, bytes)]
    PlainBlock(&'a [u8]),
    #[prototk(13, bytes)]
    FilterBlock(&'a [u8]),
    #[prototk(12, bytes)]
    FinalBlock(&'a [u8]),
}

impl Default for BenchSstEntry<'_> {
    fn default() -> Self {
        Self::PlainBlock(&[])
    }
}

#[derive(Clone, Debug, prototk_derive::Message)]
enum BenchKeyValueEntry<'a> {
    #[prototk(8, message)]
    Put(BenchKeyValuePut<'a>),
    #[prototk(9, message)]
    Del(BenchKeyValueDel<'a>),
}

impl Default for BenchKeyValueEntry<'_> {
    fn default() -> Self {
        Self::Put(BenchKeyValuePut::default())
    }
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BenchKeyValuePut<'a> {
    #[prototk(1, uint64)]
    shared: u64,
    #[prototk(2, bytes)]
    key_frag: &'a [u8],
    #[prototk(3, uint64)]
    timestamp: u64,
    #[prototk(4, bytes)]
    value: &'a [u8],
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct BenchKeyValueDel<'a> {
    #[prototk(5, uint64)]
    shared: u64,
    #[prototk(6, bytes)]
    key_frag: &'a [u8],
    #[prototk(7, uint64)]
    timestamp: u64,
}

struct BenchKeyValueRef<'a> {
    key: &'a [u8],
    timestamp: u64,
    value: Option<&'a [u8]>,
}

benchmark! {
    name = sst_seek_forward_optimized;
    SstScanParameters {
        num_keys in SST_NUM_KEYS,
        target_block_size in SST_TARGET_BLOCK_SIZES,
        restart_interval in SST_RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_sst_seek_forward_optimized
}

benchmark! {
    name = sst_seek_forward_baseline;
    SstScanParameters {
        num_keys in SST_NUM_KEYS,
        target_block_size in SST_TARGET_BLOCK_SIZES,
        restart_interval in SST_RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_sst_seek_forward_baseline
}

benchmark! {
    name = sst_seek_reverse_optimized;
    SstScanParameters {
        num_keys in SST_NUM_KEYS,
        target_block_size in SST_TARGET_BLOCK_SIZES,
        restart_interval in SST_RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_sst_seek_reverse_optimized
}

benchmark! {
    name = sst_seek_reverse_baseline;
    SstScanParameters {
        num_keys in SST_NUM_KEYS,
        target_block_size in SST_TARGET_BLOCK_SIZES,
        restart_interval in SST_RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_sst_seek_reverse_baseline
}

benchmark! {
    name = log_write_optimized;
    LogParameters {
        num_entries in LOG_NUM_ENTRIES,
        batch_entries in LOG_BATCH_ENTRIES,
        value_bytes in VALUE_BYTES,
    }
    bench_log_write_optimized
}

benchmark! {
    name = log_write_baseline;
    LogParameters {
        num_entries in LOG_NUM_ENTRIES,
        batch_entries in LOG_BATCH_ENTRIES,
        value_bytes in VALUE_BYTES,
    }
    bench_log_write_baseline
}

benchmark! {
    name = log_iterate_optimized;
    LogParameters {
        num_entries in LOG_NUM_ENTRIES,
        batch_entries in LOG_BATCH_ENTRIES,
        value_bytes in VALUE_BYTES,
    }
    bench_log_iterate_optimized
}

benchmark! {
    name = log_iterate_baseline;
    LogParameters {
        num_entries in LOG_NUM_ENTRIES,
        batch_entries in LOG_BATCH_ENTRIES,
        value_bytes in VALUE_BYTES,
    }
    bench_log_iterate_baseline
}

statslicer_main! {
    sst_seek_forward_optimized,
    sst_seek_forward_baseline,
    sst_seek_reverse_optimized,
    sst_seek_reverse_baseline,
    log_write_optimized,
    log_write_baseline,
    log_iterate_optimized,
    log_iterate_baseline,
}
