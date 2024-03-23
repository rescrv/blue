use std::fs::read;

use buffertk::Unpackable;

use scrunch::CompressedDocument;

fn main() {
    for file in std::env::args().skip(1) {
        let data = read(&file).expect("should be able to read file");
        let doc = CompressedDocument::unpack(&data).expect("should be able to parse document").0;
        println!("{}", file);
        print!("{doc:#?}");
    }
}

/*
        let (text, record_boundaries) = load_file(&file);
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        CompressedDocument::construct(text, record_boundaries, &mut builder)
            .expect("document should construct");
        drop(builder);
        let file = file + ".scrunch";
        write(file, buf).expect("write should succeed");
*/
