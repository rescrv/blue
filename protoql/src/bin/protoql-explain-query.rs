fn main() {
    let protoql = std::fs::read_to_string("/dev/stdin").expect("should read /dev/stdin");
    let query = protoql::Query::parse(protoql).expect("query should parse");
    println!("{:#?}", query);
}
