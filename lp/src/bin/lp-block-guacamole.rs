use clap::{App, Arg, ArgMatches};

use rand::Rng;

use guacamole::strings;
use guacamole::Guac;
use guacamole::Guacamole;

use lp::block::{Builder, BuilderOptions};
use lp::reference::TableBuilder;
use lp::KeyValuePair;
use lp::Table as TableTrait;
use lp::TableBuilder as TableBuilderTrait;
use lp::TableCursor as TableCursorTrait;

/////////////////////////////////////////// KeyGuacamole ///////////////////////////////////////////

#[derive(Debug)]
struct KeyGuacamole {
    key: Box<dyn strings::StringGuacamole>,
}

impl Guac<String> for KeyGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> String {
        self.key.guacamole(guac)
    }
}

//////////////////////////////////////// TimestampGuacamole ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct TimestampGuacamole {}

impl Guac<u64> for TimestampGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> u64 {
        guac.gen()
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct KeyValuePut {
    key: String,
    timestamp: u64,
    value: String,
}

/////////////////////////////////////// KeyValuePutGuacamole ///////////////////////////////////////

#[derive(Debug)]
struct KeyValuePutGuacamole {
    key: KeyGuacamole,
    timestamp: TimestampGuacamole,
    value: Box<dyn strings::StringGuacamole>,
}

impl Guac<KeyValuePut> for KeyValuePutGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> KeyValuePut {
        KeyValuePut {
            key: self.key.guacamole(guac),
            timestamp: self.timestamp.guacamole(guac),
            value: self.value.guacamole(guac),
        }
    }
}

//////////////////////////////////////////// KeyValueDel ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct KeyValueDel {
    key: String,
    timestamp: u64,
}

/////////////////////////////////////// KeyValueDelGuacamole ///////////////////////////////////////

#[derive(Debug)]
struct KeyValueDelGuacamole {
    key: KeyGuacamole,
    timestamp: TimestampGuacamole,
}

impl Guac<KeyValueDel> for KeyValueDelGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> KeyValueDel {
        KeyValueDel {
            key: self.key.guacamole(guac),
            timestamp: self.timestamp.guacamole(guac),
        }
    }
}

///////////////////////////////////////// KeyValueOperation ////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
enum KeyValueOperation {
    Put(KeyValuePut),
    Del(KeyValueDel),
}

//////////////////////////////////// KeyValueOperationGuacamole ////////////////////////////////////

#[derive(Debug)]
struct KeyValueOperationGuacamole {
    weight_put: f64,
    weight_del: f64,
    guacamole_put: KeyValuePutGuacamole,
    guacamole_del: KeyValueDelGuacamole,
}

impl Guac<KeyValueOperation> for KeyValueOperationGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> KeyValueOperation {
        let pick: f64 = guac.gen();
        if pick <= self.weight_put {
            KeyValueOperation::Put(self.guacamole_put.guacamole(guac))
        } else if pick <= self.weight_put + self.weight_del {
            KeyValueOperation::Del(self.guacamole_del.guacamole(guac))
        } else {
            panic!("infinite improbability drive");
        }
    }
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn arg_as_u64(args: &ArgMatches, value: &str, default: &str) -> u64 {
    let value = args.value_of(value).unwrap_or(default);
    match value.parse::<u64>() {
        Ok(x) => x,
        Err(e) => {
            panic!("don't know how to parse \"{}\" as u64: {}", value, e);
        }
    }
}

fn arg_as_f64(args: &ArgMatches, value: &str, default: &str) -> f64 {
    let value = args.value_of(value).unwrap_or(default);
    match value.parse::<f64>() {
        Ok(x) => x,
        Err(e) => {
            panic!("don't know how to parse \"{}\" as f64: {}", value, e);
        }
    }
}

