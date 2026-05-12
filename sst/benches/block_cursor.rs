use std::cmp::Ordering;
use std::ops::Bound;

use buffertk::Unpacker;
use statslicer::{Bencher, Parameter, Parameters, benchmark, black_box, statslicer_main};

use sst::block::{Block, BlockBuilder, BlockBuilderOptions};
use sst::bounds_cursor::BoundsCursor;
use sst::pruning_cursor::PruningCursor;
use sst::{Builder, Cursor, KeyRef, SError};

const NUM_KEYS: &[usize] = &[256, 1024, 4096, 16384];
const RESTART_INTERVALS: &[usize] = &[16, 128, 512];
const VALUE_BYTES: &[usize] = &[16, 256];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BlockCursorParameters {
    num_keys: usize,
    restart_interval: usize,
    value_bytes: usize,
}

impl Parameters for BlockCursorParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("num_keys", Parameter::Integer(self.num_keys as u64)),
            (
                "restart_interval",
                Parameter::Integer(self.restart_interval as u64),
            ),
            ("value_bytes", Parameter::Integer(self.value_bytes as u64)),
        ]
    }
}

fn key(index: usize) -> Vec<u8> {
    format!("{index:016x}").into_bytes()
}

fn build_block(params: &BlockCursorParameters) -> Block {
    let options = BlockBuilderOptions::default()
        .bytes_restart_interval(u32::MAX)
        .key_value_pairs_restart_interval(params.restart_interval as u32);
    let mut builder = BlockBuilder::new(options);
    let value = vec![0x42; params.value_bytes];
    for index in 0..params.num_keys {
        builder.put(&key(index), 0, &value).unwrap();
    }
    builder.seal().unwrap()
}

fn bounds(params: &BlockCursorParameters) -> (Bound<Vec<u8>>, Bound<Vec<u8>>) {
    (
        Bound::Included(key(params.num_keys / 4)),
        Bound::Excluded(key(params.num_keys * 3 / 4)),
    )
}

fn consume_forward<C: Cursor>(cursor: &mut C) -> usize {
    let mut visited = 0;
    loop {
        cursor.next().unwrap();
        if let Some(kvr) = cursor.key_value() {
            black_box(kvr.key);
            black_box(kvr.timestamp);
            black_box(kvr.value);
            visited += 1;
        } else {
            break;
        }
    }
    visited
}

fn consume_reverse<C: Cursor>(cursor: &mut C) -> usize {
    cursor.seek_to_last().unwrap();
    let mut visited = 0;
    loop {
        cursor.prev().unwrap();
        if let Some(kvr) = cursor.key_value() {
            black_box(kvr.key);
            black_box(kvr.timestamp);
            black_box(kvr.value);
            visited += 1;
        } else {
            break;
        }
    }
    visited
}

fn bench_forward_optimized(params: &BlockCursorParameters, b: &mut Bencher) {
    let block = build_block(params);
    let (start_bound, end_bound) = bounds(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let mut cursor = block
                .range_scan(&start_bound, &end_bound, u64::MAX)
                .unwrap();
            visited += consume_forward(&mut cursor);
        }
        black_box(visited);
    });
}

fn bench_forward_baseline(params: &BlockCursorParameters, b: &mut Bencher) {
    let block = build_block(params);
    let baseline = BaselineBlock::new(block.as_bytes());
    let (start_bound, end_bound) = bounds(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let pruning = PruningCursor::new(baseline.cursor(), u64::MAX).unwrap();
            let mut cursor = BoundsCursor::new(pruning, &start_bound, &end_bound).unwrap();
            visited += consume_forward(&mut cursor);
        }
        black_box(visited);
    });
}

fn bench_reverse_optimized(params: &BlockCursorParameters, b: &mut Bencher) {
    let block = build_block(params);
    let (start_bound, end_bound) = bounds(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let mut cursor = block
                .range_scan(&start_bound, &end_bound, u64::MAX)
                .unwrap();
            visited += consume_reverse(&mut cursor);
        }
        black_box(visited);
    });
}

fn bench_reverse_baseline(params: &BlockCursorParameters, b: &mut Bencher) {
    let block = build_block(params);
    let baseline = BaselineBlock::new(block.as_bytes());
    let (start_bound, end_bound) = bounds(params);
    let size = b.size();
    b.run(|| {
        let mut visited = 0;
        for _ in 0..size {
            let pruning = PruningCursor::new(baseline.cursor(), u64::MAX).unwrap();
            let mut cursor = BoundsCursor::new(pruning, &start_bound, &end_bound).unwrap();
            visited += consume_reverse(&mut cursor);
        }
        black_box(visited);
    });
}

