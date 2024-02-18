use std::fs::{read_to_string, write};

use scrunch::builder::Builder;
use scrunch::{CompressedDocument, Document};

pub fn load_file(file: &str) -> (Vec<u32>, Vec<usize>) {
    let text: Vec<u32> = read_to_string(file)
        .expect("file should read to string")
        .chars()
        .map(|c| c as u32)
        .collect();
    let mut record_boundaries = vec![0usize];
    for (idx, _) in text.iter().enumerate().filter(|(_, t)| **t == '\n' as u32) {
        record_boundaries.push(idx + 1);
    }
    if record_boundaries[record_boundaries.len() - 1] == text.len() {
        record_boundaries.pop();
    }
    (text, record_boundaries)
}

fn main() {
    for file in std::env::args().skip(1) {
        let (text, record_boundaries) = load_file(&file);
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        CompressedDocument::construct(text, record_boundaries, &mut builder)
            .expect("document should construct");
        drop(builder);
        let file = file + ".scrunch";
        write(file, buf).expect("write should succeed");
    }
}
