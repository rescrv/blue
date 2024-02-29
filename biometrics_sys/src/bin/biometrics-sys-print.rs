use std::fs::File;
use std::time::SystemTime;

use biometrics::PlainTextEmitter;

use biometrics_sys::BiometricsSys;

fn main() {
    let fout = File::create("/dev/stdout").unwrap();
    let mut emit = PlainTextEmitter::new(fout);
    let mut bio_sys = BiometricsSys::new();
    loop {
        let now: u64 = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock should never fail")
            .as_millis()
            .try_into()
            .expect("millis since epoch should fit u64");
        bio_sys.emit(&mut emit, now);
        std::thread::sleep(std::time::Duration::from_millis(249));
    }
}
