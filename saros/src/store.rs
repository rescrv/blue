//! Store Saros series chunks in an [`lsmtk::LsmTree`].
//!
//! The store is deliberately just a set of ordered key-value records.  Saros
//! chunks, metadata, tag postings, and file checkpoints share the same LSM tree,
//! and compaction remains unaware of time-series semantics.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::io::Read;
use std::ops::Bound;
use std::path::{Path, PathBuf};

use biometrics_prometheus::Reader;
use buffertk::{Unpacker, stack_pack};
use handled::SError;
use lsmtk::{LsmTree, LsmtkOptions};
use sha3::{Digest, Sha3_256};
use sst::{Builder, Cursor, KeyRef, SstBuilder, SstOptions};
use tag_index::{Tag, Tags};
use tuple_key2::TupleKey;

use crate::prometheus::{PrometheusLine, SensorType};
use crate::{
    BiometricsStore, ChunkEncoder, FetchCountersRequest, FetchCountersResponse, FetchedSeries,
    MetricType, Point, SError as SarosError, SeriesChunk, Time, Window, arithmetic_error,
    coding_error, internal_error, system_error, text_error, time_error,
};

/// Number of microseconds covered by one physical storage segment.
///
/// Data chunk keys include this segment start so that writes for a two-hour
/// ingest window cluster together in the tree.
pub const SEGMENT_DURATION: Time = Time(2 * 60 * 60 * 1_000_000);

/// Preferred encoded chunk size in bytes.
///
/// A series flushes after its pending encoded form first reaches this size, but
/// a chunk may be smaller when a segment ends or a caller explicitly flushes.
pub const CHUNK_TARGET_BYTES: usize = 4 * 1024;

/// Largest encoded chunk accepted by the store.
pub const CHUNK_MAX_BYTES: usize = 32 * 1024;

/// Canonical tag key used to distinguish physical scrape sources.
pub const SAROS_SOURCE_TAG: &str = "__saros_source__";

const FAMILY_SERIES: u8 = 0;
const SERIES_CHUNK: u8 = 0;
const SERIES_TAGS: u8 = 1;
const FAMILY_TAG_INDEX: u8 = 1;
const FAMILY_CHECKPOINT: u8 = 2;
const TAG_FINGERPRINT_ITEM: u8 = 1;

/// Bitmask of the [`MetricType`]s this build actually stores.
///
/// A checkpoint asserts idempotence for the types it covers, not for the whole file.
/// When gauge support lands, add its bit here and every file checkpointed under a
/// counters-only build becomes eligible for re-ingest instead of being permanently lost.
pub const SUPPORTED_METRIC_TYPES: u64 = 1 << 0;

/// The coverage attributed to a checkpoint written before `supported_types` existed.
///
/// Those builds stored counters and silently discarded everything else, so that is exactly what
/// their checkpoints cover.
const LEGACY_METRIC_TYPES: u64 = 1 << 0;

/// Records that a Prometheus scrape file has been ingested.
#[derive(Clone, Debug, Default, PartialEq, prototk_derive::Message)]
pub struct FileCheckpoint {
    /// Basename of the ingested file, kept for debugging only.
    #[prototk(1, string)]
    pub basename: String,
    /// Wall-clock ingest time for the checkpoint row.
    #[prototk(2, message)]
    pub ingested_at: Time,
    /// Bitmask of [`MetricType`]s that were storable when this checkpoint was written.
    ///
    /// Zero means the checkpoint predates this field; see [`LEGACY_METRIC_TYPES`].
    #[prototk(3, uint64)]
    pub supported_types: u64,
}

/// Tunables for [`SarosStore`].
#[derive(Clone, Debug)]
pub struct SarosStoreOptions {
    /// Process-wide budget for encoded, unflushed chunk bytes across all series.
    ///
    /// The writer used to hold raw samples per series with no global bound, so resident
    /// memory was a function of live series count times samples per chunk.  Crossing this
    /// watermark flushes the largest pending chunks first, memtable-style.  Lowering it bounds
    /// RSS harder at the cost of smaller chunks and more read amplification; that trade is the
    /// reason this is a knob and not a constant.
    pub max_pending_bytes: usize,
    /// How many segments backwards a frontier or predecessor search may walk before giving up.
    ///
    /// The search walks one segment at a time and, unbounded, runs from now to the UNIX
    /// epoch -- roughly 250,000 range scans -- for any series with no earlier data, which is
    /// the common case under series churn.  The bound makes that cost proportional to the
    /// lookback instead of to the age of the epoch.
    ///
    /// The trade is real and one-directional:  a series silent for longer than this is treated
    /// as new, so a sample older than its true frontier will be accepted rather than rejected.
    /// `saros.store.lookback_exhausted` counts every time that risk is taken.  A frontier key
    /// family would remove the walk, and with it this trade.
    pub max_lookback_segments: u64,
}

impl Default for SarosStoreOptions {
    fn default() -> Self {
        Self {
            max_pending_bytes: 64 * 1024 * 1024,
            max_lookback_segments: 168,
        }
    }
}

#[derive(Clone, Debug)]
struct Row {
    key: Vec<u8>,
    timestamp: u64,
    value: Vec<u8>,
}

/// In-memory writer state for one series.
///
/// This used to hold every pending sample as a raw `(Time, Point)` and re-encode the whole
/// chunk on every push, purely to learn a byte count -- Θ(n²) encoder work per chunk and
/// O(samples) resident memory per live series.  It now holds a live [`ChunkEncoder`] plus a
/// one-sample `tail`.
///
/// The `tail` is what makes the duplicate-timestamp overwrite free.  A repeated timestamp
/// always refers to the most recent sample, and the most recent sample has not been handed to
/// the encoder yet, so the overwrite is a field assignment rather than a re-encode.  Nothing
/// else needs the raw samples, so nothing else keeps them.
#[derive(Clone, Debug)]
struct SeriesState {
    metric_type: MetricType,
    fingerprint: [u8; 16],
    tags: Tags<'static>,
    metadata_emitted: bool,
    encoder: ChunkEncoder,
    tail: Option<(Time, Point)>,
    last_ts: Option<Time>,
}

impl SeriesState {
    fn with_last_ts(
        metric_type: MetricType,
        fingerprint: [u8; 16],
        tags: Tags<'static>,
        last_ts: Option<Time>,
    ) -> Self {
        Self {
            metric_type,
            fingerprint,
            tags,
            metadata_emitted: false,
            encoder: ChunkEncoder::new(metric_type),
            tail: None,
            last_ts,
        }
    }

    /// True when this series holds samples that have not been written to a chunk row.
    fn has_pending(&self) -> bool {
        self.tail.is_some() || !self.encoder.is_empty()
    }

    /// Encoded bytes currently held for this series, for the process-wide budget.
    ///
    /// Charges the upper bound:  a budget that under-counts does not bound anything.
    fn pending_bytes(&self) -> usize {
        self.encoder.encoded_len_upper_bound()
    }

    /// Whether the chunk under construction has reached its target size.
    ///
    /// Compares against the compressed streams alone, which is a lower bound on the encoded
    /// chunk, since message framing and the header are strictly additive.  So a chunk flushed on
    /// this condition is at least `CHUNK_TARGET_BYTES`, which is the invariant
    /// `series_state_flushes_around_target_size_and_below_hard_max` checks.  Deciding on the
    /// upper bound instead would flush marginally *under* target and break it.
    fn pending_over_target(&self) -> bool {
        self.encoder.stream_bytes() >= CHUNK_TARGET_BYTES
    }

