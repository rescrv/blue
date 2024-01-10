statslicer
==========

Statslicer is a platform for running experiments and collecting their results.

```
use guacamole::combinators::*;
use guacamole::Guacamole;

use statslicer::{benchmark, black_box, statslicer_main, Bencher, Parameter, Parameters};

#[derive(Debug, Default, Eq, PartialEq)]
struct MyParameters {
    elements: usize,
}

impl Parameters for MyParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("elements", Parameter::Integer(self.elements as u64)),
        ]
    }
}

fn bench_sort(params: &MyParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let array = to_vec(constant(params.elements), any::<u64>)(&mut guac);
    let bin_searches = to_vec(constant(b.size()), any::<u64>)(&mut guac);
    b.run(|| {
        for needle in bin_searches.iter() {
            let _ = black_box(array.binary_search(needle));
        }
    });
}

benchmark! {
    name = my_sort_benchmark;
    MyParameters {
        elements in [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024],
    }
    bench_sort
}

statslicer_main! { my_sort_benchmark }
```

Status
------

Experimental.  Likely to change in the near future.

Scope
-----

This library provides the statslicer benchmark tools and a binary to derive data and colate histograms.

Warts
-----

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/statslicer/latest/statslicer/).
