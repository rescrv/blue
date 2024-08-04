fn main() {
    let mut printed = false;
    for (k, v) in std::env::vars() {
        println!("{k}={v}");
        printed = true;
    }
    if printed {
        println!();
    }
    let args = std::env::args().collect::<Vec<_>>();
    println!("{args:?}");
}
