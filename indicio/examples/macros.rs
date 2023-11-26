use std::fmt::{Display, Formatter};

use indicio::{clue, Collector, PrintlnEmitter};

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct ExampleKey {
    #[prototk(1, string)]
    first_key_field: String,
    #[prototk(2, uint64)]
    second_key_field: u64,
}

impl Display for ExampleKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct ExampleValue {
    #[prototk(1, string)]
    first_value_field: String,
    #[prototk(2, uint64)]
    second_value_field: u64,
}

impl Display for ExampleValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

static EXAMPLE_COLLECTOR: Collector<ExampleKey, ExampleValue> = Collector::new();

fn main() {
    EXAMPLE_COLLECTOR.register(PrintlnEmitter::new());
    clue! { EXAMPLE_COLLECTOR, ExampleKey {
            first_key_field: "my key".to_string(),
            second_key_field: 42,
        } => ExampleValue {
            first_value_field: "hello world".to_string(),
            second_value_field: u64::MAX,
        }
    };
}
