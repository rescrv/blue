//! In-memory implementations of the saros traits.

use std::collections::HashMap;

use biometrics::moments::Moments;
use zerror_core::ErrorCore;

use super::{Error, Label, MetricID, MetricType, Point, Series, Tags, Time, Window};

///////////////////////////////////////////// MetricKey ////////////////////////////////////////////

#[derive(Debug, Default, Eq, PartialEq, Hash)]
struct MetricKey {
    metric_type: MetricType,
    label: Label,
    tags: Tags,
}

impl MetricKey {
    fn is(&self, metric_type: MetricType, label: &Label, tags: &Tags) -> bool {
        self.metric_type == metric_type && self.label == *label && self.tags == *tags
    }

    fn matches(&self, metric_type: MetricType, label: &Label, tags: &Tags) -> bool {
        self.metric_type == metric_type && self.label == *label && self.tags.contains(tags)
    }
}

////////////////////////////////////////// BiometricsStore /////////////////////////////////////////

/// An in-memory BiometricsStore.
#[derive(Default)]
pub struct BiometricsStore {
    metrics_by_key: Vec<(MetricKey, MetricID)>,
    counters_by_metric_id: HashMap<MetricID, Series<i64>>,
    gauges_by_metric_id: HashMap<MetricID, Series<f64>>,
    moments_by_metric_id: HashMap<MetricID, Series<Moments>>,
}

impl BiometricsStore {
    /// Update the series associated with the counter to have the prescribed reading.
    pub fn push_counter(&mut self, label: &Label, tags: &Tags, now: Time, reading: u64) {
        if let Some(metric_id) =
            self.metric_id_for_type_label_tags(MetricType::Counter, label, tags)
        {
            Self::push_collection(
                &mut self.counters_by_metric_id,
                metric_id,
                now,
                reading as i64,
            )
        } else {
            super::DROPPED_METRICS.click();
        }
    }

    /// Update the series associated with the gauge to have the prescribed reading.
    pub fn push_gauge(&mut self, label: &Label, tags: &Tags, now: Time, reading: f64) {
        if let Some(metric_id) = self.metric_id_for_type_label_tags(MetricType::Gauge, label, tags)
        {
            Self::push_collection(&mut self.gauges_by_metric_id, metric_id, now, reading)
        } else {
            super::DROPPED_METRICS.click();
        }
    }

    /// Update the series associated with the moments to have the prescribed reading.
    pub fn push_moments(&mut self, label: &Label, tags: &Tags, now: Time, reading: Moments) {
        if let Some(metric_id) =
            self.metric_id_for_type_label_tags(MetricType::Moments, label, tags)
        {
            Self::push_collection(&mut self.moments_by_metric_id, metric_id, now, reading)
        } else {
            super::DROPPED_METRICS.click();
        }
    }

    fn metric_id_for_type_label_tags(
        &mut self,
        metric_type: MetricType,
        label: &Label,
        tags: &Tags,
    ) -> Option<MetricID> {
        for (metric_key, metric_id) in self.metrics_by_key.iter() {
            if metric_key.is(metric_type, label, tags) {
                return Some(*metric_id);
            }
        }
        let metric_id = MetricID::generate()?;
        self.metrics_by_key.push((
            MetricKey {
                metric_type,
                label: label.clone(),
                tags: tags.clone(),
            },
            metric_id,
        ));
        Some(metric_id)
    }

    fn push_collection<P: Point>(
        collection: &mut HashMap<MetricID, Series<P>>,
        metric_id: MetricID,
        now: Time,
        reading: P,
    ) {
        let series = collection.entry(metric_id).or_default();
        if !series.points.is_empty() && series.points[series.points.len() - 1].0 > now {
            super::TIME_TRAVEL.click();
        } else {
            series.points.push((now, reading));
        }
    }

    fn series_from_collection<P: Point>(
        collection: &HashMap<MetricID, Series<P>>,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<P>, Error> {
        if let Some(series) = collection.get(&metric_id) {
            Ok(series.filter(window))
        } else {
            Err(Error::UnknownMetric {
                core: ErrorCore::default(),
                metric_id,
            })
        }
    }
}

impl super::BiometricsStore for BiometricsStore {
    fn metrics_by_label(
        &self,
        metric_type: MetricType,
        label: &Label,
        tags: &Tags,
        _: Window,
    ) -> Result<Vec<MetricID>, Error> {
        // TODO(rescrv): If window doesn't overlap the window for this store, return immediately.
        let mut metrics = vec![];
        for (metric_key, metric_id) in self.metrics_by_key.iter() {
            if metric_key.matches(metric_type, label, tags) {
                metrics.push(*metric_id);
            }
        }
        Ok(metrics)
    }

    fn counter_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<i64>, Error> {
        Self::series_from_collection(&self.counters_by_metric_id, metric_id, window)
    }

    fn gauge_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<f64>, Error> {
        Self::series_from_collection(&self.gauges_by_metric_id, metric_id, window)
    }

    fn moments_by_metric_id(
        &self,
        metric_id: MetricID,
        window: Window,
    ) -> Result<Series<Moments>, Error> {
        Self::series_from_collection(&self.moments_by_metric_id, metric_id, window)
    }
}
