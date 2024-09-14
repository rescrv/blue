use biometrics::{Counter, Gauge, Histogram, Moments, Sensor};
use utf8path::Path;

pub struct Emitter<'a> {
    #[allow(dead_code)]
    path: Path<'a>,
}

impl<'a> Emitter<'a> {
    pub fn new(path: Path<'a>) -> Self {
        Self { path }
    }
}

#[allow(deprecated)]
impl<'a> biometrics::Emitter for Emitter<'a> {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &Counter, now: u64) -> Result<(), std::io::Error> {
        let label = counter.label();
        let reading = counter.read();
        println!(
            "# TYPE {label} counter
{label} {reading} {now}"
        );
        Ok(())
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: u64) -> Result<(), std::io::Error> {
        let label = gauge.label();
        let reading = gauge.read();
        println!(
            "# TYPE {label} gauge
{label} {reading} {now}"
        );
        Ok(())
    }

    fn emit_moments(&mut self, moments: &Moments, now: u64) -> Result<(), std::io::Error> {
        let label = moments.label();
        let reading = moments.read();
        println!(
            "# TYPE {label}_count counter
{label}_count {} {now}
# TYPE {label}_mean gauge
{label}_mean {} {now}
# TYPE {label}_variance gauge
{label}_variance {} {now}
# TYPE {label}_skewness gauge
{label}_skewness {} {now}
# TYPE {label}_kurtosis gauge
{label}_kurtosis {} {now}",
            reading.n(),
            reading.mean(),
            reading.variance(),
            reading.skewness(),
            reading.kurtosis(),
        );
        Ok(())
    }

    fn emit_histogram(&mut self, histogram: &Histogram, now: u64) -> Result<(), std::io::Error> {
        let label = histogram.label();
        println!("# TYPE {label} histogram");
        let mut total = 0;
        let mut acc = 0.0;
        for (bucket, count) in histogram.read().iter() {
            total += count;
            acc += bucket * count as f64;
            println!("{label}_bucket{{le=\"{bucket:0.4}\"}} {total} {now}");
        }
        println!("{label}_sum {acc} {now}");
        println!("{label}_count {total} {now}");
        let exceeds_max = histogram.exceeds_max().read();
        println!("{label}_exceeds_max {exceeds_max} {now}");
        let is_negative = histogram.is_negative().read();
        println!("{label}_is_negative {is_negative} {now}");
        Ok(())
    }
}
