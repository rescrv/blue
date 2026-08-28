//! Pinned vectors shared with python/provsum.py.  If these change, the wire
//! format changed and every stored digest is invalidated.
use provsum::{Digest, Site};

#[test]
fn pinned_vectors() {
    let s = Site::from_label;
    let (a, b, c) = (s("a"), s("b"), s("c"));
    let (w1, w2, ch) = (s("w1"), s("w2"), s("choose"));
    let cases: [(&str, Digest, &str, u16); 6] = [
        (
            "origin_a",
            Digest::origin(&a),
            "0c5b01102d3aa5381bbbf5c617af80f9",
            1,
        ),
        (
            "wrap",
            Digest::origin(&a).wrap(&w1),
            "0e44a29fd575961e1efdb24d645c699a",
            2,
        ),
        (
            "merge",
            Digest::origin(&a).merge(&Digest::origin(&b)),
            "0be2208396b0ddb91a91f5252af36ed8",
            1,
        ),
        (
            "pick_ab",
            Digest::pick2(&ch, &Digest::origin(&a), &Digest::origin(&b)),
            "051ad0515135f32600c1f2c5601f2f74",
            2,
        ),
        (
            "pick_ba",
            Digest::pick2(&ch, &Digest::origin(&b), &Digest::origin(&a)),
            "19c61ed8dd4a36b41520984763516d0a",
            2,
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
            "1cd542f9fa81fc621d99cc3e93d019f7",
            4,
        ),
    ];
    for (name, d, hex, depth) in cases {
        assert_eq!(d.to_hex(), hex, "{name}");
        assert_eq!(d.depth(), depth, "{name}");
    }
}
