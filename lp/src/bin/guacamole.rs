use rand::distributions::{Alphanumeric, Distribution, Standard};
use rand::Rng;

use guacamole::Guac;
use guacamole::Guacamole;
use guacamole::strings;

//////////////////////////////////////////////// Key ///////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
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

#[derive(Clone, Debug, Default)]
struct TimestampGuacamole {
}

impl Guac<u64> for TimestampGuacamole {
    fn guacamole(&self, guac: &mut Guacamole) -> u64 {
        guac.gen()
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Default)]
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

#[derive(Clone, Debug, Default)]
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

#[derive(Debug)]
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
        let total = self.weight_put + self.weight_del;
        let pick: f64 = guac.gen();
        if pick < self.weight_put {
            KeyValueOperation::Put(self.guacamole_put.guacamole(guac))
        } else {
            KeyValueOperation::Del(self.guacamole_del.guacamole(guac))
        }
    }
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let mut guac = Guacamole::default();
    let gen = KeyValueOperationGuacamole {
        weight_put: 0.99,
        weight_del: 0.01,
        guacamole_put: KeyValuePutGuacamole {
            key:  KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength{ constant: 8 }),
                    select: Box::new(strings::RandomSelect{}),
                }),
            },
            timestamp:  TimestampGuacamole::default(),
            value:  Box::new(strings::IndependentStrings {
                length: Box::new(strings::ConstantLength{ constant: 128}),
                select: Box::new(strings::RandomSelect{}),
            }),
        },
        guacamole_del: KeyValueDelGuacamole {
            key:  KeyGuacamole {
                key: Box::new(strings::IndependentStrings {
                    length: Box::new(strings::ConstantLength{ constant: 8 }),
                    select: Box::new(strings::RandomSelect{}),
                }),
            },
            timestamp:  TimestampGuacamole::default(),
        }
    };
    loop {
        let s: KeyValueOperation = gen.guacamole(&mut guac);
        println!("{:?}", s);
    }
}
