use statslicer::{Bencher, Parameter, Parameters, benchmark, black_box, statslicer_main};

use macarunes::{
    Macaroon, NONCE_BYTES, Nonce, SIGNATURE_BYTES, Secret, ThirdPartySecret, Verifier,
};

const DISCHARGES: &[usize] = &[0, 1, 2, 4, 8, 16, 32, 64];
const REVERSED: &[bool] = &[false, true];

#[derive(Debug, Default, Eq, PartialEq)]
struct VerifyParameters {
    discharges: usize,
    reversed: bool,
}

impl Parameters for VerifyParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("discharges", Parameter::Integer(self.discharges as u64)),
            ("reversed", Parameter::Bool(self.reversed)),
        ]
    }
}

fn secret(label: u8, index: usize) -> Secret {
    let mut bytes = [0u8; SIGNATURE_BYTES];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = label
            .wrapping_add(index as u8)
            .wrapping_add((offset as u8).wrapping_mul(17));
    }
    Secret::from_bytes(bytes)
}

fn nonce(index: usize) -> Nonce {
    let mut bytes = [0u8; NONCE_BYTES];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = 0x80u8
            .wrapping_add(index as u8)
            .wrapping_add((offset as u8).wrapping_mul(31));
    }
    Nonce::from_bytes(bytes)
}

fn location(index: usize) -> String {
    format!("bench-location-{index}")
}

fn identifier(index: usize) -> String {
    format!("bench-identifier-{index}")
}

fn proof(discharge_count: usize, reversed: bool) -> (Macaroon, Secret, Vec<Macaroon>) {
    let root_secret = secret(0x10, 0);
    let mut root = Macaroon::new("bench-root", "bench-root", root_secret.clone()).unwrap();
    let secrets: Vec<_> = (0..discharge_count)
        .map(|index| secret(0x40, index))
        .collect();

    if discharge_count > 0 {
        let third_party_secret =
            ThirdPartySecret::new(root.signature(), nonce(0), &secrets[0]).unwrap();
        root.add_third_party_caveat(location(0), identifier(0), third_party_secret)
            .unwrap();
    }

    let mut discharges = Vec::with_capacity(discharge_count);
    for index in 0..discharge_count {
        let mut discharge =
            Macaroon::new(location(index), identifier(index), secrets[index].clone()).unwrap();
        if index + 1 < discharge_count {
            let third_party_secret =
                ThirdPartySecret::new(discharge.signature(), nonce(index + 1), &secrets[index + 1])
                    .unwrap();
            discharge
                .add_third_party_caveat(
                    location(index + 1),
                    identifier(index + 1),
                    third_party_secret,
                )
                .unwrap();
        }
        discharges.push(discharge);
    }

    for discharge in &mut discharges {
        root.bind_discharge(discharge).unwrap();
    }
    if reversed {
        discharges.reverse();
    }

    (root, root_secret, discharges)
}

fn bench_verify(params: &VerifyParameters, b: &mut Bencher) {
    let (root, root_secret, discharges) = proof(params.discharges, params.reversed);
    let verifier = Verifier::new();
    let size = b.size();

    // The verifier intentionally scans the whole discharge set instead of stopping at the first
    // public identifier match.  These caller-controlled macaroons may be reordered, so the benchmark
    // measures the quadratic cost that avoids order-dependent timing assumptions.
    b.run(|| {
        for _ in 0..size {
            black_box(verifier.verify(
                black_box(&root),
                black_box(&root_secret),
                black_box(&discharges),
            ))
            .unwrap();
        }
    });
}

benchmark! {
    name = verify;
    VerifyParameters {
        discharges in DISCHARGES,
        reversed in REVERSED,
    }
    bench_verify
}

statslicer_main! {
    verify
}
