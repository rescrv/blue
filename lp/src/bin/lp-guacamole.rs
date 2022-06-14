use std::collections::btree_map::BTreeMap;
use std::ops::Bound;

use clap::{Arg, ArgMatches, App};

use rand::Rng;

use guacamole::Guac;
use guacamole::Guacamole;
use guacamole::strings;

use lp::{KeyValuePair,Iterator};
use lp::block::{Block,Builder,BuilderOptions,Cursor};

//////////////////////////////////////////////// Key ///////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    key: String,
}

/////////////////////////////////////////// KeyGuacamole ///////////////////////////////////////////

#[derive(Debug)]
struct KeyGuacamole {
    key: Box<dyn strings::StringGuacamole>,
}

impl Guac<Key> for KeyGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> Key {
        Key {
            key: self.key.guacamole(guac)
        }
    }
}

//////////////////////////////////////// TimestampGuacamole ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct TimestampGuacamole {
}

impl Guac<u64> for TimestampGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> u64 {
        guac.gen()
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct KeyValuePut {
    key: Key,
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
    key: Key,
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

impl KeyValueOperation {
    fn to_key_value_pair(&self) -> lp::KeyValuePair {
        let (key, timestamp, value) = match self {
            KeyValueOperation::Put(x) => { (x.key.key.as_bytes(), x.timestamp, Some(x.value.as_bytes())) },
            KeyValueOperation::Del(x) => { (x.key.key.as_bytes(), x.timestamp, None) },
        };
        lp::KeyValuePair {
            key,
            timestamp,
            value,
        }
    }
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

////////////////////////////////////// ReferenceKeyValueStore //////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord)]
struct ReferenceKey {
    key: Key,
    timestamp: u64,
}

impl PartialOrd for ReferenceKey {
    fn partial_cmp(&self, rhs: &ReferenceKey) -> Option<std::cmp::Ordering> {
        let key1 = self.key.key.as_bytes();
        let key2 = rhs.key.key.as_bytes();
        Some(key1.cmp(key2)
            .then(self.timestamp.cmp(&rhs.timestamp).reverse()))
    }
}

#[derive(Clone, Default)]
struct ReferenceKeyValueStore {
    map: BTreeMap<ReferenceKey, KeyValueOperation>,
}

impl ReferenceKeyValueStore {
    fn op(&mut self, what: KeyValueOperation) {
        let key_ts = match what {
            KeyValueOperation::Put(ref x) => { (x.key.clone(), x.timestamp) },
            KeyValueOperation::Del(ref x) => { (x.key.clone(), x.timestamp) },
        };
        let key = ReferenceKey {
            key: key_ts.0,
            timestamp: key_ts.1,
        };
        self.map.insert(key, what);
    }

    fn seek(&mut self, what: Key) -> impl std::iter::Iterator<Item=KeyValueOperation> + '_ {
        let key = ReferenceKey {
            key: what,
            timestamp: u64::max_value(),
        };
        self.map.range((Bound::Included(key), Bound::Unbounded)).map(|x| x.1.clone())
    }
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn arg_as_u64(args: &ArgMatches, value: &str, default: &str) -> u64 {
    let value = args.value_of(value).unwrap_or(default);
    match value.parse::<u64>() {
        Ok(x) => x,
        Err(e) => {
            panic!("don't know how to parse \"{}\": {}", value, e);
        },
    }
}

fn main() {
    let app = App::new("lp-guacamole")
                      .version("0.1")
                      .about("Runs random workloads against lp.");
    let app = app.arg(Arg::with_name("num-keys")
                      .long("num-keys")
                      .takes_value(true)
                      .help("Number of keys to load into reference key-value store."));
    let app = app.arg(Arg::with_name("key-bytes")
                      .long("key-bytes")
                      .takes_value(true)
                      .help("Number of bytes to generate per key."));
    let app = app.arg(Arg::with_name("value-bytes")
                      .long("value-bytes")
                      .takes_value(true)
                      .help("Number of bytes to generate per value."));
    let app = app.arg(Arg::with_name("num-seeks")
                      .long("num-seeks")
                      .takes_value(true)
                      .help("Number of keys to scan from seek position."));
    let app = app.arg(Arg::with_name("seek-distance")
                      .long("seek-distance")
                      .takes_value(true)
                      .help("Number of keys to scan from seek position."));
    let args = app.get_matches();
    // Our workload generator
    let key_bytes = arg_as_u64(&args, "key-bytes", "8") as usize;
    let value_bytes = arg_as_u64(&args, "value-bytes", "128") as usize;
    let mut guac = Guacamole::default();
    let gen = KeyValueOperationGuacamole {
        weight_put: 0.99,
        weight_del: 0.01,
        guacamole_put: KeyValuePutGuacamole {
            key:  KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength{ constant: key_bytes }),
                    select: Box::new(strings::RandomSelect{}),
                }),
            },
            timestamp:  TimestampGuacamole::default(),
            value:  Box::new(strings::IndependentStrings {
                length: Box::new(strings::ConstantLength{ constant: value_bytes }),
                select: Box::new(strings::RandomSelect{}),
            }),
        },
        guacamole_del: KeyValueDelGuacamole {
            key:  KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength{ constant: key_bytes }),
                    select: Box::new(strings::RandomSelect{}),
                }),
            },
            timestamp:  TimestampGuacamole::default(),
        }
    };
    // Load up a minimal key-value store.
    let num_keys = arg_as_u64(&args, "num-keys", "1000");
    let mut kvs = ReferenceKeyValueStore::default();
    for _ in 0..num_keys {
        let kvo: KeyValueOperation = gen.guacamole(&mut guac);
        kvs.op(kvo);
    }
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
    for v in kvs.seek(Key::default()) {
        match v {
            KeyValueOperation::Put(x) => {
                println!("        builder.put(\"{}\".as_bytes(), {}, \"{}\".as_bytes());", &x.key.key, x.timestamp, &x.value);
                builder.put(&x.key.key.as_bytes(), x.timestamp, &x.value.as_bytes());
            },
            KeyValueOperation::Del(x) => {
                println!("        builder.del(\"{}\".as_bytes(), {});", &x.key.key, x.timestamp);
                builder.del(&x.key.key.as_bytes(), x.timestamp);
            }
        }
    }
    println!("        let finisher = builder.finish();");
    let finisher = builder.finish();
    println!("        let block = Block::new(finisher.as_slice()).unwrap();");
    let block = Block::new(finisher.as_slice()).unwrap();
    // Now seek randomly and compare the key-value store and the builder.
    let key_gen = KeyGuacamole {
        key: Box::new(strings::IndependentStrings {
            length: Box::new(strings::ConstantLength{ constant: key_bytes }),
            select: Box::new(strings::RandomSelect{}),
        }),
    };
    for _ in 0..num_seeks {
        let key: Key = key_gen.guacamole(&mut guac);
        println!("        // Top of loop seeks to: {:?}", key);
        let mut iter = kvs.seek(key.clone());
        println!("        let mut cursor = Cursor::new(&block);");
        let mut cursor = Cursor::new(&block);
        println!("        cursor.seek(\"{}\".as_bytes()).unwrap();", key.key);
        cursor.seek(key.key.as_bytes()).unwrap();
        for _ in 0..seek_distance {
            let exp = iter.next();
            println!("        let got = cursor.next().unwrap();");
            let got = cursor.next().unwrap();
            let print_x = |x: &KeyValuePair| {
                println!("        let exp = KeyValuePair {{");
                println!("            key: \"{}\".as_bytes(),", std::str::from_utf8(x.key).unwrap());
                println!("            timestamp: {},", x.timestamp);
                match x.value {
                    Some(x) => { println!("            value: Some(\"{}\".as_bytes()),", std::str::from_utf8(x).unwrap()); }
                    None => { println!("            value: None,"); }
                };
                println!("        }};");
            };
            match (exp, got) {
                (Some(x), Some(y)) => {
                    if x.to_key_value_pair() != y {
                        print_x(&x.to_key_value_pair());
                        println!("        assert_eq!(Some(exp), got);");
                        println!("    }}");
                    }
                    assert_eq!(x.to_key_value_pair(), y);
                }
                (None, None) => {
                    break
                },
                (None, Some(x)) => {
                    println!("        assert_eq!(None, got);");
                    println!("    }}");
                    panic!("found bad case (open a debugger or print out a dump of info above); got: {:?}", x);
                },
                (Some(x), None) => {
                    print_x(&x.to_key_value_pair());
                    println!("        assert_eq!(Some(exp), got);");
                    println!("    }}");
                    panic!("found bad case (open a debugger or print out a dump of info above)");
                },
            }
        }
    }
    println!("    }}");
}