    fn push(&mut self, time: Time, point: Point, rows: &mut Vec<Row>) -> Result<(), SError> {
        if let Some(last_ts) = self.last_ts {
            if time < last_ts {
                return Err(time_error(format!(
                    "sample timestamp went backwards from {} to {}",
                    last_ts.to_rfc3339(),
                    time.to_rfc3339()
                )));
            }
            if !self.has_pending() && time == last_ts {
                return Err(time_error(
                    "duplicate timestamp arrived after its chunk was flushed",
                ));
            }
        }
        if self.tail.is_some_and(|(tail_time, _)| tail_time == time) {
            self.tail = Some((time, point));
            self.last_ts = Some(time);
            return Ok(());
        }
        // Order matters.  The previous tail can no longer be overwritten -- a repeated timestamp
        // would have been caught above -- so fold it in *before* deciding whether to close the
        // chunk.  Deciding first would leave it out of the size and segment tests and shift every
        // chunk boundary by one sample relative to the pending-vector version.
        if let Some((tail_time, tail_point)) = self.tail.take() {
            self.encoder.push(tail_time, tail_point)?;
        }
        if let Some(first) = self.encoder.first_sample_ts()
            && (segment_start(first) != segment_start(time) || self.pending_over_target())
        {
            self.flush_pending(rows)?;
        }
        self.tail = Some((time, point));
        self.last_ts = Some(time);
        self.check_pending_size()
    }

    fn flush_pending(&mut self, rows: &mut Vec<Row>) -> Result<(), SError> {
        if !self.has_pending() {
            return Ok(());
        }
        let timestamp = ingest_timestamp()?;
        if !self.metadata_emitted {
            rows.push(Row {
                key: series_tags_key(self.metric_type, self.fingerprint),
                timestamp,
                value: self.tags.to_string().into_bytes(),
            });
            for tag in self.tags.tags() {
                rows.push(Row {
                    key: tag_index_key(self.metric_type, tag.key(), tag.value(), self.fingerprint),
                    timestamp,
                    value: Vec::new(),
                });
            }
            self.metadata_emitted = true;
        }
        if let Some((tail_time, tail_point)) = self.tail.take() {
            self.encoder.push(tail_time, tail_point)?;
        }
        let encoder = std::mem::replace(&mut self.encoder, ChunkEncoder::new(self.metric_type));
        let chunk = encoder.seal()?;
        let value = chunk.encode();
        if value.len() > CHUNK_MAX_BYTES {
            // Unreachable while `check_pending_size` runs on every push, because
            // `ChunkEncoder::encoded_len` is an upper bound on this length.  Kept as the
            // authoritative check so the invariant does not depend on that reasoning.
            return Err(coding_error(format!(
                "series chunk exceeded max bytes: {} > {}",
                value.len(),
                CHUNK_MAX_BYTES
            )));
        }
        rows.push(Row {
            key: series_chunk_key(
                self.metric_type,
                segment_start(chunk.first_sample_ts),
                self.fingerprint,
                chunk.last_sample_ts,
            ),
            timestamp,
            value,
        });
        Ok(())
    }

