use setsum::Setsum;

fn main() {
    for argument in std::env::args().skip(1) {
        if let Some(setsum) = Setsum::from_hexdigest(&argument) {
            println!("{}", (Setsum::default() - setsum).hexdigest());
        } else {
            eprintln!("don't know how to parse {argument} as setsum");
            std::process::exit(1);
        }
    }
}