benchmark! {
    name = block_forward_optimized;
    BlockCursorParameters {
        num_keys in NUM_KEYS,
        restart_interval in RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_forward_optimized
}

benchmark! {
    name = block_forward_baseline;
    BlockCursorParameters {
        num_keys in NUM_KEYS,
        restart_interval in RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_forward_baseline
}

benchmark! {
    name = block_reverse_optimized;
    BlockCursorParameters {
        num_keys in NUM_KEYS,
        restart_interval in RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_reverse_optimized
}

benchmark! {
    name = block_reverse_baseline;
    BlockCursorParameters {
        num_keys in NUM_KEYS,
        restart_interval in RESTART_INTERVALS,
        value_bytes in VALUE_BYTES,
    }
    bench_reverse_baseline
}

#[derive(Clone, Copy)]
struct BaselineBlock<'a> {
    bytes: &'a [u8],
    restarts_boundary: usize,
    restarts_idx: usize,
    num_restarts: usize,
}

impl<'a> BaselineBlock<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let num_restarts = read_u32(bytes, bytes.len() - 4) as usize;
        let footer_body = num_restarts * 4;
        let footer_head = 1 + varint_size(footer_body as u64);
        let capstone = 1 + 4;
        let restarts_idx = bytes.len() - capstone - footer_body;
        let restarts_boundary = restarts_idx - footer_head;
        Self {
            bytes,
            restarts_boundary,
            restarts_idx,
            num_restarts,
        }
    }

    fn cursor(self) -> BaselineBlockCursor<'a> {
        BaselineBlockCursor {
            block: self,
            position: BaselinePosition::First,
        }
    }

    fn restart_point(&self, restart_idx: usize) -> usize {
        read_u32(self.bytes, self.restarts_idx + restart_idx * 4) as usize
    }

    fn restart_for_offset(&self, offset: usize) -> usize {
        let mut left = 0;
        let mut right = self.num_restarts - 1;
        while left < right {
            let mid = (left + right).div_ceil(2);
            let value = self.restart_point(mid);
            match offset.cmp(&value) {
                Ordering::Less => right = mid - 1,
                Ordering::Equal => {
                    left = mid;
                    right = mid;
                }
                Ordering::Greater => left = mid,
            }
        }
        left
    }

    fn entry(&self, offset: usize) -> ParsedEntry<'a> {
        assert!(offset < self.restarts_boundary);
        let mut up = Unpacker::new(&self.bytes[offset..self.restarts_boundary]);
        let entry = up.unpack().unwrap();
        let next_offset = self.restarts_boundary - up.remain().len();
        ParsedEntry { entry, next_offset }
    }
}

struct ParsedEntry<'a> {
    entry: BenchKeyValueEntry<'a>,
    next_offset: usize,
}

#[derive(Clone, Debug)]
enum BaselinePosition {
    First,
    Last,
    Positioned {
        restart_idx: usize,
        offset: usize,
        next_offset: usize,
        key: Vec<u8>,
        timestamp: u64,
    },
}

impl BaselinePosition {
    fn is_positioned(&self) -> bool {
        matches!(self, Self::Positioned { .. })
    }
}

struct BaselineBlockCursor<'a> {
    block: BaselineBlock<'a>,
    position: BaselinePosition,
}

impl<'a> BaselineBlockCursor<'a> {
    fn next_offset(&self) -> usize {
        match &self.position {
            BaselinePosition::First => 0,
            BaselinePosition::Last => self.block.restarts_boundary,
            BaselinePosition::Positioned { next_offset, .. } => *next_offset,
        }
    }

    fn restart_idx(&self) -> usize {
        match &self.position {
            BaselinePosition::First => 0,
            BaselinePosition::Last => self.block.num_restarts,
            BaselinePosition::Positioned { restart_idx, .. } => *restart_idx,
        }
    }

    fn seek_restart(&mut self, restart_idx: usize) -> Option<KeyRef<'_>> {
        let offset = self.block.restart_point(restart_idx);
        let prev_key = match self.position {
            BaselinePosition::First | BaselinePosition::Last => Vec::new(),
            BaselinePosition::Positioned { ref mut key, .. } => std::mem::take(key),
        };
        self.position = self.extract_key(offset, prev_key);
        self.key()
    }

    fn extract_key(&self, offset: usize, mut key: Vec<u8>) -> BaselinePosition {
        if offset >= self.block.restarts_boundary {
            return BaselinePosition::Last;
        }
        let entry = self.block.entry(offset);
        let restart_idx = self.block.restart_for_offset(offset);
        key.truncate(entry.entry.shared());
        key.extend_from_slice(entry.entry.key_frag());
        BaselinePosition::Positioned {
            restart_idx,
            offset,
            next_offset: entry.next_offset,
            key,
            timestamp: entry.entry.timestamp(),
        }
    }
}

