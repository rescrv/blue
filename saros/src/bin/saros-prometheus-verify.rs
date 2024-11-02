fn main() {
    for arg in std::env::args().skip(1) {
        let contents = std::fs::read_to_string(arg).expect("should be able to read file");
        let prometheus_lines = saros::support_nom::parse_all(saros::prometheus::parse)(&contents)
            .expect("should be able to parse file");
        println!("{prometheus_lines:#?}");
    }
}
