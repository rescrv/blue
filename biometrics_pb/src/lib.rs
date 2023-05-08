use prototk_derive::Message;

////////////////////////////////////////////// Counter /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Counter {
    #[prototk(1, uint64)]
    pub count: u64,
}

impl From<u64> for Counter {
    fn from(count: u64) -> Self {
        Self {
            count,
        }
    }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Gauge {
    #[prototk(2, double)]
    pub value: f64,
}

impl From<f64> for Gauge {
    fn from(value: f64) -> Self {
        Self {
            value,
        }
    }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct Moments {
    #[prototk(3, uint64)]
    pub n: u64,
    #[prototk(4, double)]
    pub m1: f64,
    #[prototk(5, double)]
    pub m2: f64,
    #[prototk(6, double)]
    pub m3: f64,
    #[prototk(7, double)]
    pub m4: f64,
}

impl From<biometrics::moments::Moments> for Moments {
    fn from(m: biometrics::moments::Moments) -> Self {
        Self {
            n: m.n,
            m1: m.m1,
            m2: m.m2,
            m3: m.m3,
            m4: m.m4,
        }
    }
}
