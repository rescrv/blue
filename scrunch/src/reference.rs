use crate::Index;

pub struct ReferenceIndex {
    text: Vec<char>,
}

impl ReferenceIndex {
    pub fn new(text: &[char]) -> Self {
        ReferenceIndex {
            text: text.to_vec(),
        }
    }
}

impl Index for ReferenceIndex {
    type Item = char;

    fn length(&self) -> usize {
        self.text.len()
    }

    fn extract<'a>(
        &'a self,
        idx: usize,
        count: usize,
    ) -> Option<Box<dyn Iterator<Item = Self::Item> + 'a>> {
        Some(Box::new(ReferenceExtractIterator {
            iter: self.text[idx..idx + count].iter(),
        }))
    }

    fn search<'a>(&'a self, needle: &'a [Self::Item]) -> Box<dyn Iterator<Item = usize> + 'a> {
        if needle.len() == 0 {
            return Box::new(0..self.length());
        }
        let mut results = Vec::new();
        for (idx, candidate) in self.text.windows(needle.len()).enumerate() {
            if candidate == needle {
                results.push(idx);
            }
        }
        Box::new(ReferenceSearchIterator { results, pos: 0 })
    }

    fn count(&self, needle: &[char]) -> usize {
        self.search(needle).count()
    }
}

pub struct ReferenceExtractIterator<'a> {
    iter: ::std::slice::Iter<'a, char>,
}

impl<'a> Iterator for ReferenceExtractIterator<'a> {
    type Item = char;

    fn next(&mut self) -> std::option::Option<char> {
        match self.iter.next() {
            Some(c) => Some(*c),
            None => None,
        }
    }
}

pub struct ReferenceSearchIterator {
    results: Vec<usize>,
    pos: usize,
}

impl Iterator for ReferenceSearchIterator {
    type Item = usize;

    fn next(&mut self) -> std::option::Option<usize> {
        if self.pos >= self.results.len() {
            return None;
        }
        let idx = self.pos;
        self.pos += 1;
        Some(self.results[idx])
    }
}
