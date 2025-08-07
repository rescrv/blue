use guacamole::combinators::*;
use guacamole::Guacamole;

use tag_index::{Tag, Tags};

fn generator<'a>(
    key: &'a str,
    params: &'a str,
) -> impl FnMut(&mut Guacamole) -> Option<Tag<'a>> + 'a {
    assert!(params.starts_with('!'));
    let params = &params[1..];
    let mut idx = 0;
    let pieces = params.split('/').collect::<Vec<_>>();
    assert!(pieces.len() <= 2, "must provide !# or !#/#, nothing more");
    let (count, random) = if pieces.len() == 2 {
        let c = pieces[0]
            .parse::<usize>()
            .expect("first argument must be a usize");
        let r = pieces[1]
            .parse::<usize>()
            .expect("second argument must be a usize");
        (c, r)
    } else {
        let c = pieces[0]
            .parse::<usize>()
            .expect("argument must be a usize");
        (c, 4294967291)
    };
    let mut strings = from_seed(string(|_| 8, to_charset(CHAR_SET_LOWER)));
    let mut set_index = unique_set_index(random);
    move |_| {
        if idx >= count {
            None
        } else {
            idx += 1;
            Tag::new(key, &strings(set_index(idx - 1))).map(Tag::into_owned)
        }
    }
}

fn materialize(guac: &mut Guacamole, tag: Tag) -> Vec<Tag<'static>> {
    if tag.value().starts_with('!') {
        let mut ret = vec![];
        let mut gen = generator(tag.key(), tag.value());
        while let Some(tag) = gen(guac) {
            ret.push(tag.into_owned());
        }
        ret
    } else {
        vec![tag.into_owned()]
    }
}

fn generate(guac: &mut Guacamole, s: &str) {
    let tags = Tags::new(s).expect("tags should parse");
    let mut product = vec![];
    for tag in tags.tags() {
        product.push(materialize(guac, tag));
    }
    let mut indices = vec![0; product.len()];
    loop {
        let mut tags = ":".to_string();
        for idx in 0..indices.len() {
            tags += &product[idx][indices[idx]].to_string();
            tags.push(':');
        }
        println!("{tags}");
        for idx in 0..indices.len() {
            indices[idx] += 1;
            if indices[idx] < product[idx].len() {
                break;
            }
            indices[idx] = 0;
        }
        if indices.iter().all(|i| *i == 0) {
            break;
        }
    }
}

fn main() {
    let mut guac = Guacamole::new(0);
    let args: Vec<String> = std::env::args().collect();
    for arg in &args[1..] {
        generate(&mut guac, arg);
    }
}
