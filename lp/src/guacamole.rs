use clap::{App, Arg, ArgMatches};

use rand::Rng;

use guacamole::strings;
use guacamole::Guac;
use guacamole::Guacamole;

use super::block::{Block, BlockBuilder, BlockCursor};
use super::reference::TableBuilder as ReferenceBuilder;
use super::table::{Table, TableBuilder, TableCursor};
use super::{Builder, Cursor, KeyValuePair};

/////////////////////////////////////////// KeyGuacamole ///////////////////////////////////////////

#[derive(Debug)]
pub struct KeyGuacamole {
    pub key: Box<dyn strings::StringGuacamole>,
}

impl Guac<String> for KeyGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> String {
        self.key.guacamole(guac)
    }
}

//////////////////////////////////////// TimestampGuacamole ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimestampGuacamole {}

impl Guac<u64> for TimestampGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> u64 {
        guac.gen()
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeyValuePut {
    pub key: String,
    pub timestamp: u64,
    pub value: String,
}

/////////////////////////////////////// KeyValuePutGuacamole ///////////////////////////////////////

#[derive(Debug)]
pub struct KeyValuePutGuacamole {
    pub key: KeyGuacamole,
    pub timestamp: TimestampGuacamole,
    pub value: Box<dyn strings::StringGuacamole>,
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
pub struct KeyValueDel {
    pub key: String,
    pub timestamp: u64,
}

/////////////////////////////////////// KeyValueDelGuacamole ///////////////////////////////////////

#[derive(Debug)]
pub struct KeyValueDelGuacamole {
    pub key: KeyGuacamole,
    pub timestamp: TimestampGuacamole,
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
pub enum KeyValueOperation {
    Put(KeyValuePut),
    Del(KeyValueDel),
}

//////////////////////////////////// KeyValueOperationGuacamole ////////////////////////////////////

#[derive(Debug)]
pub struct KeyValueOperationGuacamole {
    pub weight_put: f64,
    pub weight_del: f64,
    pub guacamole_put: KeyValuePutGuacamole,
    pub guacamole_del: KeyValueDelGuacamole,
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

///////////////////////////////////////////// arg_as_* /////////////////////////////////////////////

pub fn arg_as_u64(args: &ArgMatches, value: &str, default: &str) -> u64 {
    let value = args.value_of(value).unwrap_or(default);
    match value.parse::<u64>() {
        Ok(x) => x,
        Err(e) => {
            panic!("don't know how to parse \"{}\" as u64: {}", value, e);
        }
    }
}

pub fn arg_as_f64(args: &ArgMatches, value: &str, default: &str) -> f64 {
    let value = args.value_of(value).unwrap_or(default);
    match value.parse::<f64>() {
        Ok(x) => x,
        Err(e) => {
            panic!("don't know how to parse \"{}\" as f64: {}", value, e);
        }
    }
}

//////////////////////////////////////////////// App ///////////////////////////////////////////////

pub fn app(
    name: &'static str,
    version: &'static str,
    about: &'static str,
) -> App<'static, 'static> {
    let app = App::new(name).version(version).about(about);
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
    app
}

//////////////////////////////////////////// TableTrait ////////////////////////////////////////////

pub trait TableTrait<'a> {
    type Builder: TableBuilderTrait<'a, Table = Self>;
    type Cursor: Cursor;

    fn iterate(&self) -> Self::Cursor;
}

///////////////////////////////////////// TableBuilderTrait ////////////////////////////////////////

pub trait TableBuilderTrait<'a>: Builder<Sealed=Self::Table> {
    type Table: TableTrait<'a>;
}

//////////////////////////////////////////// Block impls ///////////////////////////////////////////

impl<'a> TableTrait<'a> for Block {
    type Builder = BlockBuilder;
    type Cursor = BlockCursor;

    fn iterate(&self) -> Self::Cursor {
        Block::iterate(self)
    }
}

impl<'a> TableBuilderTrait<'a> for BlockBuilder {
    type Table = Block;
}

//////////////////////////////////////////// Table impls ///////////////////////////////////////////

impl<'a> TableTrait<'a> for Table {
    type Builder = TableBuilder;
    type Cursor = TableCursor;