impl Cursor for BaselineBlockCursor<'_> {
    fn seek_to_first(&mut self) -> Result<(), SError> {
        self.position = BaselinePosition::First;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), SError> {
        self.position = BaselinePosition::Last;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), SError> {
        let mut left = 0;
        let mut right = self.block.num_restarts - 1;
        while left < right {
            let mid = left + (right - left).div_ceil(2);
            let kvp = self.seek_restart(mid).unwrap();
            match key.cmp(kvp.key) {
                Ordering::Less | Ordering::Equal => right = mid - 1,
                Ordering::Greater => left = mid,
            }
        }
        let mut kref = self.seek_restart(left);
        while let Some(x) = kref {
            if key > x.key {
                self.next()?;
                kref = self.key();
            } else {
                break;
            }
        }
        Ok(())
    }

    fn prev(&mut self) -> Result<(), SError> {
        let target_next_offset = match self.position {
            BaselinePosition::First => return Ok(()),
            BaselinePosition::Last => self.block.restarts_boundary,
            BaselinePosition::Positioned { offset, .. } => offset,
        };
        if target_next_offset == 0 {
            self.position = BaselinePosition::First;
            return Ok(());
        }
        let current_restart_idx = self.restart_idx();
        let restart_idx = if current_restart_idx >= self.block.num_restarts
            || target_next_offset <= self.block.restart_point(current_restart_idx)
        {
            current_restart_idx - 1
        } else {
            current_restart_idx
        };
        self.seek_restart(restart_idx);
        while self.next_offset() < target_next_offset {
            self.next()?;
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), SError> {
        if matches!(self.position, BaselinePosition::First) {
            self.seek_restart(0);
            return Ok(());
        }
        if matches!(self.position, BaselinePosition::Last) {
            return Ok(());
        }
        let offset = self.next_offset();
        if offset >= self.block.restarts_boundary {
            self.position = BaselinePosition::Last;
            return Ok(());
        }
        if self.restart_idx() + 1 < self.block.num_restarts
            && self.block.restart_point(self.restart_idx() + 1) <= offset
        {
            self.seek_restart(self.restart_idx() + 1);
            return Ok(());
        }
        assert!(self.position.is_positioned());
        let prev_key = match self.position {
            BaselinePosition::First | BaselinePosition::Last => Vec::new(),
            BaselinePosition::Positioned { ref mut key, .. } => std::mem::take(key),
        };
        self.position = self.extract_key(offset, prev_key);
        Ok(())
    }

    fn key(&self) -> Option<KeyRef<'_>> {
        match &self.position {
            BaselinePosition::First | BaselinePosition::Last => None,
            BaselinePosition::Positioned { key, timestamp, .. } => Some(KeyRef {
                key,
                timestamp: *timestamp,
            }),
        }
    }

    fn value(&self) -> Option<&'_ [u8]> {
        match self.position {
            BaselinePosition::First | BaselinePosition::Last => None,
            BaselinePosition::Positioned { offset, .. } => {
                let entry = self.block.entry(offset);
                entry.entry.value()
            }
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn varint_size(mut value: u64) -> usize {
    let mut size = 1;
    value >>= 7;
    while value > 0 {
        size += 1;
        value >>= 7;
    }
    size
}

#[derive(Clone, Debug, prototk_derive::Message)]
enum BenchKeyValueEntry<'a> {
    #[prototk(8, message)]
    Put(BenchKeyValuePut<'a>),
    #[prototk(9, message)]
    Del(BenchKeyValueDel<'a>),
}

impl<'a> BenchKeyValueEntry<'a> {
    fn shared(&self) -> usize {
        match self {
            Self::Put(x) => x.shared as usize,
            Self::Del(x) => x.shared as usize,
        }
    }

    fn key_frag(&self) -> &'a [u8] {
        match self {
            Self::Put(x) => x.key_frag,
            Self::Del(x) => x.key_frag,
        }
    }

    fn timestamp(&self) -> u64 {
        match self {
            Self::Put(x) => x.timestamp,
            Self::Del(x) => x.timestamp,
        }
    }

    fn value(&self) -> Option<&'a [u8]> {
        match self {
            Self::Put(x) => Some(x.value),
            Self::Del(_) => None,
        }
    }
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

statslicer_main! {
    block_forward_optimized,
    block_forward_baseline,
    block_reverse_optimized,
    block_reverse_baseline,
}
