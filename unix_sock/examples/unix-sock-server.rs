use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
struct Counter {
    count: AtomicU64,
}

impl unix_sock::Invokable for Counter {
    fn invoke(&self, command: &str) -> String {
        println!("INVOKE: {command:?}");
        format!("{}\n", self.count.fetch_add(1, Ordering::Relaxed))
    }
}

fn main() {
    let counter = Counter::default();
    let mut server =
        unix_sock::Server::new("unix.sock", counter).expect("server should instantiate");
    let context = unix_sock::Context::new().expect("context should create");
    server.serve(&context).expect("serve should never error");
}
