fn main() {
    for line in std::io::stdin().lines() {
        let line = line.expect("should be able to read stdin");
        if let Err(error) = saros::querylang::parse(&line) {
            println!("{error:?}");
        } else {
            println!("valid");
        }
    }
}
