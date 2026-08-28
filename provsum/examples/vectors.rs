//! Prints test vectors; `python3 python/provsum.py` must print identical lines.
use provsum::{Digest, Site};

fn main() {
    let s = Site::from_label;
    let (a, b, c) = (s("a"), s("b"), s("c"));
    let (w1, w2, ch) = (s("w1"), s("w2"), s("choose"));
    let raw = Site([
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ]);
    let vectors = [
        ("origin_raw_bytes", Digest::origin(&raw)),
        ("origin_a", Digest::origin(&a)),
        ("wrap", Digest::origin(&a).wrap(&w1)),
        ("merge", Digest::origin(&a).merge(&Digest::origin(&b))),
        (
            "pick_ab",
            Digest::pick2(&ch, &Digest::origin(&a), &Digest::origin(&b)),
        ),
        (
            "pick_ba",
            Digest::pick2(&ch, &Digest::origin(&b), &Digest::origin(&a)),
        ),
        (
            "nested",
            Digest::pick2(
                &ch,
                &Digest::origin(&a)
                    .wrap(&w1)
                    .merge(&Digest::origin(&b).wrap(&w1)),
                &Digest::origin(&c).wrap(&w2),
            )
            .wrap(&w2),
        ),
    ];
    for (k, v) in vectors {
        println!("{k} {} {}", v.to_hex(), v.depth());
    }
}
