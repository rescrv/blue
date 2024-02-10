fn main() {
    let protoql = std::fs::read_to_string("/dev/stdin").expect("should read /dev/stdin");
    let table_set = protoql::TableSet::parse(protoql).expect("schema should parse");
    for key in table_set.describe_keys() {
        println!("{}", key);
    }
}
