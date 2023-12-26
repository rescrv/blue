extern crate proptest;

use std::io::Cursor;

use keyvalint::{KeyValuePair, KeyValueRef};
use proptest::prelude::{ProptestConfig, Strategy};

use sst::log::{LogBuilder, LogIterator, LogOptions, WriteBatch};
use sst::Builder;

proptest::prop_compose! {
    pub fn arb_string()(str in "[a-zA-Z][_a-zA-Z0-9]{0, 64}") -> String {
        str.to_string()
    }
}

proptest::prop_compose! {
    pub fn arb_key_value_pair()(key in arb_string(),
                                value in arb_string(),
                                timestamp in 0..u64::MAX) -> KeyValuePair {
        KeyValuePair {
            key: key.as_bytes().to_vec(),
            timestamp,
            value: Some(value.as_bytes().to_vec()),
        }
    }
}

proptest::prop_compose! {
    pub fn inner_arb_write_batch()(mut batch in proptest::collection::vec(arb_key_value_pair(), 0..16)) -> (WriteBatch, Vec<KeyValuePair>) {
        batch.sort();
        let mut wb = WriteBatch::default();
        let mut kvps = vec![];
        for kvp in batch {
            // Rather than rely on proptest's prop_filter we will just truncate the batch.  It's
            // too hard to size the batches just right otherwise.
            if wb.insert(KeyValueRef::from(&kvp)).is_err() {
                break;
            }
            kvps.push(kvp);
        }
        (wb, kvps)
    }
}

fn sized_right(wb: &(WriteBatch, Vec<KeyValuePair>)) -> bool {
    wb.0.approximate_size() > 0 && (wb.0.approximate_size() as u64) < sst::log::MAX_BATCH_SIZE
}

proptest::prop_compose! {
    pub fn arb_write_batch()(batch in inner_arb_write_batch().prop_filter("batch size", sized_right)) -> (WriteBatch, Vec<KeyValuePair>) {
        batch
    }
}

#[allow(clippy::ptr_arg)]
fn still_sized_right(wbs: &Vec<(WriteBatch, Vec<KeyValuePair>)>) -> bool {
    wbs.iter()
        .map(|wb| wb.0.approximate_size() as u64)
        .fold(0, u64::saturating_add)
        < (keyvalint::TABLE_FULL_SIZE / 64) as u64
}

proptest::proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2, .. ProptestConfig::default()
    })]

    #[test]
    fn log(write_batches in proptest::collection::vec(arb_write_batch(), 0..1024).prop_filter("table size", still_sized_right)) {
        let mut buffer = Vec::new();
        let mut log = LogBuilder::from_write(LogOptions::default(), &mut buffer).expect("log writer should work");
        for (wb, _) in write_batches.iter() {
            log.append(wb).expect("append should work");
        }
        log.seal().expect("seal should work");
        let mut log = LogIterator::from_reader(LogOptions::default(), Cursor::new(buffer)).expect("log reader should work");
        for (_, kvps) in write_batches.iter() {
            for kvp in kvps.iter() {
                if let Some(kvr) = log.next().expect("next should not fail") {
                    assert_eq!(KeyValueRef::from(kvp), kvr);
                } else {
                    panic!("log truncates early");
                }
            }
        }
        assert_eq!(None, log.next().expect("next should never fail"))
    }
}
