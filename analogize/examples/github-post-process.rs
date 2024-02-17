use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::str::FromStr;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde_json::Value;

fn extract_timestamp(field: &str, value: &Value) -> Option<DateTime<Utc>> {
    let v = value.get(field);
    if let Some(Value::String(timestamp)) = &v {
        Some(DateTime::parse_from_rfc3339(timestamp).ok()?.to_utc())
    } else if let Some(Value::Number(timestamp)) = &v {
        DateTime::from_timestamp(timestamp.as_i64()?, 0)
    } else {
        None
    }
}

fn main() {
    let mut seen = HashSet::new();
    let mut events = vec![];
    let args: Vec<String> = std::env::args().collect();
    for arg in args[1..].iter() {
        let reader = BufReader::new(File::open(arg).expect("file should open"));
        for line in reader.lines() {
            let line = line.expect("line should unwrap");
            let value: Value = serde_json::from_str(&line).expect("json should deserialize");
            let mut append = false;
            let id = value.get("id");
            if let Some(Value::String(id)) = id {
                let id = u64::from_str(&id).unwrap();
                if !seen.contains(&id) {
                    seen.insert(id);
                    append = true;
                }
            } else if let Some(Value::Number(id)) = id {
                let id = id.as_u64().unwrap();
                if !seen.contains(&id) {
                    seen.insert(id);
                    append = true;
                }
            } else {
                panic!("could not parse ID");
            }
            if append {
                let mut buf = vec![];
                serde_json::to_writer(&mut buf, &value).expect("serde should write");
                let json = String::from_utf8(buf).expect("json should be valid utf8");
                let timestamp =
                    extract_timestamp("created_at", &value).expect("timestamp should extract");
                events.push((timestamp, json));
            }
        }
    }
    events.sort_by_key(|x| x.0);
    let mut window = None;
    let mut output = None;
    for (timestamp, event) in events {
        let this = timestamp.duration_trunc(TimeDelta::minutes(5)).unwrap();
        if window.is_none() || window.unwrap() < this {
            window = Some(this);
            let when = this.to_rfc3339_opts(chrono::format::SecondsFormat::Secs, true);
            output = Some(
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(format!("{}.json", when))
                    .unwrap(),
            );
        }
        writeln!(output.as_ref().unwrap(), "{}", event).unwrap();
    }
}
