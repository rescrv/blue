use std::fs::File;
use std::io::{BufRead, BufReader};
use std::pin::Pin;

use tag_index::{InvertedTagIndex, Tag, TagIndex, Tags};

fn construct_index(tagfile: &str) -> Pin<Box<InvertedTagIndex>> {
    let reader = BufReader::new(File::open(tagfile).unwrap());
    let iti = InvertedTagIndex::default();
    for line in reader.lines() {
        let line = line.unwrap();
        let tags = Tags::new(line.trim()).unwrap();
        iti.insert(tags);
    }
    Box::pin(iti)
}

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
    let iti = construct_index(args[1]);
    let queries = parse_queries(args[2]);
    let queries_len = queries.len();
    let mut count = 0;
    let start = std::time::Instant::now();
    for query in queries.into_iter() {
        let tagses = iti.search(&[query]).unwrap();
        count += tagses.len();
    }
    let elapsed = start.elapsed();
    println!("performed {} queries in {:?} and fetched {} tags", queries_len, elapsed, count);
}