    fn iterate(&self) -> Self::Cursor {
        Table::iterate(self)
    }
}

impl<'a> TableBuilderTrait<'a> for TableBuilder {
    type Table = Table;
}

////////////////////////////////////////////// fuzzer //////////////////////////////////////////////

pub fn fuzzer<T, B, F>(name: &'static str, version: &'static str, about: &'static str, new_table: F)
where
    for<'a> T: TableTrait<'a>,
    for<'a> B: TableBuilderTrait<'a, Table = T>,
    F: Fn() -> B,
{
    let app = app(name, version, about);
    let args = app.get_matches();
    // Our workload generator.
    let key_bytes = arg_as_u64(&args, "key-bytes", "8") as usize;
    let value_bytes = arg_as_u64(&args, "value-bytes", "128") as usize;
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
    let mut builder = ReferenceBuilder::default();
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
    let num_seeks = arg_as_u64(&args, "num-seeks", "1000");
    let seek_distance = arg_as_u64(&args, "seek-distance", "10");
    let prev_probability = arg_as_f64(&args, "prev-probability", "0.01");
    println!("    fn test() {{");
    println!("        // --num-keys {}", num_keys);
    println!("        // --key-bytes {}", key_bytes);
    println!("        // --value-bytes {}", value_bytes);
    println!("        // --num-seeks {}", num_seeks);
    println!("        // --seek-distance {}", seek_distance);
    println!("        // --prev-probability {}", prev_probability);
    let mut builder = new_table();
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
                    std::str::from_utf8(x.key.as_bytes()).unwrap(),
                    x.timestamp,
                    std::str::from_utf8(v.as_bytes()).unwrap()
                );
                builder
                    .put(x.key.as_bytes(), x.timestamp, v.as_bytes())
                    .unwrap();
            }
            None => {
                println!(
                    "        builder.del(\"{}\".as_bytes(), {}).unwrap();",
                    std::str::from_utf8(x.key.as_bytes()).unwrap(),
                    x.timestamp
                );
                builder.del(x.key.as_bytes(), x.timestamp).unwrap();
            }
        };
    }
    println!("        let table = builder.seal().unwrap();");
    let table = builder.seal().unwrap();
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
        println!("        let mut cursor = table.iterate();");
        let mut cursor = table.iterate();
        println!(
            "        cursor.seek(\"{}\".as_bytes(), {}).unwrap();",
            key, ts
        );
        cursor.seek(key.as_bytes(), ts).unwrap();
        for _ in 0..seek_distance {
            let will_do_prev = guac.gen_range(0.0, 1.0) < prev_probability;
            let (exp, got) = if will_do_prev {
                let exp = iter.prev().unwrap();
                println!("        let got = cursor.prev().unwrap();");
                let got = cursor.prev().unwrap();
                (exp, got)
            } else {
                let exp = iter.next().unwrap();
                println!("        let got = cursor.next().unwrap();");
                let got = cursor.next().unwrap();
                (exp, got)
            };
            let print_x = |x: &KeyValuePair| {
                println!("        let exp = KeyValuePair {{");
                println!(
                    "            key: \"{}\".as_bytes().to_vec(),",
                    std::str::from_utf8(x.key.as_bytes()).unwrap()
                );
                println!("            timestamp: {},", x.timestamp);
                match &x.value {
                    Some(x) => {
                        println!(
                            "            value: Some(\"{}\".as_bytes().to_vec()),",
                            std::str::from_utf8(x.as_bytes()).unwrap()
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
                        println!("        assert_eq!(Some(exp), got);");
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

#[cfg(test)]
mod tests {
    use super::super::Error;
    use super::*;

    struct TestTable {}

    impl<'a> TableTrait<'a> for TestTable {
        type Builder = TestBuilder;
        type Cursor = TestCursor;

        fn iterate(&self) -> Self::Cursor {
            unimplemented!();
        }
    }

    struct TestBuilder {}

    impl<'a> TableBuilderTrait<'a> for TestBuilder {
        type Table = TestTable;
    }

    impl Builder for TestBuilder {
        type Sealed = TestTable;

        fn approximate_size(&self) -> usize {
            unimplemented!();
        }

        fn put(&mut self, _key: &[u8], _timestamp: u64, _value: &[u8]) -> Result<(), Error> {
            unimplemented!();
        }

        fn del(&mut self, _key: &[u8], _timestamp: u64) -> Result<(), Error> {
            unimplemented!();
        }

        fn seal(self) -> Result<TestTable, Error> {
            unimplemented!();
        }
    }

    struct TestCursor {}

    impl Cursor for TestCursor {
        fn seek_to_first(&mut self) -> Result<(), Error> {
            unimplemented!();
        }

        fn seek_to_last(&mut self) -> Result<(), Error> {
            unimplemented!();
        }

        fn seek(&mut self, _key: &[u8], _timestamp: u64) -> Result<(), Error> {
            unimplemented!();
        }

        fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }

        fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }

        fn same(&mut self) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }
    }
}