    fn check_pending_size(&self) -> Result<(), SError> {
        let size = self.encoder.encoded_len_upper_bound();
        if size > CHUNK_MAX_BYTES {
            return Err(coding_error(format!(
                "series chunk exceeded max bytes: {size} > {CHUNK_MAX_BYTES}"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct BatchSeries {
    tags: Tags<'static>,
    last_ts: Option<Time>,
    duplicate_at_last_allowed: bool,
    samples: Vec<(Time, Point)>,
}

impl BatchSeries {
    fn new(tags: Tags<'static>, last_ts: Option<Time>, duplicate_at_last_allowed: bool) -> Self {
        Self {
            tags,
            last_ts,
            duplicate_at_last_allowed,
            samples: Vec::new(),
        }
    }

    fn push(&mut self, time: Time, point: Point) -> Result<(), SError> {
        if self.samples.is_empty()
            && let Some(last_ts) = self.last_ts
        {
            if time < last_ts {
                return Err(time_error(format!(
                    "sample timestamp went backwards from {} to {}",
                    last_ts.to_rfc3339(),
                    time.to_rfc3339()
                )));
            }
            if time == last_ts && !self.duplicate_at_last_allowed {
                return Err(time_error(
                    "duplicate timestamp arrived after its chunk was flushed",
                ));
            }
        }
        if let Some((last_ts, last_point)) = self.samples.last_mut() {
            if time < *last_ts {
                return Err(time_error(format!(
                    "sample timestamp went backwards from {} to {}",
                    last_ts.to_rfc3339(),
                    time.to_rfc3339()
                )));
            }
            if time == *last_ts {
                *last_point = point;
                return Ok(());
            }
        }
        self.samples.push((time, point));
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct SeriesFrontier {
    tags: Option<Tags<'static>>,
    last_ts: Option<Time>,
}

/// An SST-backed Saros store.
///
/// The store accepts Prometheus-compatible scrape files, writes chunk and index
/// records to temporary SSTs, and ingests those SSTs into an LSM tree.
pub struct SarosStore {
    root: PathBuf,
    tree: LsmTree,
    options: SarosStoreOptions,
    rows: Vec<Row>,
    pending_checkpoints: BTreeSet<[u8; 32]>,
    series: BTreeMap<(MetricType, [u8; 16]), SeriesState>,
    pending_bytes: usize,
    flush_counter: u64,
}

impl SarosStore {
    /// Open an existing store or create a new one at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying LSM tree cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SError> {
        Self::open_with_options(path, SarosStoreOptions::default())
    }

    /// Open an existing store or create a new one at `path` with explicit tunables.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying LSM tree cannot be opened.
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: SarosStoreOptions,
    ) -> Result<Self, SError> {
        let root = path.as_ref().to_path_buf();
        let lsmtk_options = LsmtkOptions::default().with_path(root.to_string_lossy().to_string());
        let tree = LsmTree::open(lsmtk_options)?;
        Ok(Self {
            root,
            tree,
            options,
            rows: Vec::new(),
            pending_checkpoints: BTreeSet::new(),
            series: BTreeMap::new(),
            pending_bytes: 0,
            flush_counter: 0,
        })
    }

    /// The tunables this store was opened with.
    pub fn options(&self) -> &SarosStoreOptions {
        &self.options
    }

    /// Ingest a Prometheus scrape file opened through [`biometrics_prometheus`].
    ///
    /// The file basename is stored in the checkpoint value for debugging, while
    /// the file content hash determines idempotence.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, the Prometheus text is
    /// malformed, or any supported sample violates Saros ordering rules.
    pub fn ingest_prometheus_reader(
        &mut self,
        reader: Reader,
        source_id: &str,
    ) -> Result<bool, SError> {
        let basename = reader
            .path()
            .file_name()
            .unwrap_or(reader.path().as_str())
            .to_string();
        let mut contents = Vec::new();
        let mut file = &*reader;
        file.read_to_end(&mut contents)
            .map_err(|err| system_error(err.to_string()))?;
        self.ingest_prometheus_bytes(&basename, &contents, source_id)
    }

    /// Ingest one Prometheus scrape file from disk.
    ///
    /// Returns `Ok(true)` when this content hash is accepted for the first time
    /// and `Ok(false)` when the content hash already has a checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or if
    /// [`SarosStore::ingest_prometheus_bytes`] rejects the file contents.
    pub fn ingest_prometheus_file(
        &mut self,
        path: impl AsRef<utf8path::Path<'static>>,
        source_id: &str,
    ) -> Result<bool, SError> {
        let reader = Reader::open(path.as_ref().clone())?;
        self.ingest_prometheus_reader(reader, source_id)
    }

    /// Ingest Prometheus scrape text under a debugging basename.
    ///
    /// This method validates the whole file before mutating in-memory series
    /// state.  A file that fails validation is not checkpointed and does not
    /// leave partial samples behind.
    ///
    /// # Errors
    ///
    /// Returns an error for non-UTF-8 input, unsupported or malformed
    /// Prometheus rows, missing timestamps, reserved Saros labels, or samples
    /// that are not ordered after the current per-series frontier.
    pub fn ingest_prometheus_bytes(
        &mut self,
        basename: &str,
        contents: &[u8],
        source_id: &str,
    ) -> Result<bool, SError> {
        let content_hash = content_hash(contents);
        if self.pending_checkpoints.contains(&content_hash)
            || self.checkpoint_covers_supported(content_hash)?
        {
            return Ok(false);
        }
        let text = std::str::from_utf8(contents).map_err(|_| text_error("input is not UTF-8"))?;
        let prometheus_lines = crate::support_nom::parse_all(crate::prometheus::parse)(text)
            .map_err(|err| text_error(err.to_string()))?;
        self.ingest_prometheus_lines(&prometheus_lines, source_id)?;
        self.emit_checkpoint(content_hash, basename)?;
        Ok(true)
    }

    /// Flush pending samples, metadata, index postings, and checkpoints.
    ///
    /// Rows are sorted into SST order and ingested into the backing LSM tree.
    ///
    /// # Errors
    ///
    /// Returns an error if chunk encoding, temporary SST construction, LSM
    /// ingestion, or temporary-file cleanup fails.
    pub fn flush(&mut self) -> Result<(), SError> {
        for state in self.series.values_mut() {
            state.flush_pending(&mut self.rows)?;
        }
        self.pending_bytes = 0;
        if self.rows.is_empty() {
            return Ok(());
        }
        // This used to clone every row before sorting, doubling peak RSS at exactly the
        // moment it was already highest, and the clone was never read after the subsequent
        // `clear()`.  Take instead.  A failed build puts the rows back, so a caller that retries
        // `flush` does not silently lose the batch -- the old code's `clear()` ran only on
        // success, and that behaviour is preserved deliberately rather than by accident.
        let mut rows = std::mem::take(&mut self.rows);
        rows.sort_by(|lhs, rhs| {
            KeyRef::new(&lhs.key, lhs.timestamp).cmp(&KeyRef::new(&rhs.key, rhs.timestamp))
        });
        rows.dedup_by(|lhs, rhs| lhs.key == rhs.key && lhs.timestamp == rhs.timestamp);
        match self.build_and_ingest(&rows) {
            Ok(()) => {
                self.pending_checkpoints.clear();
                Ok(())
            }
            Err(err) => {
                self.rows = rows;
                Err(err)
            }
        }
    }

    fn build_and_ingest(&mut self, rows: &[Row]) -> Result<(), SError> {
        let path = self.next_flush_path();
        if path.exists() {
            std::fs::remove_file(&path).map_err(|err| system_error(err.to_string()))?;
        }
        let mut builder = SstBuilder::new(SstOptions::default(), &path)?;
        for row in rows.iter() {
            builder.put(&row.key, row.timestamp, &row.value)?;
        }
        builder.seal()?;
        self.tree.ingest(&path)?;
        std::fs::remove_file(&path).map_err(|err| system_error(err.to_string()))?;
        Ok(())
    }

    fn ingest_prometheus_lines(
        &mut self,
        lines: &[PrometheusLine],
        source_id: &str,
    ) -> Result<(), SError> {
        let batch = self.collect_prometheus_samples(lines, source_id)?;
        for ((metric_type, fingerprint), batch_series) in batch {
            let state = self
                .series
                .entry((metric_type, fingerprint))
                .or_insert_with(|| {
                    SeriesState::with_last_ts(
                        metric_type,
                        fingerprint,
                        batch_series.tags.clone(),
                        batch_series.last_ts,
                    )
                });
            if state.tags != batch_series.tags {
                return Err(internal_error("series fingerprint collision"));
            }
            let before = state.pending_bytes();
            for (time, point) in batch_series.samples {
                state.push(time, point, &mut self.rows)?;
            }
            let after = state.pending_bytes();
            // Intermediate flushes inside `push` already emitted their rows, so the difference
            // between the two snapshots is the net change to what is still resident.
            self.pending_bytes = self.pending_bytes.saturating_add(after).saturating_sub(before);
        }
        self.enforce_pending_budget()
    }

    /// Flush the largest pending chunks until the process-wide budget is satisfied.
    ///
    /// The writer had no global bound at all, so resident memory was live-series count
    /// times samples-per-chunk.  Largest-first is memtable discipline:  it buys the most
    /// headroom per chunk written and leaves small, slow series accumulating toward their
    /// target instead of being flushed into tiny chunks.
    fn enforce_pending_budget(&mut self) -> Result<(), SError> {
        if self.pending_bytes <= self.options.max_pending_bytes {
            return Ok(());
        }
        let target = self.options.max_pending_bytes / 2;
        let mut order: Vec<((MetricType, [u8; 16]), usize)> = self
            .series
            .iter()
            .filter(|(_, state)| state.has_pending())
            .map(|(key, state)| (*key, state.pending_bytes()))
            .collect();
        order.sort_by(|lhs, rhs| rhs.1.cmp(&lhs.1));
        for (key, bytes) in order {
            if self.pending_bytes <= target {
                break;
            }
            if let Some(state) = self.series.get_mut(&key) {
                state.flush_pending(&mut self.rows)?;
                crate::PENDING_BUDGET_FLUSH.click();
            }
            self.pending_bytes = self.pending_bytes.saturating_sub(bytes);
        }
        Ok(())
    }

    fn collect_prometheus_samples(
        &self,
        lines: &[PrometheusLine],
        source_id: &str,
    ) -> Result<BTreeMap<(MetricType, [u8; 16]), BatchSeries>, SError> {
        let mut declarations = BTreeMap::new();
        for line in lines {
            if let PrometheusLine::TypeDeclaration(decl) = line {
                let previous = declarations.insert(decl.label().to_string(), decl.sensor_type());
                if previous.is_some_and(|previous| previous != decl.sensor_type()) {
                    return Err(text_error(format!(
                        "conflicting TYPE declarations for {}",
                        decl.label()
                    )));
                }
            }
        }
        let mut batch = BTreeMap::new();
        for line in lines {
            if let PrometheusLine::MetricReading(reading) = line {
                let Some(sensor_type) = reading_sensor_type(&declarations, &reading.metric_name)?
                else {
                    continue;
                };
                if sensor_type != SensorType::Counter {
                    // These readings are parsed and then discarded.  The checkpoint
                    // records that fact via `supported_types`, so the file becomes eligible for
                    // re-ingest once the type is supported, and this counter makes the discard
                    // visible in the meantime.
                    crate::SKIPPED_UNSUPPORTED.click();
                    continue;
                }
                let tags = canonical_tags(&reading.metric_name, &reading.labels, source_id)?;
                let fingerprint = series_fingerprint(&tags);
                let time = prometheus_timestamp(reading.timestamp)?;
                let point = Point(reading.reading);
                let metric_type = MetricType::Counter;
                let key = (metric_type, fingerprint);
                let entry = match batch.entry(key) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => {
                        let (last_ts, duplicate_at_last_allowed) = if let Some(state) =
                            self.series.get(&key)
                        {
                            if state.tags != tags {
                                return Err(internal_error("series fingerprint collision"));
                            }
                            (state.last_ts, state.has_pending())
                        } else {
                            let frontier = self.load_series_frontier(metric_type, fingerprint)?;
                            if let Some(stored_tags) = frontier.tags
                                && stored_tags != tags
                            {
                                return Err(internal_error("series fingerprint collision"));
                            }
                            (frontier.last_ts, false)
                        };
                        entry.insert(BatchSeries::new(
                            tags.clone(),
                            last_ts,
                            duplicate_at_last_allowed,
                        ))
                    }
                };
                if entry.tags != tags {
                    return Err(internal_error("series fingerprint collision"));
                }
                entry.push(time, point)?;
            }
        }
        Ok(batch)
    }

    fn load_series_frontier(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
    ) -> Result<SeriesFrontier, SError> {
        let tags = self.load_tags(metric_type, fingerprint)?;
        let last_ts = self.latest_series_timestamp(metric_type, fingerprint)?;
        Ok(SeriesFrontier { tags, last_ts })
    }

    /// The last stored sample timestamp for a series, or `None` within the configured lookback.
    ///
    /// This used to range over the whole chunk family for the metric type, `seek_to_last`
    /// and then `prev()` until the fingerprint matched -- O(total chunks) per cold series, so
    /// O(C·N) to warm N of them.  Chunk keys sort `(segment, fingerprint, last_ts)`, which means
    /// a series' chunks are contiguous *within* a segment; `predecessor_chunk` already exploits
    /// that and this now defers to it.
    ///
    /// The search starts one segment into the future so a chunk written with clock skew, or a
    /// backfill dated slightly ahead, is still seen.  A chunk dated further ahead than that is
    /// not, which is the same class of residual risk as the lookback bound:  both are removed by
    /// a frontier key family, and neither is silent -- see `saros.store.lookback_exhausted`.
    fn latest_series_timestamp(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
    ) -> Result<Option<Time>, SError> {
        let now = Time::now().ok_or_else(|| time_error("could not get current time"))?;
        let from = now + SEGMENT_DURATION;
        Ok(self
            .predecessor_chunk(metric_type, fingerprint, from)?
            .map(|(_, last_ts, _)| last_ts))
    }

    /// Whether a checkpoint already covers every metric type this build can store.
    ///
    /// The old predicate was "a checkpoint row exists", which made a file that was
    /// ingested under a counters-only build permanently un-reingestable -- its gauges were
    /// dropped and its content hash was checkpointed anyway.  Coverage is now recorded, so
    /// adding a bit to [`SUPPORTED_METRIC_TYPES`] re-opens exactly the files that need it while
    /// leaving content-hash idempotence intact per (hash, type set).
    fn checkpoint_covers_supported(&self, content_hash: [u8; 32]) -> Result<bool, SError> {
        let mut is_tombstone = false;
        let Some(value) = self
            .tree
            .load(&checkpoint_key(content_hash), &mut is_tombstone)?
        else {
            return Ok(false);
        };
        if is_tombstone {
            return Ok(false);
        }
        let mut unpacker = Unpacker::new(&value);
        let checkpoint: FileCheckpoint = unpacker
            .unpack()
            .map_err(|err: prototk::SError| coding_error(err.to_string()))?;
        let covered = if checkpoint.supported_types == 0 {
            LEGACY_METRIC_TYPES
        } else {
            checkpoint.supported_types
        };
        Ok(covered & SUPPORTED_METRIC_TYPES == SUPPORTED_METRIC_TYPES)
    }

    fn emit_checkpoint(&mut self, content_hash: [u8; 32], basename: &str) -> Result<(), SError> {
        let ingested_at = Time::now().ok_or_else(|| time_error("could not get current time"))?;
        let checkpoint = FileCheckpoint {
            basename: basename.to_string(),
            ingested_at,
            supported_types: SUPPORTED_METRIC_TYPES,
        };
        self.rows.push(Row {
            key: checkpoint_key(content_hash),
            timestamp: ingest_timestamp()?,
            value: stack_pack(&checkpoint).to_vec(),
        });
        self.pending_checkpoints.insert(content_hash);
        Ok(())
    }

    fn next_flush_path(&mut self) -> PathBuf {
        let pid = std::process::id();
        let counter = self.flush_counter;
        self.flush_counter += 1;
        self.root
            .join(format!("saros-flush-{pid}-{counter}.sst.tmp"))
    }

    /// Fingerprints matching every tag, in ascending order.
    ///
    /// This used to materialize each posting list into a `Vec<[u8; 16]>` -- it had to,
    /// because it sorted the lists by `Vec::len` to pick a join order -- and then build a
    /// `BTreeSet` per additional matcher.  For `__name__=x` over 10⁶ series that is ~16 MB of
    /// fingerprints plus ~10⁶ tree nodes allocated before a single sample is read, with no early
    /// exit.
    ///
    /// The key layout `(1, metric_type, tag_key, tag_value, fingerprint)` already makes each
    /// matcher a sorted run in the tree, so a leapfrog join works directly:  one cursor per
    /// matcher, repeatedly seek every cursor to the current maximum fingerprint, emit on
    /// unanimous agreement.  Resident state is O(matchers); cost is bounded by the smallest list
    /// with skip-out on the rest.  The sort and dedup in `posting_list` are gone with it --
    /// `range_scan` already yields merged keys in sorted order.
    ///
    /// Cursor order is still arbitrary, so the skip-out is not as sharp as it could be.  Ordering
    /// by estimated cardinality without materializing is future work; until then an arbitrary
    /// order is correct, just slower.
    fn matching_fingerprints(
        &self,
        metric_type: MetricType,
        tags: &Tags<'_>,
    ) -> Result<Vec<[u8; 16]>, SError> {
        let matchers: Vec<(String, String)> = tags
            .tags()
            .map(|tag| (tag.key().to_string(), tag.value().to_string()))
            .collect();
        if matchers.is_empty() {
            // This used to return an empty result, reporting "no data" for a query that
            // actually asked for everything.  `Tags::parse` rejects the empty set, so this is
            // reachable only through a direct construction, but defence in depth costs one line
            // and at 10⁶ series a whole-store scan is not something to serve by accident.
            return Err(text_error("query must constrain at least one tag"));
        }
        let mut cursors = Vec::with_capacity(matchers.len());
        for (key, value) in matchers.iter() {
            let start = tag_index_key(metric_type, key, value, [0u8; 16]);
            let end = tag_index_key(metric_type, key, value, [0xffu8; 16]);
            let start_bound = Bound::Included(start);
            let end_bound = Bound::Included(end);
            let mut cursor = self.tree.range_scan(&start_bound, &end_bound)?;
            cursor.next()?;
            cursors.push(cursor);
        }
        let arity = cursors.len();
        let mut matches = Vec::new();
        let Some(mut candidate) = Self::cursor_fingerprint(&cursors[0])? else {
            return Ok(matches);
        };
        // `agreed` counts how many cursors are known to sit on `candidate`, starting with the
        // one it was read from.  `idx` is the cursor advanced most recently.
        let mut agreed = 1usize;
        let mut idx = 0usize;
        loop {
            if agreed == arity {
                matches.push(candidate);
                idx = (idx + 1) % arity;
                cursors[idx].next()?;
                let Some(next) = Self::cursor_fingerprint(&cursors[idx])? else {
                    break;
                };
                candidate = next;
                agreed = 1;
                continue;
            }
            idx = (idx + 1) % arity;
            let (key, value) = &matchers[idx];
            let target = tag_index_key(metric_type, key, value, candidate);
            cursors[idx].seek(&target)?;
            let Some(found) = Self::cursor_fingerprint(&cursors[idx])? else {
                break;
            };
            if found == candidate {
                agreed += 1;
            } else {
                // `seek` lands at or after the target, so this strictly advances the candidate,
                // which is what makes the loop terminate.
                candidate = found;
                agreed = 1;
            }
        }
        // A key present at more than one timestamp can surface twice; the old `posting_list`
        // deduped for the same reason.  `matches` is ascending, so this is linear.
        matches.dedup();
        Ok(matches)
    }

    fn cursor_fingerprint(cursor: &impl Cursor) -> Result<Option<[u8; 16]>, SError> {
        let Some(kvr) = cursor.key_value() else {
            return Ok(None);
        };
        let (_, _, _, fingerprint) = parse_tag_index_key(kvr.key)?;
        Ok(Some(fingerprint))
    }

    fn load_tags(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
    ) -> Result<Option<Tags<'static>>, SError> {
        let mut is_tombstone = false;
        let Some(value) = self.tree.load(
            &series_tags_key(metric_type, fingerprint),
            &mut is_tombstone,
        )?
        else {
            return Ok(None);
        };
        if is_tombstone {
            return Ok(None);
        }
        let tags =
            String::from_utf8(value).map_err(|_| coding_error("stored tags are not UTF-8"))?;
        Tags::new(tags)
            .map(Tags::into_owned)
            .ok_or_else(|| coding_error("stored tags did not parse"))
            .map(Some)
    }

    /// Load canonical tags for an ascending set of fingerprints in one forward pass.
    ///
    /// `fetch_counters` did one `load_tags` point lookup per matched series.  The
    /// fingerprints come out of the leapfrog join already ascending and the metadata family
    /// sorts `(0, 1, metric_type, fingerprint)`, so a single cursor seeking forward through them
    /// turns k random lookups into one near-sequential scan.
    ///
    /// The chunk family is the other half of the same problem and is *not* done here.  Inverting
    /// `load_chunks` into a per-segment merge join is the larger win on paper, but its payoff
    /// depends on the relative cost of `Cursor::seek` against opening a fresh `range_scan` in
    /// lsmtk, which is a number to measure rather than assume.  Measure it before rewriting the
    /// read path's inner loop.
    fn load_tags_batch(
        &self,
        metric_type: MetricType,
        fingerprints: &[[u8; 16]],
    ) -> Result<BTreeMap<[u8; 16], Tags<'static>>, SError> {
        let mut found = BTreeMap::new();
        let Some(first) = fingerprints.first() else {
            return Ok(found);
        };
        let last = fingerprints.last().expect("non-empty checked above");
        let start_bound = Bound::Included(series_tags_key(metric_type, *first));
        let end_bound = Bound::Included(series_tags_key(metric_type, *last));
        let mut cursor = self.tree.range_scan(&start_bound, &end_bound)?;
        for fingerprint in fingerprints.iter() {
            cursor.seek(&series_tags_key(metric_type, *fingerprint))?;
            let Some(kvr) = cursor.key_value() else {
                break;
            };
            if kvr.key != series_tags_key(metric_type, *fingerprint).as_slice() {
                continue;
            }
            let Some(value) = kvr.value else {
                continue;
            };
            let tags = std::str::from_utf8(value)
                .map_err(|_| coding_error("stored tags are not UTF-8"))?
                .to_string();
            let tags = Tags::new(tags)
                .map(Tags::into_owned)
                .ok_or_else(|| coding_error("stored tags did not parse"))?;
            found.insert(*fingerprint, tags);
        }
        Ok(found)
    }

    fn load_chunks(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
        window: Window,
    ) -> Result<Vec<SeriesChunk>, SError> {
        let mut chunks = BTreeMap::new();
        if let Some((segment, last_ts, chunk)) =
            self.predecessor_chunk(metric_type, fingerprint, window.start)?
        {
            chunks.insert((segment, last_ts), chunk);
        }
        let mut segment = segment_start(window.start);
        while segment < window.limit {
            let segment_limit = segment + SEGMENT_DURATION;
            let start_ts = if segment == segment_start(window.start) {
                window.start
            } else {
                segment
            };
            let start = series_chunk_key(metric_type, segment, fingerprint, start_ts);
            let end = series_chunk_key(metric_type, segment, fingerprint, segment_limit);
            let start_bound = Bound::Included(start);
            let end_bound = Bound::Excluded(end);
            let mut cursor = self.tree.range_scan(&start_bound, &end_bound)?;
            cursor.next()?;
            while let Some(kvr) = cursor.key_value() {
                let (_, key_segment, key_fingerprint, last_ts) = parse_series_chunk_key(kvr.key)?;
                if key_fingerprint != fingerprint {
                    return Err(internal_error("range scan escaped series fingerprint"));
                }
                let Some(value) = kvr.value else {
                    cursor.next()?;
                    continue;
                };
                let chunk = SeriesChunk::decode(value)?;
                if chunk.first_sample_ts >= window.limit {
                    break;
                }
                chunks.insert((key_segment, last_ts), chunk);
                cursor.next()?;
            }
            segment = segment_limit;
        }
        Ok(chunks.into_values().collect())
    }

    /// The most recent chunk for a series at or before `time`, within the configured lookback.
    ///
    /// This walk is one range scan per segment and, unbounded, runs from `time` to the
    /// UNIX epoch -- on the order of 250,000 scans in 2026 -- for any series with no earlier
    /// data.  Under series churn that is the common case, not the rare one, which is why
    /// `latest_series_timestamp` could not simply route the frontier probe through here until
    /// this walk was bounded.
    ///
    /// `SarosStoreOptions::max_lookback_segments` bounds it.  Exhausting the bound is counted,
    /// not swallowed:  on the query path it costs a rate extrapolation, and on the ingest path
    /// it means monotonicity is no longer enforced against the true frontier for that series.
    fn predecessor_chunk(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
        time: Time,
    ) -> Result<Option<(Time, Time, SeriesChunk)>, SError> {
        let mut segment = segment_start(time);
        let mut walked = 0u64;
        loop {
            let start = series_chunk_key(metric_type, segment, fingerprint, segment);
            let end = if segment == segment_start(time) {
                Bound::Included(series_chunk_key(metric_type, segment, fingerprint, time))
            } else {
                Bound::Excluded(series_chunk_key(
                    metric_type,
                    segment,
                    fingerprint,
                    segment + SEGMENT_DURATION,
                ))
            };
            let start_bound = Bound::Included(start);
            let mut cursor = self.tree.range_scan(&start_bound, &end)?;
            cursor.seek_to_last()?;
            cursor.prev()?;
            if let Some(kvr) = cursor.key_value() {
                let (_, key_segment, key_fingerprint, last_ts) = parse_series_chunk_key(kvr.key)?;
                if key_fingerprint != fingerprint {
                    return Err(internal_error("predecessor escaped series fingerprint"));
                }
                if let Some(value) = kvr.value {
                    return Ok(Some((key_segment, last_ts, SeriesChunk::decode(value)?)));
                }
            }
            if segment.0 <= 0 {
                return Ok(None);
            }
            walked += 1;
            if walked >= self.options.max_lookback_segments {
                crate::LOOKBACK_EXHAUSTED.click();
                return Ok(None);
            }
            segment = segment - SEGMENT_DURATION;
        }
    }
}

impl BiometricsStore for SarosStore {
    fn fetch_counters(
        &self,
        _: &rpc_pb::Context,
        req: FetchCountersRequest,
    ) -> Result<FetchCountersResponse, SarosError> {
        let req_tags =
            Tags::new(req.tags).ok_or_else(|| text_error("counter request tags did not parse"))?;
        let window = req.params.window_including_lookback();
        let fingerprints = self.matching_fingerprints(MetricType::Counter, &req_tags)?;
        let tags_by_fingerprint = self.load_tags_batch(MetricType::Counter, &fingerprints)?;
        let mut serieses = Vec::new();
        for fingerprint in fingerprints {
            let Some(tags) = tags_by_fingerprint.get(&fingerprint) else {
                continue;
            };
            let chunks = self.load_chunks(MetricType::Counter, fingerprint, window)?;
            if !chunks.is_empty() {
                serieses.push(FetchedSeries {
                    tags: tags.to_string(),
                    chunks,
                });
            }
        }
        Ok(FetchCountersResponse { serieses })
    }

    fn fetch_gauges(
        &self,
        _: &rpc_pb::Context,
        _: crate::FetchGaugesRequest,
    ) -> Result<crate::FetchGaugesResponse, SError> {
        Ok(crate::FetchGaugesResponse::default())
    }

    fn fetch_histograms(
        &self,
        _: &rpc_pb::Context,
        _: crate::FetchHistogramsRequest,
    ) -> Result<crate::FetchHistogramsResponse, SError> {
        Ok(crate::FetchHistogramsResponse::default())
    }
}

pub fn segment_start(time: Time) -> Time {
    Time(time.0.div_euclid(SEGMENT_DURATION.0) * SEGMENT_DURATION.0)
}

/// Construct the canonical tags for one Prometheus reading.
///
/// The result always includes `__name__` and [`SAROS_SOURCE_TAG`].  Input labels
/// in the reserved `__saros_*__` namespace are rejected.
pub fn canonical_tags(
    metric_name: &str,
    labels: &std::collections::HashMap<String, String>,
    source_id: &str,
) -> Result<Tags<'static>, SError> {
    let mut pairs = Vec::with_capacity(labels.len() + 2);
    pairs.push(("__name__".to_string(), metric_name.to_string()));
    pairs.push((SAROS_SOURCE_TAG.to_string(), source_id.to_string()));
    for (key, value) in labels {
        if is_reserved_saros_tag(key) {
            return Err(text_error(format!("reserved Saros label: {key}")));
        }
        pairs.push((key.clone(), value.clone()));
    }
    pairs.sort();
    let mut tags = Vec::with_capacity(pairs.len());
    for (key, value) in pairs.iter() {
        tags.push(
            Tag::new(key, value)
                .ok_or_else(|| text_error("tag did not parse"))?
                .into_owned(),
        );
    }
    // `Tags::from` was infallible and skipped validation, so a `:` in any key or value
    // produced a `Tags` that would not parse -- and `series_fingerprint` calls `tags.tags()`,
    // which unwraps that parse.  A Prometheus recording-rule name panicked the ingest process.
    // `try_from` validates, converting the panic into the rejection path
    // `ingest_prometheus_bytes` already documents.
    //
    // This is the safety fix and not the feature fix.  Rejecting every recording-rule metric is
    // not acceptable behaviour; the repair is to stop using an unescaped delimiter-joined string
    // as the canonical encoding.  `series_fingerprint` already builds a self-delimiting
    // `tuple_key2` per tag, so the encoder exists.  That is a format change, not a localized fix.
    Tags::try_from(tags).map_err(|_| text_error("canonical tags did not round-trip"))
}

/// Compute the folded setsum fingerprint for canonical tags.
///
/// Each tag contributes a self-delimiting `tuple_key2` item
/// `(1, tag_key, tag_value)` to the set before the 32-byte digest is folded to
/// 16 bytes by XORing the upper and lower halves.
pub fn series_fingerprint(tags: &Tags<'_>) -> [u8; 16] {
    let mut setsum = setsum::Setsum::default();
    for tag in tags.tags() {
        let item = TupleKey::builder()
            .u8(TAG_FINGERPRINT_ITEM)
            .string(tag.key())
            .string(tag.value())
            .build();
        setsum.insert(item.as_bytes());
    }
    let digest = setsum.digest();
    let mut fingerprint = [0u8; 16];
    for idx in 0..16 {
        fingerprint[idx] = digest[idx] ^ digest[idx + 16];
    }
    fingerprint
}

// NOTE:  `series_chunk_metric_prefix` and its `_u8` sibling existed only to bound the
// whole-chunk-family backward scan in `latest_series_timestamp`.  Nothing scans the whole family
// any more, so they are gone.  Reintroducing a key that spans every segment for a metric type
// recreates the whole-family scan problem; do it deliberately if at all.

/// Construct the key for an encoded series chunk.
///
/// The tuple is `(0, 0, metric_type, segment_start, series_fingerprint,
/// last_sample_ts)`.  The value is a [`SeriesChunk`].
pub fn series_chunk_key(
    metric_type: MetricType,
    segment_start: Time,
    fingerprint: [u8; 16],
    last_sample_ts: Time,
) -> Vec<u8> {
    TupleKey::builder()
        .u8(FAMILY_SERIES)
        .u8(SERIES_CHUNK)
        .u8(metric_type.to_u8())
        .i64(segment_start.0)
        .bytes(fingerprint)
        .i64(last_sample_ts.0)
        .build()
        .into_bytes()
}

/// Construct the metadata key for a physical series.
///
/// The tuple is `(0, 1, metric_type, series_fingerprint)`.  The value is the
/// canonical tag string.
pub fn series_tags_key(metric_type: MetricType, fingerprint: [u8; 16]) -> Vec<u8> {
    TupleKey::builder()
        .u8(FAMILY_SERIES)
        .u8(SERIES_TAGS)
        .u8(metric_type.to_u8())
        .bytes(fingerprint)
        .build()
        .into_bytes()
}

/// Construct one inverted-index posting for a canonical tag.
///
/// The tuple is `(1, metric_type, tag_key, tag_value, series_fingerprint)`.
/// The value is empty.
pub fn tag_index_key(
    metric_type: MetricType,
    key: &str,
    value: &str,
    fingerprint: [u8; 16],
) -> Vec<u8> {
    TupleKey::builder()
        .u8(FAMILY_TAG_INDEX)
        .u8(metric_type.to_u8())
        .string(key)
        .string(value)
        .bytes(fingerprint)
        .build()
        .into_bytes()
}

/// Construct the idempotence checkpoint key for a scrape file.
///
/// The tuple is `(2, content_hash)`.  The value is a [`FileCheckpoint`].
pub fn checkpoint_key(content_hash: [u8; 32]) -> Vec<u8> {
    TupleKey::builder()
        .u8(FAMILY_CHECKPOINT)
        .bytes(content_hash)
        .build()
        .into_bytes()
}

fn parse_series_chunk_key(key: &[u8]) -> Result<(MetricType, Time, [u8; 16], Time), SError> {
    let tuple = TupleKey::from_bytes(key.to_vec());
    let mut parser = tuple.parser();
    let family = parser.u8().map_err(|err| coding_error(err.to_string()))?;
    let subfamily = parser.u8().map_err(|err| coding_error(err.to_string()))?;
    if family != FAMILY_SERIES || subfamily != SERIES_CHUNK {
        return Err(coding_error("key is not a series chunk key"));
    }
    let metric_type =
        MetricType::from_u8(parser.u8().map_err(|err| coding_error(err.to_string()))?)
            .ok_or_else(|| coding_error("bad metric type in series chunk key"))?;
    let segment = Time(parser.i64().map_err(|err| coding_error(err.to_string()))?);
    let fingerprint: [u8; 16] = parser
        .bytes()
        .map_err(|err| coding_error(err.to_string()))?
        .try_into()
        .map_err(|_| coding_error("bad fingerprint length"))?;
    let last_ts = Time(parser.i64().map_err(|err| coding_error(err.to_string()))?);
    parser
        .finish()
        .map_err(|err| coding_error(err.to_string()))?;
    Ok((metric_type, segment, fingerprint, last_ts))
}

fn parse_tag_index_key(key: &[u8]) -> Result<(MetricType, String, String, [u8; 16]), SError> {
    let tuple = TupleKey::from_bytes(key.to_vec());
    let mut parser = tuple.parser();
    let family = parser.u8().map_err(|err| coding_error(err.to_string()))?;
    if family != FAMILY_TAG_INDEX {
        return Err(coding_error("key is not a tag index key"));
    }
    let metric_type =
        MetricType::from_u8(parser.u8().map_err(|err| coding_error(err.to_string()))?)
            .ok_or_else(|| coding_error("bad metric type in tag index key"))?;
    let tag_key = parser
        .string()
        .map_err(|err| coding_error(err.to_string()))?;
    let tag_value = parser
        .string()
        .map_err(|err| coding_error(err.to_string()))?;
    let fingerprint: [u8; 16] = parser
        .bytes()
        .map_err(|err| coding_error(err.to_string()))?
        .try_into()
        .map_err(|_| coding_error("bad fingerprint length"))?;
    parser
        .finish()
        .map_err(|err| coding_error(err.to_string()))?;
    Ok((metric_type, tag_key, tag_value, fingerprint))
}

fn content_hash(contents: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(contents);
    hasher.finalize().into()
}

fn ingest_timestamp() -> Result<u64, SError> {
    let now = Time::now().ok_or_else(|| time_error("could not get current time"))?;
    u64::try_from(now.0).map_err(|_| time_error("current time is before epoch"))
}

fn prometheus_timestamp(timestamp: Option<f64>) -> Result<Time, SError> {
    let timestamp = timestamp.ok_or_else(|| text_error("metric reading lacks timestamp"))?;
    if !timestamp.is_finite() || timestamp < 0.0 || timestamp.fract() != 0.0 {
        return Err(text_error("metric timestamp is not a non-negative integer"));
    }
    if timestamp > (i64::MAX / 1000) as f64 {
        return Err(arithmetic_error("metric timestamp exceeds i64 micros"));
    }
    let millis = timestamp as i64;
    let micros = millis
        .checked_mul(1000)
        .ok_or_else(|| arithmetic_error("metric timestamp multiplication overflowed"))?;
    Time::from_micros(micros).ok_or_else(|| time_error("metric timestamp did not parse"))
}

fn reading_sensor_type(
    declarations: &BTreeMap<String, SensorType>,
    metric_name: &str,
) -> Result<Option<SensorType>, SError> {
    if let Some(sensor_type) = declarations.get(metric_name).copied() {
        if sensor_type == SensorType::Histogram {
            return Err(text_error(format!(
                "histogram metric {metric_name} had a bare reading"
            )));
        }
        return Ok(Some(sensor_type));
    }
    for suffix in ["_bucket", "_sum", "_count"] {
        if let Some(base) = metric_name.strip_suffix(suffix)
            && declarations.get(base) == Some(&SensorType::Histogram)
        {
            return Ok(None);
        }
    }
    Err(text_error(format!(
        "metric reading {metric_name} lacks TYPE declaration"
    )))
}

/// Whether an input label key is reserved.
///
/// This used to guard `__saros_*__` only, so `__name__` was accepted from callers.
/// `canonical_tags` pushes its own `__name__` and then appends caller labels unfiltered, and
/// `series_fingerprint` inserts tags into a setsum, which is order-independent -- so
/// `http_x{__name__="http_y"}` and `http_y{__name__="http_x"}` canonicalize to the same multiset
/// and the same fingerprint.  The collision guard compares tag strings, which are equal too, so
/// it cannot fire.
///
/// The rule is now the one Prometheus itself uses:  the `__` prefix is reserved.  That covers
/// `__name__`, `__saros_source__`, and whatever Prometheus reserves next.
fn is_reserved_saros_tag(key: &str) -> bool {
    key.starts_with("__")
}

fn _decode_checkpoint(value: &[u8]) -> Result<FileCheckpoint, SError> {
    let mut unpacker = Unpacker::new(value);
    unpacker
        .unpack()
        .map_err(|err: prototk::SError| coding_error(err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{QueryEngine, query};

    fn test_root(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("saros-{name}-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn one_second_params(start: Time, limit: Time) -> query::QueryParams {
        query::QueryParams::new(
            Window::new(start, limit).unwrap(),
            Time::from_secs(1).unwrap(),
        )
        .unwrap()
    }

    fn fetch_counter_series(
        store: &SarosStore,
        tags: &str,
        params: query::QueryParams,
    ) -> FetchCountersResponse {
        store
            .fetch_counters(
                &rpc_pb::Context::default(),
                FetchCountersRequest {
                    params,
                    tags: tags.to_string(),
                },
            )
            .unwrap()
    }

    #[test]
    fn canonical_fingerprint_is_label_order_independent() {
        let mut lhs = HashMap::new();
        lhs.insert("b".to_string(), "2".to_string());
        lhs.insert("a".to_string(), "1".to_string());
        let mut rhs = HashMap::new();
        rhs.insert("a".to_string(), "1".to_string());
        rhs.insert("b".to_string(), "2".to_string());
        let lhs = canonical_tags("metric", &lhs, "source").unwrap();
        let rhs = canonical_tags("metric", &rhs, "source").unwrap();
        assert_eq!(lhs, rhs);
        assert_eq!(series_fingerprint(&lhs), series_fingerprint(&rhs));
    }

    #[test]
    fn canonical_tags_reject_reserved_saros_labels() {
        let mut labels = HashMap::new();
        labels.insert("__saros_source__".to_string(), "spoof".to_string());
        assert!(canonical_tags("metric", &labels, "source").is_err());
    }

    #[test]
    fn segment_start_uses_two_hour_boundaries() {
        assert_eq!(0, segment_start(Time::from_micros(0).unwrap()).to_micros());
        assert_eq!(
            0,
            segment_start(Time::from_micros(SEGMENT_DURATION.to_micros() - 1).unwrap()).to_micros()
        );
        assert_eq!(
            SEGMENT_DURATION.to_micros(),
            segment_start(Time::from_micros(SEGMENT_DURATION.to_micros()).unwrap()).to_micros()
        );
    }

    #[test]
    fn prometheus_ingest_rejects_missing_type_timestamp_and_reserved_label() {
        let root = test_root("strict");
        let mut store = SarosStore::open(&root).unwrap();
        assert!(
            store
                .ingest_prometheus_bytes("missing-type.prom", b"foo 1 42\n", "source")
                .is_err()
        );
        assert!(
            store
                .ingest_prometheus_bytes(
                    "missing-timestamp.prom",
                    b"# TYPE foo counter\nfoo 1\n",
                    "source"
                )
                .is_err()
        );
        assert!(
            store
                .ingest_prometheus_bytes(
                    "reserved.prom",
                    b"# TYPE foo counter\nfoo{__saros_source__=\"bad\"} 1 42\n",
                    "source"
                )
                .is_err()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prometheus_ingest_checkpoints_by_content_hash() {
        let root = test_root("checkpoint");
        let mut store = SarosStore::open(&root).unwrap();
        let contents = b"# TYPE foo counter\nfoo 1 42\n";
        assert!(
            store
                .ingest_prometheus_bytes("first.prom", contents, "source")
                .unwrap()
        );
        store.flush().unwrap();
        let mut is_tombstone = false;
        let checkpoint = store
            .tree
            .load(&checkpoint_key(content_hash(contents)), &mut is_tombstone)
            .unwrap()
            .unwrap();
        assert!(!is_tombstone);
        let checkpoint = _decode_checkpoint(&checkpoint).unwrap();
        assert_eq!(
            FileCheckpoint {
                basename: "first.prom".to_string(),
                ingested_at: checkpoint.ingested_at,
                supported_types: SUPPORTED_METRIC_TYPES,
            },
            checkpoint
        );
        assert!(
            !store
                .ingest_prometheus_bytes("renamed.prom", contents, "source")
                .unwrap()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_prometheus_ingest_does_not_flush_partial_samples() {
        let root = test_root("failed-transaction");
        let mut store = SarosStore::open(&root).unwrap();
        assert!(
            store
                .ingest_prometheus_bytes(
                    "bad.prom",
                    b"# TYPE foo counter\nfoo 1 0\nbar 2 1000\n",
                    "source"
                )
                .is_err()
        );
        store.flush().unwrap();

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(2_000_000).unwrap(),
        );
        let resp = fetch_counter_series(&store, ":__name__=foo:", params);
        assert!(resp.serieses.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_histogram_file_is_checkpointed_without_counter_series() {
        let root = test_root("histogram-checkpoint");
        let mut store = SarosStore::open(&root).unwrap();
        let contents = br#"# TYPE request_duration_seconds histogram
request_duration_seconds_bucket{le="1"} 7 0
request_duration_seconds_sum 12 0
request_duration_seconds_count 7 0
"#;
        assert!(
            store
                .ingest_prometheus_bytes("histogram.prom", contents, "source")
                .unwrap()
        );
        store.flush().unwrap();
        assert!(
            !store
                .ingest_prometheus_bytes("histogram-renamed.prom", contents, "source")
                .unwrap()
        );

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(1_000_000).unwrap(),
        );
        let resp =
            fetch_counter_series(&store, ":__name__=request_duration_seconds_count:", params);
        assert!(resp.serieses.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_id_is_part_of_physical_series_identity() {
        let root = test_root("source-identity");
        let mut store = SarosStore::open(&root).unwrap();
        store
            .ingest_prometheus_bytes(
                "source-a.prom",
                b"# TYPE foo counter\nfoo{host=\"h\"} 1 0\n",
                "source-a",
            )
            .unwrap();
        store
            .ingest_prometheus_bytes(
                "source-b.prom",
                b"# TYPE foo counter\nfoo{host=\"h\"} 3 0\n",
                "source-b",
            )
            .unwrap();
        store.flush().unwrap();

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(1_000_000).unwrap(),
        );
        let resp = fetch_counter_series(&store, ":__name__=foo:", params);
        let window = params.window();
        let step = params.step();
        let mut serieses = Vec::new();
        for fetched in resp.serieses {
            let series = crate::Series::decode_chunks(None, window, step, &fetched.chunks)
                .unwrap()
                .unwrap();
            serieses.push((fetched.tags, series.points().to_vec()));
        }
        serieses.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        assert_eq!(
            vec![
                (
                    ":__name__=foo:__saros_source__=source-a:host=h:".to_string(),
                    vec![Point(1.0)],
                ),
                (
                    ":__name__=foo:__saros_source__=source-b:host=h:".to_string(),
                    vec![Point(3.0)],
                ),
            ],
            serieses
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_timestamp_before_flush_keeps_last_value() {
        let root = test_root("duplicate-before-flush");
        let mut store = SarosStore::open(&root).unwrap();
        store
            .ingest_prometheus_bytes(
                "samples.prom",
                b"# TYPE foo counter\nfoo 1 0\nfoo 2 0\n",
                "source",
            )
            .unwrap();
        store.flush().unwrap();

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(1_000_000).unwrap(),
        );
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert_eq!(vec![Point(2.0)], series[0].points());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_timestamp_after_flush_is_rejected_without_new_checkpoint() {
        let root = test_root("duplicate-after-flush");
        let mut store = SarosStore::open(&root).unwrap();
        store
            .ingest_prometheus_bytes("first.prom", b"# TYPE foo counter\nfoo 1 0\n", "source")
            .unwrap();
        store.flush().unwrap();
        assert!(
            store
                .ingest_prometheus_bytes(
                    "duplicate.prom",
                    b"# TYPE foo counter\nfoo 2 0\n",
                    "source"
                )
                .is_err()
        );
        store.flush().unwrap();

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(1_000_000).unwrap(),
        );
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert_eq!(vec![Point(1.0)], series[0].points());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restart_rejects_duplicate_timestamp_at_disk_frontier() {
        let root = test_root("restart-duplicate-frontier");
        let mut store = SarosStore::open(&root).unwrap();
        store
            .ingest_prometheus_bytes("first.prom", b"# TYPE foo counter\nfoo 1 0\n", "source")
            .unwrap();
        store.flush().unwrap();
        drop(store);

        let mut store = SarosStore::open(&root).unwrap();
        assert!(
            store
                .ingest_prometheus_bytes(
                    "duplicate.prom",
                    b"# TYPE foo counter\nfoo 2 0\n",
                    "source"
                )
                .is_err()
        );
        store.flush().unwrap();

        let params = one_second_params(
            Time::from_micros(0).unwrap(),
            Time::from_micros(1_000_000).unwrap(),
        );
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert_eq!(vec![Point(1.0)], series[0].points());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restart_rejects_out_of_order_sample_after_later_segment() {
        let root = test_root("restart-frontier-later-segment");
        let mut store = SarosStore::open(&root).unwrap();
        let segment_ms = SEGMENT_DURATION.to_micros() / 1000;
        let initial = format!(
            "# TYPE foo counter\nfoo 1 {}\nfoo 2 {}\n",
            segment_ms - 1_000,
            segment_ms + 10_000
        );
        store
            .ingest_prometheus_bytes("initial.prom", initial.as_bytes(), "source")
            .unwrap();
        store.flush().unwrap();
        drop(store);

        let mut store = SarosStore::open(&root).unwrap();
        let stale = format!("# TYPE foo counter\nfoo 3 {}\n", segment_ms + 5_000);
        assert!(
            store
                .ingest_prometheus_bytes("stale.prom", stale.as_bytes(), "source")
                .is_err()
        );
        let newer = format!("# TYPE foo counter\nfoo 4 {}\n", segment_ms + 20_000);
        assert!(
            store
                .ingest_prometheus_bytes("newer.prom", newer.as_bytes(), "source")
                .unwrap()
        );
        store.flush().unwrap();

        let start = Time::from_micros(SEGMENT_DURATION.to_micros()).unwrap();
        let limit = Time::from_micros(SEGMENT_DURATION.to_micros() + 25_000_000).unwrap();
        let window = Window::new(start, limit).unwrap();
        let params = query::QueryParams::new(window, Time::from_secs(5).unwrap()).unwrap();
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert_eq!(
            vec![Point(1.0), Point(1.0), Point(2.0), Point(2.0), Point(4.0),],
            series[0].points()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn series_state_flushes_around_target_size_and_below_hard_max() {
        let labels = HashMap::new();
        let tags = canonical_tags("foo", &labels, "source").unwrap();
        let fingerprint = series_fingerprint(&tags);
        let mut state = SeriesState::with_last_ts(MetricType::Counter, fingerprint, tags, None);
        let mut rows = Vec::new();
        for idx in 0..1_500 {
            let bits = 0x3ff0_0000_0000_0000u64 ^ ((idx as u64) << 31) ^ (idx as u64);
            state
                .push(
                    Time::from_micros(idx * 1_000).unwrap(),
                    Point(f64::from_bits(bits)),
                    &mut rows,
                )
                .unwrap();
            if rows
                .iter()
                .any(|row| parse_series_chunk_key(&row.key).is_ok())
            {
                break;
            }
        }
        state.flush_pending(&mut rows).unwrap();
        let mut chunk_sizes = Vec::new();
        for row in rows.iter() {
            if parse_series_chunk_key(&row.key).is_ok() {
                let chunk = SeriesChunk::decode(&row.value).unwrap();
                assert_eq!(
                    segment_start(chunk.first_sample_ts),
                    segment_start(chunk.last_sample_ts)
                );
                assert!(row.value.len() <= CHUNK_MAX_BYTES);
                chunk_sizes.push(row.value.len());
            }
        }
        assert!(chunk_sizes.len() >= 2);
        assert!(chunk_sizes[0] >= CHUNK_TARGET_BYTES);
    }

    #[test]
    fn counter_query_carries_predecessor_across_segment_start() {
        let root = test_root("predecessor");
        let mut store = SarosStore::open(&root).unwrap();
        let segment_ms = SEGMENT_DURATION.to_micros() / 1000;
        let contents = format!(
            "# TYPE foo counter\nfoo 1 {}\nfoo 2 {}\n",
            segment_ms - 1_000,
            segment_ms + 10_000
        );
        store
            .ingest_prometheus_bytes("samples.prom", contents.as_bytes(), "source")
            .unwrap();
        store.flush().unwrap();

        let start = Time::from_micros(SEGMENT_DURATION.to_micros()).unwrap();
        let limit = Time::from_micros(SEGMENT_DURATION.to_micros() + 30_000_000).unwrap();
        let window = Window::new(start, limit).unwrap();
        let params = query::QueryParams::new(window, Time::from_secs(10).unwrap()).unwrap();
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert_eq!(vec![Point(1.0), Point(2.0), Point(2.0)], series[0].points());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn counter_query_uses_nan_until_first_sample() {
        let root = test_root("leading-nan");
        let mut store = SarosStore::open(&root).unwrap();
        let segment_ms = SEGMENT_DURATION.to_micros() / 1000;
        let contents = format!("# TYPE foo counter\nfoo 2 {}\n", segment_ms + 10_000);
        store
            .ingest_prometheus_bytes("samples.prom", contents.as_bytes(), "source")
            .unwrap();
        store.flush().unwrap();

        let start = Time::from_micros(SEGMENT_DURATION.to_micros()).unwrap();
        let limit = Time::from_micros(SEGMENT_DURATION.to_micros() + 30_000_000).unwrap();
        let window = Window::new(start, limit).unwrap();
        let params = query::QueryParams::new(window, Time::from_secs(10).unwrap()).unwrap();
        let engine = QueryEngine::new(store);
        let series = engine
            .query(&rpc_pb::Context::default(), "counters(foo)", params)
            .unwrap();
        assert_eq!(1, series.len());
        assert!(series[0].points()[0].0.is_nan());
        assert_eq!(Point(2.0), series[0].points()[1]);
        assert_eq!(Point(2.0), series[0].points()[2]);
        let _ = std::fs::remove_dir_all(root);
    }
}
