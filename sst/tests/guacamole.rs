extern crate sst;

use rand::{Rng, RngCore};

use guacamole::Guacamole;

use sst::block::{Block, BlockBuilder, BlockCursor};
use sst::reference::ReferenceBuilder;
use sst::{Builder, Cursor, SstBuilder, SstCursor, Sst};

////////////////////////////////////////// BufferGuacamole /////////////////////////////////////////

#[derive(Debug)]
pub struct BufferGuacamole {
    pub sz: usize,
}

impl BufferGuacamole {
    fn new(sz: usize) -> Self {
        Self { sz }
    }

    fn guacamole(&self, guac: &mut Guacamole) -> Vec<u8> {
        let mut buf = vec![0u8; self.sz];
        guac.fill_bytes(&mut buf);
        buf
    }
}

//////////////////////////////////////// TimestampGuacamole ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimestampGuacamole {}

impl TimestampGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> u64 {
        guac.gen()
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeyValuePut {
    pub key: Vec<u8>,
    pub timestamp: u64,
    pub value: Vec<u8>,
}

/////////////////////////////////////// KeyValuePutGuacamole ///////////////////////////////////////

pub struct KeyValuePutGuacamole {
    pub key: BufferGuacamole,
    pub timestamp: TimestampGuacamole,
    pub value: BufferGuacamole,
}

impl KeyValuePutGuacamole {
    fn guacamole(&mut self, guac: &mut Guacamole) -> KeyValuePut {
        KeyValuePut {
            key: self.key.guacamole(guac),
            timestamp: self.timestamp.guacamole(guac),
            value: self.value.guacamole(guac),
        }
    }
}

//////////////////////////////////////////// KeyValueDel ///////////////////////////////////////////

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeyValueDel {
    pub key: Vec<u8>,
    pub timestamp: u64,
}

/////////////////////////////////////// KeyValueDelGuacamole ///////////////////////////////////////

pub struct KeyValueDelGuacamole {
    pub key: BufferGuacamole,
    pub timestamp: TimestampGuacamole,
}

impl KeyValueDelGuacamole {
    fn guacamole(&mut self, guac: &mut Guacamole) -> KeyValueDel {
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

pub struct KeyValueOperationGuacamole {
    pub weight_put: f64,
    pub weight_del: f64,
    pub guacamole_put: KeyValuePutGuacamole,
    pub guacamole_del: KeyValueDelGuacamole,
}

impl KeyValueOperationGuacamole {
    fn guacamole(&mut self, guac: &mut Guacamole) -> KeyValueOperation {
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

//////////////////////////////////////////// TableTrait ////////////////////////////////////////////

pub trait TableTrait<'a> {
    type Builder: TableBuilderTrait<'a, Table = Self>;
    type Cursor: Cursor;

    fn cursor(&self) -> Self::Cursor;
}

///////////////////////////////////////// TableBuilderTrait ////////////////////////////////////////

pub trait TableBuilderTrait<'a>: Builder<Sealed = Self::Table> {
    type Table: TableTrait<'a>;
}

//////////////////////////////////////////// Block impls ///////////////////////////////////////////

impl<'a> TableTrait<'a> for Block {
    type Builder = BlockBuilder;
    type Cursor = BlockCursor;

    fn cursor(&self) -> Self::Cursor {
        Block::cursor(self)
    }
}

impl<'a> TableBuilderTrait<'a> for BlockBuilder {
    type Table = Block;
}

///////////////////////////////////////////// Sst impls ////////////////////////////////////////////

impl<'a> TableTrait<'a> for Sst {
    type Builder = SstBuilder;
    type Cursor = SstCursor;

    fn cursor(&self) -> Self::Cursor {
        Sst::cursor(self)
    }
}

impl<'a> TableBuilderTrait<'a> for SstBuilder {
    type Table = Sst;
}

/////////////////////////////////////////// FuzzerConfig ///////////////////////////////////////////

pub struct FuzzerConfig {
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub num_keys: u64,
    pub num_seeks: u64,
    pub seek_distance: u64,
    pub prev_probability: f64,
}

impl Default for FuzzerConfig {
    fn default() -> Self {
        Self {
            key_bytes: 8,
            value_bytes: 128,
            num_keys: 1000,
            num_seeks: 1000,
            seek_distance: 10,
            prev_probability: 0.01,
        }
    }
}

////////////////////////////////////////////// fuzzer //////////////////////////////////////////////

pub fn fuzzer<T, B, F>(name: &str, config: FuzzerConfig, new_table: F)
where
    for<'a> T: TableTrait<'a>,
    for<'a> B: TableBuilderTrait<'a, Table = T>,
    F: Fn(&str) -> B,
{
    // Our workload generator.
    let mut guac = Guacamole::default();
    let mut gen = KeyValueOperationGuacamole {
        weight_put: 0.99,
        weight_del: 0.01,
        guacamole_put: KeyValuePutGuacamole {
            key: BufferGuacamole::new(config.key_bytes),
            timestamp: TimestampGuacamole::default(),
            value: BufferGuacamole::new(config.value_bytes),
        },
        guacamole_del: KeyValueDelGuacamole {
            key: BufferGuacamole::new(config.key_bytes),
            timestamp: TimestampGuacamole::default(),
        },
    };
    // Load up a minimal key-value store.
    let mut builder = ReferenceBuilder::default();
    for _ in 0..config.num_keys {
        let kvo: KeyValueOperation = gen.guacamole(&mut guac);
        match kvo {
            KeyValueOperation::Put(x) => {
                builder
                    .put(&x.key, x.timestamp, &x.value)
                    .unwrap();
            }
            KeyValueOperation::Del(x) => {
                builder.del(&x.key, x.timestamp).unwrap();
            }
        }
    }
    let kvs = builder.seal().unwrap();
    // Create a new builder using the keys in the key-value store.
    let mut builder = new_table(name);
    let mut ref_cursor = kvs.cursor();
    loop {
        ref_cursor.next().unwrap();
        let x = ref_cursor.value();
        if x.is_none() {
            break;
        }
        let x = x.unwrap();
        match x.value {
            Some(ref v) => {
                builder.put(x.key, x.timestamp, v).unwrap();
            }
            None => {
                builder.del(x.key, x.timestamp).unwrap();
            }
        };
    }
    let table = builder.seal().unwrap();
    // Now seek randomly and compare the key-value store and the builder.
    let key_gen = BufferGuacamole::new(config.key_bytes);
    for _ in 0..config.num_seeks {
        let key: Vec<u8> = key_gen.guacamole(&mut guac);
        ref_cursor.seek(&key).unwrap();
        let mut cursor = table.cursor();
        cursor.seek(&key).unwrap();
        for _ in 0..config.seek_distance {
            let will_do_prev = guac.gen_range(0.0, 1.0) < config.prev_probability;
            let (exp, got) = if will_do_prev {
                ref_cursor.prev().unwrap();
                cursor.prev().unwrap();
                let exp = ref_cursor.value();
                let got = cursor.value();
                (exp, got)
            } else {
                ref_cursor.next().unwrap();
                cursor.next().unwrap();
                let exp = ref_cursor.value();
                let got = cursor.value();
                (exp, got)
            };
            match (exp, got) {
                (Some(x), Some(y)) => {
                    assert_eq!(x, y);
                }
                (None, None) => break,
                (None, Some(x)) => {
                    panic!("found bad case (open a debugger or print out a dump of info above); got: {:?}", x);
                }
                (Some(x), None) => {
                    panic!("found bad case (open a debugger or print out a dump of info above): exp: {:?}", x);
                }
            }
        }
    }
}

////////////////////////////////////////// guacamole_tests /////////////////////////////////////////

#[macro_export]
macro_rules! guacamole_tests {
    ($($name:ident: $builder:expr,)*) => {
    $(
        #[cfg(test)]
        mod $name {
            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_1_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_1_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 1,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_256_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_256_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 256,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_4096_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 4096,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10_key_bytes_16384_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10_key_bytes_16384_value_bytes_32768_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10,
                    key_bytes: 16384,
                    value_bytes: 32768,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_1_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 1,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_16_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 16,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_0_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 0,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_1_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 1,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_16_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 16,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_0";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.0,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

            #[test]
            fn num_keys_10000_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125() {
                let name = stringify!($name).to_string() + "::" + "num_keys_10000_key_bytes_256_value_bytes_256_num_seeks_1000_seek_distance_10_prev_probability_0_125";
                let config = crate::guacamole::FuzzerConfig {
                    num_keys: 10000,
                    key_bytes: 256,
                    value_bytes: 256,
                    num_seeks: 1000,
                    seek_distance: 10,
                    prev_probability: 0.125,
                };
                crate::guacamole::fuzzer(&name, config, $builder);
            }

        }
    )*
    }
}
