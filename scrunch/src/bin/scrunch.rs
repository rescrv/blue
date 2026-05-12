use std::fs::{read, write};

use scrunch::builder::Builder;
use scrunch::{CompressedDocument, Document};

pub fn load_file(file: &str) -> (Vec<u32>, Vec<usize>) {
    let bytes = read(file).expect("file should read");
    let mut record_boundaries = vec![0usize];
    let text: Vec<u32> = if bytes.is_ascii() {
        let mut text = Vec::with_capacity(bytes.len());
        for (idx, byte) in bytes.into_iter().enumerate() {
            if byte == b'\n' {
                record_boundaries.push(idx + 1);
            }
            text.push(byte as u32);
        }
        text
    } else {
        let string = String::from_utf8(bytes).expect("file should read to string");
        let mut text = Vec::with_capacity(string.len());
        for c in string.chars() {
            text.push(c as u32);
            if c == '\n' {
                record_boundaries.push(text.len());
            }
        }
        text
    };
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