fn main() {
    let app = App::new("lp-block-guacamole")
        .version("0.1")
        .about("Runs random workloads against lp-block.");
    let app = app.arg(
        Arg::with_name("num-keys")
            .long("num-keys")
            .takes_value(true)
            .help("Number of keys to load into reference key-value store."),
    );
    let app = app.arg(
        Arg::with_name("key-bytes")
            .long("key-bytes")
            .takes_value(true)
            .help("Number of bytes to generate per key."),
    );
    let app = app.arg(
        Arg::with_name("value-bytes")
            .long("value-bytes")
            .takes_value(true)
            .help("Number of bytes to generate per value."),
    );
    let app = app.arg(
        Arg::with_name("num-seeks")
            .long("num-seeks")
            .takes_value(true)
            .help("Number of keys to scan from seek position."),
    );
    let app = app.arg(
        Arg::with_name("seek-distance")
            .long("seek-distance")
            .takes_value(true)
            .help("Number of keys to scan from seek position."),
    );
    let app = app.arg(
        Arg::with_name("prev-probability")
            .long("prev-probability")
            .takes_value(true)
            .help("Probability of calling \"prev\" on a cursor instead of \"next\"."),
    );
    let args = app.get_matches();
    // Our workload generator.
    let key_bytes = arg_as_u64(&args, "key-bytes", "8") as usize;
    let value_bytes = arg_as_u64(&args, "value-bytes", "128") as usize;
    let prev_probability = arg_as_f64(&args, "prev-probability", "0.01") as f64;
    let mut guac = Guacamole::default();
    let gen = KeyValueOperationGuacamole {
        weight_put: 0.99,
        weight_del: 0.01,
        guacamole_put: KeyValuePutGuacamole {
            key: KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength {
                        constant: key_bytes,
                    }),
                    select: Box::new(strings::RandomSelect {}),
                }),
            },
            timestamp: TimestampGuacamole::default(),
            value: Box::new(strings::IndependentStrings {
                length: Box::new(strings::ConstantLength {
                    constant: value_bytes,
                }),
                select: Box::new(strings::RandomSelect {}),
            }),
        },
        guacamole_del: KeyValueDelGuacamole {
            key: KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength {
                        constant: key_bytes,
                    }),
                    select: Box::new(strings::RandomSelect {}),
                }),
            },
            timestamp: TimestampGuacamole::default(),
        },
    };
    // Load up a minimal key-value store.
    let num_keys = arg_as_u64(&args, "num-keys", "1000");
    let mut builder = TableBuilder::default();
    for _ in 0..num_keys {
        let kvo: KeyValueOperation = gen.guacamole(&mut guac);
        match kvo {
            KeyValueOperation::Put(x) => {
                builder
                    .put(x.key.as_bytes(), x.timestamp, x.value.as_bytes())
                    .unwrap();
            }
            KeyValueOperation::Del(x) => {
                builder.del(x.key.as_bytes(), x.timestamp).unwrap();
            }
        }
    }
    let kvs = builder.seal().unwrap();
    // Create a new builder using the keys in the key-value store.
    let builder_opts = BuilderOptions {
        bytes_restart_interval: 512,
        key_value_pairs_restart_interval: 16,
    };
    let mut builder = Builder::new(builder_opts);
    let num_seeks = arg_as_u64(&args, "num-seeks", "1000");
    let seek_distance = arg_as_u64(&args, "seek-distance", "10");
    println!("    fn test() {{");
    println!("        // --num-keys {}", num_keys);
    println!("        // --key-bytes {}", key_bytes);
    println!("        // --value-bytes {}", value_bytes);
    println!("        // --num-seeks {}", num_seeks);
    println!("        // --seek-distance {}", seek_distance);
    println!("        let builder_opts = BuilderOptions {{");
    println!("            bytes_restart_interval: 512,");
    println!("            key_value_pairs_restart_interval: 16,");
    println!("        }};");
    println!("        let mut builder = Builder::new(builder_opts);");
    let mut iter = kvs.iterate();
    loop {
        let x = iter.next().unwrap();
        if x.is_none() {
            break;
        }
        let x = x.unwrap();
        match x.value {
            Some(ref v) => {
                println!(
                    "        builder.put(\"{}\".as_bytes(), {}, \"{}\".as_bytes()).unwrap();",
                    std::str::from_utf8(x.key).unwrap(),
                    x.timestamp,
                    std::str::from_utf8(v).unwrap()
                );
                builder.put(x.key, x.timestamp, v).unwrap();
            }
            None => {
                println!(
                    "        builder.del(\"{}\".as_bytes(), {}).unwrap();",
                    std::str::from_utf8(x.key).unwrap(),
                    x.timestamp
                );
                builder.del(x.key, x.timestamp).unwrap();
            }
        };
    }
    println!("        let block = builder.seal().unwrap();");
    let block = builder.seal().unwrap();
    // Now seek randomly and compare the key-value store and the builder.
    let key_gen = KeyGuacamole {
        key: Box::new(strings::IndependentStrings {
            length: Box::new(strings::ConstantLength {
                constant: key_bytes,
            }),
            select: Box::new(strings::RandomSelect {}),
        }),
    };
    let ts_gen = TimestampGuacamole {};
    for _ in 0..num_seeks {
        let key: String = key_gen.guacamole(&mut guac);
        let ts: u64 = ts_gen.guacamole(&mut guac);
        println!("        // Top of loop seeks to: {:?}@{}", key, ts);
        iter.seek(key.as_bytes(), ts).unwrap();
        println!("        let mut cursor = block.iterate();");
        let mut cursor = block.iterate();
        println!(
            "        cursor.seek(\"{}\".as_bytes(), {}).unwrap();",
            key, ts
        );
        cursor.seek(key.as_bytes(), ts).unwrap();
        for _ in 0..seek_distance {
            let exp = iter.next().unwrap();
            println!("        let got = cursor.next().unwrap();");
            let got = cursor.next().unwrap();
            let print_x = |x: &KeyValuePair| {
                println!("        let exp = KeyValuePair {{");
                println!(
                    "            key: \"{}\".as_bytes(),",
                    std::str::from_utf8(x.key).unwrap()
                );
                println!("            timestamp: {},", x.timestamp);
                match x.value {
                    Some(x) => {
                        println!(
                            "            value: Some(\"{}\".as_bytes()),",
                            std::str::from_utf8(x).unwrap()
                        );
                    }
                    None => {
                        println!("            value: None,");
                    }
                };
                println!("        }};");
            };
            match (exp, got) {
                (Some(x), Some(y)) => {
                    if x != y {
                        print_x(&x);
                        println!("        assert_eq!(exp, got);");
                        println!("    }}");
                    }
                    assert_eq!(x, y);
                }
                (None, None) => break,
                (None, Some(x)) => {
                    println!("        assert_eq!(None, got);");
                    println!("    }}");
                    panic!("found bad case (open a debugger or print out a dump of info above); got: {:?}", x);
                }
                (Some(x), None) => {
                    print_x(&x);
                    println!("        assert_eq!(exp, got);");
                    println!("    }}");
                    panic!("found bad case (open a debugger or print out a dump of info above)");
                }
            }
        }
    }
    println!("    }}");
}
