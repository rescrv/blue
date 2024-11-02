use std::collections::{BTreeMap, HashSet};

use tag_index::{Tag, Tags};
use utf8path::Path;

use super::prometheus::{PrometheusLine, SensorType};
use super::*;

////////////////////////////////////// RecoveryBiometricsStore /////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct RecoveryBiometricsStore {
    counters: BTreeMap<Tags<'static>, BTreeMap<Time, Point>>,
    gauges: BTreeMap<Tags<'static>, BTreeMap<Time, Point>>,
}

impl RecoveryBiometricsStore {
    pub fn new() -> Self {
        Self {
            counters: BTreeMap::default(),
            gauges: BTreeMap::default(),
        }
    }

    pub fn window(&self) -> Option<Window> {
        let mut start = None;
        let mut limit = None;
        self.counters.iter().for_each(|(_, data)| {
            data.iter().for_each(|(ts, _)| {
                start = Some(std::cmp::min(start.unwrap_or(*ts), *ts));
                limit = Some(std::cmp::max(limit.unwrap_or(*ts), *ts));
            });
        });
        Window::new(start?, limit?)
    }

    pub fn load(&mut self, path: &Path) -> Result<(), Error> {
        let base = path.basename().into_owned();
        let Some(tags_ts) = base.as_str().strip_suffix(".prom") else {
            return Err(Error::text(format!(
                "filename ({}) did not end in .prom",
                path
            )));
        };
        let Some((tags, ts)) = tags_ts.rsplit_once('.') else {
            todo!();
        };
        let tags = Tags::new(tags).ok_or_else(|| Error::text("tags did not parse"))?;
        let _ts: i64 = ts
            .parse()
            .map_err(|_| Error::text("timestamp in filename did not parse"))?;
        let contents = std::fs::read_to_string(path)?;
        let prometheus_lines = super::support_nom::parse_all(super::prometheus::parse)(&contents)
            .map_err(|e| Error::text(e.to_string()))?;
        let mut counters = HashSet::new();
        let mut gauges = HashSet::new();
        for line in prometheus_lines.iter() {
            if let PrometheusLine::TypeDeclaration(decl) = line {
                match decl.sensor_type() {
                    SensorType::Counter => {
                        counters.insert(decl.label());
                    }
                    SensorType::Gauge => {
                        gauges.insert(decl.label());
                    }
                    // TODO(rescrv): more support
                    _ => {}
                }
            }
        }
        for line in prometheus_lines.iter() {
            if let PrometheusLine::MetricReading(reading) = line {
                let mut tags = tags.tags().collect::<Vec<_>>();
                let tag = Tag::new("__name__", &reading.metric_name)
                    .ok_or_else(|| Error::text("tag did not parse"))?;
                tags.insert(0, tag);
                let local_tags = reading
                    .labels
                    .iter()
                    .map(|(k, v)| {
                        Tag::new(k, v)
                            .ok_or_else(|| Error::text("tag did not parse"))
                            .map(Tag::into_owned)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                tags.extend(local_tags);
                let tags: Tags = Tags::from(tags).into_owned();
                let ts = reading
                    .timestamp
                    .and_then(|ts| Time::from_micros(1_000 * (ts as i64)))
                    .ok_or_else(|| Error::text("timestamp did not parse"))?;
                if counters.contains(reading.metric_name.as_str()) {
                    self.counters
                        .entry(tags)
                        .or_default()
                        .insert(ts, Point(reading.reading));
                } else if gauges.contains(reading.metric_name.as_str()) {
                    self.gauges
                        .entry(tags)
                        .or_default()
                        .insert(ts, Point(reading.reading));
                }
            }
        }
        Ok(())
    }
}

impl BiometricsStore for RecoveryBiometricsStore {
    fn fetch_counters(
        &self,
        _: &rpc_pb::Context,
        req: FetchCountersRequest,
    ) -> Result<FetchCountersResponse, Error> {
        let Some(req_tags) = Tags::new(&req.tags) else {
            todo!();
        };
        let mut serieses = vec![];
        for (tags, data) in self.counters.iter() {
            if req_tags.tags().all(|tag| tags.tags().any(|t| t == tag)) {
                let mut se = SeriesEncoder::default();
                let mut first = None;
                let mut points_pushed = 0;
                for (ts, point) in data.iter() {
                    if (req.params.window().start..req.params.window().limit).contains(ts) {
                        if first.is_none() {
                            first = Some((*ts, *point));
                        }
                        se.push(*ts, *point)?;
                        points_pushed += 1;
                    }
                }
                let Some(first) = first else {
                    continue;
                };
                let series = EncodedSeries::new(first.1, se.as_ref().to_vec());
                let decoder: SeriesDecoder = SeriesDecoder::from(series.bytes.as_ref());
                let returned = decoder.into_iter().collect::<Result<Vec<_>, _>>()?;
                assert_eq!(returned.len(), points_pushed);
                serieses.push(series);
            }
        }
        Ok(FetchCountersResponse { serieses })
    }

    fn fetch_gauges(
        &self,
        _: &rpc_pb::Context,
        _: FetchGaugesRequest,
    ) -> Result<FetchGaugesResponse, Error> {
        Ok(FetchGaugesResponse::default())
    }

    fn fetch_histograms(
        &self,
        _: &rpc_pb::Context,
        _: FetchHistogramsRequest,
    ) -> Result<FetchHistogramsResponse, Error> {
        Ok(FetchHistogramsResponse::default())
    }
}
