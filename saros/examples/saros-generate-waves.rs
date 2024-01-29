//! Generate sin and cos waves to visually test the timeseries engine.

use std::f64::consts::PI;

use biometrics::{Counter, Gauge, Sensor};

use saros::dashboard::{Dashboard, Panel, Plot, RewindWriter};
use saros::{Query, QueryEngine, Tags, Time, Window};

fn generate_sin(periodicity: u64, second: i64) -> f64 {
    (1.0 + (2.0 * PI * second as f64 / periodicity as f64).sin()) / 2.0
}

fn generate_cos(periodicity: u64, second: i64) -> f64 {
    (1.0 + (2.0 * PI * second as f64 / periodicity as f64).cos()) / 2.0
}

fn generate_saw(periodicity: u64, second: i64) -> f64 {
    (second as u64 % periodicity) as f64 / periodicity as f64
}

fn main() {
    let tags = Tags::default();
    let mut store = saros::memory::BiometricsStore::default();
    static GAUGE_SIN_10: Gauge = Gauge::new("waves.sin.10");
    static GAUGE_SIN_60: Gauge = Gauge::new("waves.sin.60");
    static GAUGE_SIN_1200: Gauge = Gauge::new("waves.sin.1200");
    static GAUGE_SIN_3600: Gauge = Gauge::new("waves.sin.3600");
    static GAUGE_NEG_SIN_10: Gauge = Gauge::new("waves.neg_sin.10");
    static GAUGE_NEG_SIN_60: Gauge = Gauge::new("waves.neg_sin.60");
    static GAUGE_NEG_SIN_1200: Gauge = Gauge::new("waves.neg_sin.1200");
    static GAUGE_NEG_SIN_3600: Gauge = Gauge::new("waves.neg_sin.3600");
    static GAUGE_COS_10: Gauge = Gauge::new("waves.cos.10");
    static GAUGE_COS_60: Gauge = Gauge::new("waves.cos.60");
    static GAUGE_COS_1200: Gauge = Gauge::new("waves.cos.1200");
    static GAUGE_COS_3600: Gauge = Gauge::new("waves.cos.3600");
    static GAUGE_SAW_10: Gauge = Gauge::new("waves.saw.10");
    static GAUGE_SAW_60: Gauge = Gauge::new("waves.saw.60");
    static GAUGE_SAW_1200: Gauge = Gauge::new("waves.saw.1200");
    static GAUGE_SAW_3600: Gauge = Gauge::new("waves.saw.3600");
    static COUNTER_1: Counter = Counter::new("waves.inc.1");
    static COUNTER_2: Counter = Counter::new("waves.inc.2");
    for i in 0..86_400i64 {
        // Sin waves
        GAUGE_SIN_10.set(generate_sin(10, i));
        store.push_gauge(&GAUGE_SIN_10.label().into(), &tags, Time::from_secs(i), GAUGE_SIN_10.read());
        GAUGE_SIN_60.set(generate_sin(60, i));
        store.push_gauge(&GAUGE_SIN_60.label().into(), &tags, Time::from_secs(i), GAUGE_SIN_60.read());
        GAUGE_SIN_1200.set(generate_sin(1200, i));
        store.push_gauge(&GAUGE_SIN_1200.label().into(), &tags, Time::from_secs(i), GAUGE_SIN_1200.read());
        GAUGE_SIN_3600.set(generate_sin(3600, i));
        store.push_gauge(&GAUGE_SIN_3600.label().into(), &tags, Time::from_secs(i), GAUGE_SIN_3600.read());
        // Negative sin waves
        GAUGE_NEG_SIN_10.set(0.0 - generate_sin(10, i));
        store.push_gauge(&GAUGE_NEG_SIN_10.label().into(), &tags, Time::from_secs(i), GAUGE_NEG_SIN_10.read());
        GAUGE_NEG_SIN_60.set(0.0 - generate_sin(60, i));
        store.push_gauge(&GAUGE_NEG_SIN_60.label().into(), &tags, Time::from_secs(i), GAUGE_NEG_SIN_60.read());
        GAUGE_NEG_SIN_1200.set(0.0 - generate_sin(1200, i));
        store.push_gauge(&GAUGE_NEG_SIN_1200.label().into(), &tags, Time::from_secs(i), GAUGE_NEG_SIN_1200.read());
        GAUGE_NEG_SIN_3600.set(0.0 - generate_sin(3600, i));
        store.push_gauge(&GAUGE_NEG_SIN_3600.label().into(), &tags, Time::from_secs(i), GAUGE_NEG_SIN_3600.read());
        // Cos waves
        GAUGE_COS_10.set(generate_cos(10, i));
        store.push_gauge(&GAUGE_COS_10.label().into(), &tags, Time::from_secs(i), GAUGE_COS_10.read());
        GAUGE_COS_60.set(generate_cos(60, i));
        store.push_gauge(&GAUGE_COS_60.label().into(), &tags, Time::from_secs(i), GAUGE_COS_60.read());
        GAUGE_COS_1200.set(generate_cos(1200, i));
        store.push_gauge(&GAUGE_COS_1200.label().into(), &tags, Time::from_secs(i), GAUGE_COS_1200.read());
        GAUGE_COS_3600.set(generate_cos(3600, i));
        store.push_gauge(&GAUGE_COS_3600.label().into(), &tags, Time::from_secs(i), GAUGE_COS_3600.read());
        // Saw waves
        GAUGE_SAW_10.set(generate_saw(10, i));
        store.push_gauge(&GAUGE_SAW_10.label().into(), &tags, Time::from_secs(i), GAUGE_SAW_10.read());
        GAUGE_SAW_60.set(generate_saw(60, i));
        store.push_gauge(&GAUGE_SAW_60.label().into(), &tags, Time::from_secs(i), GAUGE_SAW_60.read());
        GAUGE_SAW_1200.set(generate_saw(1200, i));
        store.push_gauge(&GAUGE_SAW_1200.label().into(), &tags, Time::from_secs(i), GAUGE_SAW_1200.read());
        GAUGE_SAW_3600.set(generate_saw(3600, i));
        store.push_gauge(&GAUGE_SAW_3600.label().into(), &tags, Time::from_secs(i), GAUGE_SAW_3600.read());
        // Counters
        COUNTER_1.click();
        store.push_counter(&COUNTER_1.label().into(), &tags, Time::from_secs(i), COUNTER_1.read());
        COUNTER_2.click();
        COUNTER_2.click();
        store.push_counter(&COUNTER_2.label().into(), &tags, Time::from_secs(i), COUNTER_2.read());
    }
    let plot10 = Plot::new("λ = 10", Query::Simple("waves.sin.10".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot60 = Plot::new("λ = 60", Query::Simple("waves.sin.60".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot1200 = Plot::new("λ = 1200", Query::Simple("waves.sin.1200".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot3600 = Plot::new("λ = 3600", Query::Simple("waves.sin.3600".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let panel1 = Panel::new("sin waves")
        .with_plot(plot10)
        .with_plot(plot60)
        .with_plot(plot1200)
        .with_plot(plot3600);
    let plot10 = Plot::new("λ = 10", Query::Simple("waves.cos.10".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot60 = Plot::new("λ = 60", Query::Simple("waves.cos.60".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot1200 = Plot::new("λ = 1200", Query::Simple("waves.cos.1200".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let plot3600 = Plot::new("λ = 3600", Query::Simple("waves.cos.3600".into(), tags.clone()))
        .with_y_min(0.0)
        .with_y_max(1.0);
    let panel2 = Panel::new("cos waves")
        .with_plot(plot10)
        .with_plot(plot60)
        .with_plot(plot1200)
        .with_plot(plot3600);
    let plot = Plot::new("zero", Query::Union(vec![Query::Simple("waves.sin.1200".into(), tags.clone()), Query::Simple("waves.neg_sin.1200".into(), tags.clone())]))
        .with_y_min(-1.0)
        .with_y_max(1.0);
    let panel3 = Panel::new("matched sin waves")
        .with_plot(plot);
    let dash = Dashboard::new("Periodic Functions")
        .with_panel(panel1)
        .with_panel(panel2)
        .with_panel(panel3)
        .with_label_width(20);
    let qe = QueryEngine::new()
        .with_biometrics_store(store);
    let mut rewindable = RewindWriter::from(std::io::stdout());
    for i in 0..8_640-360 {
        rewindable.rewind().unwrap();
        dash.plot(&qe, Window(Time::from_secs(i * 10), Time::from_secs(i * 10 + 3_600)), &mut rewindable).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
