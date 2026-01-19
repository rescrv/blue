//! Generates 1.5 billion structured errors with 3-10 fields each.

use std::io::Write;

use guacamole::combinators::range_to;
use guacamole::combinators::select;
use guacamole::combinators::string;
use guacamole::combinators::to_charset;
use guacamole::combinators::uniform;
use guacamole::combinators::CHAR_SET_ALNUM;
use guacamole::combinators::CHAR_SET_ALPHA;
use guacamole::Guacamole;
use handled::SError;

const PHASES: &[&str] = &[
    "parse",
    "lex",
    "eval",
    "compile",
    "link",
    "runtime",
    "io",
    "network",
    "database",
    "auth",
    "validation",
    "transform",
    "serialize",
    "deserialize",
    "dispatch",
    "route",
];

const CODES: &[&str] = &[
    "syntax-error",
    "type-mismatch",
    "undefined-symbol",
    "index-out-of-bounds",
    "null-pointer",
    "stack-overflow",
    "heap-exhausted",
    "timeout",
    "connection-refused",
    "permission-denied",
    "not-found",
    "already-exists",
    "invalid-argument",
    "precondition-failed",
    "internal-error",
    "unavailable",
];

const FIELD_NAMES: &[&str] = &[
    "line",
    "column",
    "offset",
    "length",
    "index",
    "count",
    "expected",
    "actual",
    "path",
    "file",
    "function",
    "module",
    "timestamp",
    "request_id",
    "user_id",
    "session_id",
    "retry_count",
    "duration_ms",
    "bytes_read",
    "bytes_written",
];

fn generate_error(guac: &mut Guacamole, output: &mut Vec<u8>) {
    let mut phase_selector = select(range_to(PHASES.len()), PHASES);
    let mut code_selector = select(range_to(CODES.len()), CODES);
    let mut field_name_selector = select(range_to(FIELD_NAMES.len()), FIELD_NAMES);
    let mut num_fields = uniform(3usize, 11usize);
    let mut atom_value = uniform(0u64, 1_000_000u64);
    let mut string_gen = string(uniform(5usize, 20usize), to_charset(CHAR_SET_ALPHA));
    let mut ident_gen = string(uniform(3usize, 12usize), to_charset(CHAR_SET_ALNUM));

    output.clear();
    let mut err = SError::new(phase_selector(guac)).with_code(code_selector(guac));

    let field_count = num_fields(guac);
    for i in 0..field_count {
        let field_name = if i < FIELD_NAMES.len() {
            field_name_selector(guac).to_string()
        } else {
            ident_gen(guac)
        };
        let field_type: u8 = range_to(3u8)(guac);
        match field_type {
            0 => {
                let val = atom_value(guac);
                err = err.with_atom_field(&field_name, val);
            }
            1 => {
                let value = string_gen(guac);
                err = err.with_string_field(&field_name, &value);
            }
            _ => {
                err = err.with_atom_field(&field_name, ident_gen(guac));
            }
        }
    }
    writeln!(output, "{}", err).expect("write failed");
}

fn main() {
    let total: u64 = 1_500_000_000;
    let mut guac = Guacamole::new(0xdeadbeef);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::with_capacity(1 << 20, stdout.lock());
    let mut buffer = Vec::with_capacity(1024);

    for _ in 0..total {
        generate_error(&mut guac, &mut buffer);
        out.write_all(&buffer).expect("write failed");
    }
    out.flush().expect("flush failed");
}
