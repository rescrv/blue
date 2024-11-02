use std::time::{Duration, SystemTime};

use biometrics::{Collector, Counter, Gauge, Histogram, Moments};
use biometrics_prometheus::{Emitter, Options};
use guacamole::combinators::*;
use guacamole::Guacamole;

static REQUESTS_COUNT: Counter = Counter::new("request_count");
static LAST_REQUEST_INTERARRIVAL_US: Gauge = Gauge::new("last_request_interarrival_us");
static INTERARRIVAL_US_MOMENTS: Moments = Moments::new("interarrival_moments_us");

static INTERARRIVAL_US_HISTOGRAM_IMPL: sig_fig_histogram::LockFreeHistogram<270> =
    sig_fig_histogram::LockFreeHistogram::<270>::new(2);
static INTERARRIVAL_US_HISTOGRAM: Histogram =
    Histogram::new("interarrival_histogram_us", &INTERARRIVAL_US_HISTOGRAM_IMPL);

fn main() {
    let collector = Collector::new();
    collector.register_counter(&REQUESTS_COUNT);
    collector.register_gauge(&LAST_REQUEST_INTERARRIVAL_US);
    collector.register_moments(&INTERARRIVAL_US_MOMENTS);
    collector.register_histogram(&INTERARRIVAL_US_HISTOGRAM);
    let mut emitter = Emitter::new(Options {
        flush_interval: Duration::from_secs(30),
        prefix: ":app=guacamole:.".into(),
        segment_size: 1048576,
    });
    let mut guac = Guacamole::new(0);
    let limit = SystemTime::now();
    let mut start = limit - Duration::from_secs(86400);
    let mut emit_next = start;
    while start < limit {
        let interarrival_time = interarrival_duration(10_000.0)(&mut guac);
        while start + interarrival_time >= emit_next {
            collector
                .emit(
                    &mut emitter,
                    emit_next
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                )
                .unwrap();
            emit_next += Duration::from_secs(10);
        }
        let itus = interarrival_time.as_micros() as f64;
        REQUESTS_COUNT.click();
        LAST_REQUEST_INTERARRIVAL_US.set(itus);
        INTERARRIVAL_US_MOMENTS.add(itus);
        INTERARRIVAL_US_HISTOGRAM.observe(itus);
        start += interarrival_time;
    }
}
