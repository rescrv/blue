//! Prints test vectors; `python3 python/provsum.py` must print identical lines.
use provsum::{Digest, Site};

fn main() {
    let s = Site::from_label;
    let (a, b, c) = (s("a"), s("b"), s("c"));
    let (w1, w2, ch) = (s("w1"), s("w2"), s("choose"));
    let vectors = [
        ("origin_a", Digest::origin(&a)),
        ("wrap", Digest::origin(&a).wrap(&w1)),
        ("merge", Digest::origin(&a).merge(&Digest::origin(&b))),
        ("pick_ab", Digest::pick2(&ch, &Digest::origin(&a), &Digest::origin(&b))),
        ("pick_ba", Digest::pick2(&ch, &Digest::origin(&b), &Digest::origin(&a))),
        (
            "nested",
            Digest::pick2(
                &ch,
                &Digest::origin(&a).wrap(&w1).merge(&Digest::origin(&b).wrap(&w1)),
                &Digest::origin(&c).wrap(&w2),
            )
            .wrap(&w2),
        ),
    ];
    for (k, v) in vectors {
        println!("{k} {} {}", v.to_hex(), v.depth());
    }
}
