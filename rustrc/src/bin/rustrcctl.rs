//! ```
//! USAGE: rustrcctl [--control-sock PATH] COMMAND [ARGS...]
//! ```
//!
//! Send one command to a running rustrc over its control socket and print the response.  With no
//! COMMAND, read one command per line from stdin.  The socket defaults to $RUSTRC_CONTROL_SOCK,
//! then rc.sock.
//!
//! Commands understood by rustrc:
//!
//! ```text
//! status [SERVICE...]                    state, pid, uptime, starts, last exit, backoff, log
//! services -l [SERVICE...]               list known services
//! services -e [SERVICE...]               list enabled services
//! services -r [-n]                       reload (or, with -n, show what a reload would do)
//! services -s|-S|-R SERVICE...           start, stop, restart
//! kill [-s SIGNAL] SERVICE|PID...        signal a service's processes (default TERM)
//! ```
//!
//! Exits 1 if any line of a response reports an error, 2 if the socket can't be reached.

fn is_error(response: &str) -> bool {
    response
        .lines()
        .any(|l| l.starts_with("error:") || l.contains(": error:"))
}

fn invoke(client: &mut unix_sock::Client, sock: &str, command: &str) -> bool {
    match client.invoke(command) {
        Ok(response) => {
            let response = response.trim_end();
            if !response.is_empty() {
                println!("{response}");
            }
            !is_error(response)
        }
        Err(err) => {
            eprintln!("rustrcctl: {sock}: {err}");
            std::process::exit(2);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut sock = std::env::var("RUSTRC_CONTROL_SOCK").unwrap_or_else(|_| "rc.sock".to_string());
    if let Some(first) = args.first() {
        if first == "-h" || first == "--help" {
            println!("USAGE: rustrcctl [--control-sock PATH] COMMAND [ARGS...]");
            return;
        }
        if first == "--control-sock" {
            if args.len() < 2 {
                eprintln!("--control-sock requires a path");
                std::process::exit(129);
            }
            sock = args[1].clone();
            args.drain(..2);
        } else if let Some(path) = first.strip_prefix("--control-sock=") {
            sock = path.to_string();
            args.remove(0);
        }
    }
    let mut client = match unix_sock::Client::new(sock.as_str()) {
        Ok(client) => client,
        Err(err) => {
            eprintln!("rustrcctl: {sock}: {err}");
            std::process::exit(2);
        }
    };
    let ok = if args.is_empty() {
        let mut ok = true;
        let mut line = String::new();
        loop {
            line.clear();
            match std::io::stdin().read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(err) => {
                    eprintln!("rustrcctl: stdin: {err}");
                    std::process::exit(2);
                }
            }
            let command = line.trim();
            if command.is_empty() {
                continue;
            }
            ok &= invoke(&mut client, &sock, command);
        }
        ok
    } else {
        invoke(&mut client, &sock, &shvar::quote(args))
    };
    if !ok {
        std::process::exit(1);
    }
}
