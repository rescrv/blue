fn main() {
    let mut client = unix_sock::Client::new("unix.sock").expect("client should instantiate");
    let mut buffer = String::new();
    let stdin = std::io::stdin();
    loop {
        buffer.clear();
        stdin.read_line(&mut buffer).expect("read line should work");
        if buffer.is_empty() {
            break;
        }
        let response = client
            .invoke(&buffer)
            .expect("invoke should always succeed");
        println!("{}", response.trim());
    }
}
