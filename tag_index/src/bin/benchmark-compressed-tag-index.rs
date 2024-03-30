use std::fs::File;
use std::io::{BufRead, BufReader};

use tag_index::{CompressedTagIndex, Tag, TagIndex, Tags};

fn parse_queries(queries: &str) -> Vec<Tag> {
    let reader = BufReader::new(File::open(queries).unwrap());
    let mut queries = vec![];
    for line in reader.lines() {
        let line = line.unwrap();
        queries.extend(Tags::new(&line).unwrap().tags().map(Tag::to_owned));
    }
    queries
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    if args.len() != 3 {
        eprintln!("USAGE: benchmark-inverted-tag-index tagfile queries");
        std::process::exit(129);
    }
    let cti = CompressedTagIndex::open(args[1]).unwrap();
    let queries = parse_queries(args[2]);
    let queries_len = queries.len();
    let mut count = 0;
    let start = std::time::Instant::now();
    for (idx, query) in queries.into_iter().enumerate() {
        let tagses = cti.search(&[query]).unwrap();
        count += tagses.len();
        println!("{}% done", idx);
    }
    let elapsed = start.elapsed();
    println!("performed {} queries in {:?} and fetched {} tags", queries_len, elapsed, count);
}
