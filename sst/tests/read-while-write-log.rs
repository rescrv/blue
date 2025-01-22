extern crate sst;

use sst::log::{LogBuilder, LogIterator, LogOptions, WriteBatch};
use sst::Builder;

#[test]
fn read_while_writing_log() {
    let _ = std::fs::remove_file("read_while_writing.log");
    let mut log_builder = LogBuilder::new(LogOptions::default(), "read_while_writing.log").unwrap();
    // First write batch: key1
    let mut wb = WriteBatch::default();
    wb.put(b"key1", 1, b"value1").unwrap();
    log_builder.append(&wb).unwrap();
    log_builder.flush().unwrap();
    log_builder.fsync().unwrap();
    // The key should be visible.
    let mut log = LogIterator::new(LogOptions::default(), "read_while_writing.log").unwrap();
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key1", kvp.key);
    assert_eq!(Some(b"value1".as_ref()), kvp.value);
    assert_eq!(1, kvp.timestamp);
    // Second write batch: key2
    let mut wb = WriteBatch::default();
    wb.put(b"key2", 2, b"value2").unwrap();
    log_builder.append(&wb).unwrap();
    log_builder.flush().unwrap();
    log_builder.fsync().unwrap();
    // A new iterator should see both keys.
    let mut log = LogIterator::new(LogOptions::default(), "read_while_writing.log").unwrap();
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key1", kvp.key);
    assert_eq!(Some(b"value1".as_ref()), kvp.value);
    assert_eq!(1, kvp.timestamp);
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key2", kvp.key);
    assert_eq!(Some(b"value2".as_ref()), kvp.value);
    assert_eq!(2, kvp.timestamp);
}

#[test]
fn read_while_writing_conclog() {
    let _ = std::fs::remove_file("read_while_writing_concurrent.log");
    let mut log_builder =
        LogBuilder::new(LogOptions::default(), "read_while_writing_concurrent.log").unwrap();
    // First write batch: key1
    let mut wb = WriteBatch::default();
    wb.put(b"key1", 1, b"value1").unwrap();
    log_builder.append(&wb).unwrap();
    log_builder.flush().unwrap();
    log_builder.fsync().unwrap();
    // The key should be visible.
    let mut log =
        LogIterator::new(LogOptions::default(), "read_while_writing_concurrent.log").unwrap();
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key1", kvp.key);
    assert_eq!(Some(b"value1".as_ref()), kvp.value);
    assert_eq!(1, kvp.timestamp);
    // Second write batch: key2
    let mut wb = WriteBatch::default();
    wb.put(b"key2", 2, b"value2").unwrap();
    log_builder.append(&wb).unwrap();
    log_builder.flush().unwrap();
    log_builder.fsync().unwrap();
    // A new iterator should see both keys.
    let mut log =
        LogIterator::new(LogOptions::default(), "read_while_writing_concurrent.log").unwrap();
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key1", kvp.key);
    assert_eq!(Some(b"value1".as_ref()), kvp.value);
    assert_eq!(1, kvp.timestamp);
    let kvp = log.next().unwrap().unwrap();
    assert_eq!(b"key2", kvp.key);
    assert_eq!(Some(b"value2".as_ref()), kvp.value);
    assert_eq!(2, kvp.timestamp);
}
