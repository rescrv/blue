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
    BiometricsStore, FetchCountersRequest, FetchCountersResponse, FetchedSeries, MetricType, Point,
    SError as SarosError, SeriesChunk, Time, Window, arithmetic_error, coding_error,
    internal_error, system_error, text_error, time_error,
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

/// Records that a Prometheus scrape file has been ingested.
#[derive(Clone, Debug, Default, PartialEq, prototk_derive::Message)]
pub struct FileCheckpoint {
    /// Basename of the ingested file, kept for debugging only.
    #[prototk(1, string)]
    pub basename: String,
    /// Wall-clock ingest time for the checkpoint row.
    #[prototk(2, message)]
    pub ingested_at: Time,
}

#[derive(Clone, Debug)]
struct Row {
    key: Vec<u8>,
    timestamp: u64,
    value: Vec<u8>,
}

#[derive(Clone, Debug)]
struct SeriesState {
    metric_type: MetricType,
    fingerprint: [u8; 16],
    tags: Tags<'static>,
    metadata_emitted: bool,
    pending: Vec<(Time, Point)>,
    pending_over_target: bool,
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
            pending: Vec::new(),
            pending_over_target: false,
            last_ts,
        }
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
            if self.pending.is_empty() && time == last_ts {
                return Err(time_error(
                    "duplicate timestamp arrived after its chunk was flushed",
                ));
            }
        }
        if self
            .pending
            .last()
            .is_some_and(|(pending_time, _)| *pending_time == time)
        {
            let last = self.pending.last_mut().expect("checked pending last");
            last.1 = point;
            self.refresh_pending_size()?;
            self.last_ts = Some(time);
            return Ok(());
        }
        if !self.pending.is_empty() {
            let pending_segment = segment_start(self.pending[0].0);
            let next_segment = segment_start(time);
            if pending_segment != next_segment || self.pending_over_target {
                self.flush_pending(rows)?;
            }
        }
        self.pending.push((time, point));
        self.last_ts = Some(time);
        self.refresh_pending_size()
    }

    fn flush_pending(&mut self, rows: &mut Vec<Row>) -> Result<(), SError> {
        if self.pending.is_empty() {
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
        let chunk = SeriesChunk::from_samples(self.metric_type, &self.pending)?;
        let value = chunk.encode();
        if value.len() > CHUNK_MAX_BYTES {
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
        self.pending.clear();
        self.pending_over_target = false;
        Ok(())
    }

    fn refresh_pending_size(&mut self) -> Result<(), SError> {
        let chunk = SeriesChunk::from_samples(self.metric_type, &self.pending)?;
        let size = chunk.encode().len();
        if size > CHUNK_MAX_BYTES {
            return Err(coding_error(format!(
                "series chunk exceeded max bytes: {size} > {CHUNK_MAX_BYTES}"
            )));
        }
        self.pending_over_target = size >= CHUNK_TARGET_BYTES;
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
    rows: Vec<Row>,
    pending_checkpoints: BTreeSet<[u8; 32]>,
    series: BTreeMap<(MetricType, [u8; 16]), SeriesState>,
    flush_counter: u64,
}

impl SarosStore {
    /// Open an existing store or create a new one at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying LSM tree cannot be opened.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SError> {
        let root = path.as_ref().to_path_buf();
        let options = LsmtkOptions::default().with_path(root.to_string_lossy().to_string());
        let tree = LsmTree::open(options)?;
        Ok(Self {
            root,
            tree,
            rows: Vec::new(),
            pending_checkpoints: BTreeSet::new(),
            series: BTreeMap::new(),
            flush_counter: 0,
        })
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
            || self.checkpoint_exists(content_hash)?
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
        if self.rows.is_empty() {
            return Ok(());
        }
        let mut rows = self.rows.clone();
        rows.sort_by(|lhs, rhs| {
            KeyRef::new(&lhs.key, lhs.timestamp).cmp(&KeyRef::new(&rhs.key, rhs.timestamp))
        });
        rows.dedup_by(|lhs, rhs| lhs.key == rhs.key && lhs.timestamp == rhs.timestamp);
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
        self.rows.clear();
        self.pending_checkpoints.clear();
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
            for (time, point) in batch_series.samples {
                state.push(time, point, &mut self.rows)?;
            }
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
                            (state.last_ts, !state.pending.is_empty())
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

    fn latest_series_timestamp(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
    ) -> Result<Option<Time>, SError> {
        let start = series_chunk_metric_prefix(metric_type);
        let end = series_chunk_metric_prefix_u8(metric_type.to_u8() + 1);
        let start_bound = Bound::Included(start);
        let end_bound = Bound::Excluded(end);
        let mut cursor = self.tree.range_scan(&start_bound, &end_bound)?;
        cursor.seek_to_last()?;
        cursor.prev()?;
        while let Some(kvr) = cursor.key_value() {
            let (_, _, key_fingerprint, last_ts) = parse_series_chunk_key(kvr.key)?;
            if key_fingerprint == fingerprint && kvr.value.is_some() {
                return Ok(Some(last_ts));
            }
            cursor.prev()?;
        }
        Ok(None)
    }

    fn checkpoint_exists(&self, content_hash: [u8; 32]) -> Result<bool, SError> {
        let mut is_tombstone = false;
        Ok(self
            .tree
            .load(&checkpoint_key(content_hash), &mut is_tombstone)?
            .is_some()
            && !is_tombstone)
    }

    fn emit_checkpoint(&mut self, content_hash: [u8; 32], basename: &str) -> Result<(), SError> {
        let ingested_at = Time::now().ok_or_else(|| time_error("could not get current time"))?;
        let checkpoint = FileCheckpoint {
            basename: basename.to_string(),
            ingested_at,
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

    fn matching_fingerprints(
        &self,
        metric_type: MetricType,
        tags: &Tags<'_>,
    ) -> Result<Vec<[u8; 16]>, SError> {
        let mut sets = Vec::new();
        for tag in tags.tags() {
            sets.push(self.posting_list(metric_type, tag.key(), tag.value())?);
        }
        if sets.is_empty() {
            return Ok(Vec::new());
        }
        sets.sort_by_key(Vec::len);
        let mut intersection = sets.remove(0);
        for set in sets {
            let set: BTreeSet<_> = set.into_iter().collect();
            intersection.retain(|fingerprint| set.contains(fingerprint));
        }
        Ok(intersection)
    }

    fn posting_list(
        &self,
        metric_type: MetricType,
        tag_key: &str,
        tag_value: &str,
    ) -> Result<Vec<[u8; 16]>, SError> {
        let start = tag_index_key(metric_type, tag_key, tag_value, [0u8; 16]);
        let end = tag_index_key(metric_type, tag_key, tag_value, [0xffu8; 16]);
        let start_bound = Bound::Included(start);
        let end_bound = Bound::Included(end);
        let mut cursor = self.tree.range_scan(&start_bound, &end_bound)?;
        let mut fingerprints = Vec::new();
        cursor.next()?;
        while let Some(kvr) = cursor.key_value() {
            let (_, _, _, fingerprint) = parse_tag_index_key(kvr.key)?;
            fingerprints.push(fingerprint);
            cursor.next()?;
        }
        fingerprints.sort();
        fingerprints.dedup();
        Ok(fingerprints)
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

    fn predecessor_chunk(
        &self,
        metric_type: MetricType,
        fingerprint: [u8; 16],
        time: Time,
    ) -> Result<Option<(Time, Time, SeriesChunk)>, SError> {
        let mut segment = segment_start(time);
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
        let mut serieses = Vec::new();
        for fingerprint in self.matching_fingerprints(MetricType::Counter, &req_tags)? {
            let Some(tags) = self.load_tags(MetricType::Counter, fingerprint)? else {
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
    Ok(Tags::from(tags))
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

fn series_chunk_metric_prefix(metric_type: MetricType) -> Vec<u8> {
    series_chunk_metric_prefix_u8(metric_type.to_u8())
}

fn series_chunk_metric_prefix_u8(metric_type: u8) -> Vec<u8> {
    TupleKey::builder()
        .u8(FAMILY_SERIES)
        .u8(SERIES_CHUNK)
        .u8(metric_type)
        .build()
        .into_bytes()
}

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

fn is_reserved_saros_tag(key: &str) -> bool {
    key.starts_with("__saros_") && key.ends_with("__")
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
