#![allow(clippy::excessive_precision)]
#![allow(clippy::approx_constant)]

use indicio::{clue, Collector, ALWAYS};

static TEST_COLLECTOR: Collector = Collector::new();

fn main() {
    clue!(TEST_COLLECTOR, ALWAYS, {
        hello: "world",
        consts: [
            2.71828182845904523536028747135266250_f64,
            3.14159265358979323846264338327950288_f64,
        ],
        recursive: {
            hello: "world",
            consts: [
                2.71828182845904523536028747135266250_f64,
                3.14159265358979323846264338327950288_f64,
            ],
        },
    });
}
