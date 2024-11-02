use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::ffi::c_void;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use buffertk::Unpackable;
use listfree::List;
use scrunch::builder::Builder;
use scrunch::{CompressedDocument, Document};

///////////////////////////////////////////// constants ////////////////////////////////////////////

const BAD_CHARS: &str = "=";

//////////////////////////////////////////////// Tag ///////////////////////////////////////////////

fn invalid(s: &str) -> bool {
    s.is_empty()
        || s.chars()
            .any(|c| BAD_CHARS.contains(c) || c.is_whitespace() || c.is_control())
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub struct Tag<'a> {
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

impl<'a> Tag<'a> {
    pub fn new(key: &'a str, value: &'a str) -> Option<Tag<'a>> {
        if invalid(key) || invalid(value) {
            None
        } else {
            Some(Tag {
                key: Cow::Borrowed(key),
                value: Cow::Borrowed(value),
            })
        }
    }

    pub fn into_owned(self) -> Tag<'static> {
        Tag {
            key: Cow::Owned(self.key.into_owned()),
            value: Cow::Owned(self.value.into_owned()),
        }
    }

    pub fn key(&self) -> &str {
        self.key.as_ref()
    }

    pub fn value(&self) -> &str {
        self.value.as_ref()
    }
}

impl<'a> Debug for Tag<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}={}", self.key, self.value)
    }
}

impl<'a> Display for Tag<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}={}", self.key, self.value)
    }
}

/////////////////////////////////////////////// Tags ///////////////////////////////////////////////

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Tags<'a> {
    tags: Cow<'a, str>,
}

impl<'a> Tags<'a> {
    pub fn new<'b, S: Into<Cow<'b, str>>>(tags: S) -> Option<Tags<'b>> {
        let tags = tags.into();
        Self::parse(&tags)?;
        Some(Tags { tags })
    }

    pub fn into_owned(self) -> Tags<'static> {
        Tags {
            tags: Cow::Owned(self.tags.into_owned()),
        }
    }

    pub fn tags(&self) -> impl Iterator<Item = Tag<'_>> + '_ {
        // NOTE(rescrv):  We never construct a Tags that won't parse.
        Self::parse(&self.tags).unwrap().into_iter()
    }

    fn parse<'b>(tags: &'b Cow<'b, str>) -> Option<Vec<Tag<'b>>> {
        if !tags.starts_with(':') || !tags.ends_with(':') {
            return None;
        }
        let mut t = &tags[1..];
        let mut seen = HashSet::new();
        let mut tags = vec![];
        while !t.is_empty() {
            let colon = t.find(':')?;
            let equal = t.find('=')?;
            if colon < equal {
                return None;
            }
            let key = &t[..equal];
            if seen.contains(key) {
                return None;
            }
            let value = &t[equal + 1..colon];
            tags.push(Tag::new(key, value)?);
            seen.insert(key);
            t = &t[colon + 1..];
        }
        if seen.is_empty() {
            return None;
        }
        Some(tags)
    }
}

impl<'a> From<Vec<Tag<'a>>> for Tags<'static> {
    fn from(tags: Vec<Tag<'a>>) -> Self {
        let mut s = String::new();
        for tag in tags {
            s.push(':');
            s.push_str(tag.key());
            s.push('=');
            s.push_str(tag.value());
        }
        s.push(':');
        Tags {
            tags: Cow::Owned(s),
        }
    }
}

impl<'a> Debug for Tags<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.tags)
    }
}

impl<'a> Display for Tags<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.tags)
    }
}

///////////////////////////////////////////// TagIndex /////////////////////////////////////////////

pub trait TagIndex {
    fn search<'a>(self: &'a Pin<Box<Self>>, tags: &[Tag]) -> Result<Vec<Tags<'a>>, std::io::Error>;
}

//////////////////////////////////////// CompressedTagIndex ////////////////////////////////////////

pub struct CompressedTagIndex<'a> {
    data: *mut c_void,
    size: usize,
    doc: MaybeUninit<CompressedDocument<'a>>,
}

impl<'a> CompressedTagIndex<'a> {
    pub fn create<P: AsRef<Path>>(tagses: &[Tags], path: P) -> Result<(), std::io::Error> {
        let mut contents = String::with_capacity(tagses.iter().map(|t| t.tags.len() + 1).sum());
        let mut record_boundaries = Vec::with_capacity(tagses.len());
        for tags in tagses {
            record_boundaries.push(contents.len());
            contents += &tags.tags;
            contents.push('\n');
        }
        let text = contents.chars().map(|c| c as u32).collect();
        let mut buf = Vec::new();
        let mut builder = Builder::new(&mut buf);
        CompressedDocument::construct(text, record_boundaries, &mut builder).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("could not construct document: {err:?}"),
            )
        })?;
        drop(builder);
        std::fs::write(path.as_ref(), buf)
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Pin<Box<Self>>, std::io::Error> {
        let file = File::open(path.as_ref())?;
        let md = file.metadata()?;
        if md.len() > usize::MAX as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "file overflows usize",
            ));
        }
        // SAFETY(rescrv):  We know this mapping is safe to dereference and later drop.
        let mapping = unsafe {
            libc::mmap64(
                std::ptr::null_mut(),
                md.len() as usize,
                libc::PROT_READ,
                libc::MAP_SHARED | libc::MAP_POPULATE,
                file.as_raw_fd(),
                0,
            )
        };
        if mapping == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        let mut pin = Box::pin(CompressedTagIndex {
            data: mapping,
            size: md.len() as usize,
            doc: MaybeUninit::uninit(),
        });
        // SAFETY(rescrv):  We only ever refer to this region of memory as a slice of u8.
        let buf = unsafe { std::slice::from_raw_parts(pin.data as *const u8, pin.size) };
        let doc = CompressedDocument::unpack(buf)
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("could not unpack document: {err:?}"),
                )
            })?
            .0;
        pin.doc.write(doc);
        Ok(pin)
    }
}

impl<'a> TagIndex for CompressedTagIndex<'a> {
    fn search<'b>(self: &'b Pin<Box<Self>>, tags: &[Tag]) -> Result<Vec<Tags<'b>>, std::io::Error> {
        let doc = unsafe { &self.doc.assume_init_ref() };
        let mut first = true;
        let mut records = HashSet::new();
        for tag in tags.iter() {
            let mut needle = Vec::with_capacity(tag.key.len() + tag.value.len() + 3);
            needle.push(':' as u32);
            needle.extend(tag.key.as_ref().chars().map(|c| c as u32));
            needle.push('=' as u32);
            needle.extend(tag.value.as_ref().chars().map(|c| c as u32));
            needle.push(':' as u32);
            let mut new_records = HashSet::new();
            for text_offset in doc.search(&needle).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("could not lookup text offset: {err:?}"),
                )
            })? {
                let record_offset = doc.lookup(text_offset).map_err(|err| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("could not lookup text offset: {err:?}"),
                    )
                })?;
                if first || records.contains(&record_offset) {
                    new_records.insert(record_offset);
                }
            }
            first = false;
            records = new_records;
        }
        let mut tags = Vec::with_capacity(records.len());
        for record in records {
            let record = doc.retrieve(record).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("could not lookup text offset: {err:?}"),
                )
            })?;
            let record: String = record.iter().flat_map(|c| char::from_u32(*c)).collect();
            let record = record.trim();
            if let Some(t) = Tags::new(record) {
                let t = t.into_owned();
                tags.push(t)
            }
        }
        Ok(tags)
    }
}

impl<'a> Drop for CompressedTagIndex<'a> {
    fn drop(&mut self) {
        // SAFETY(rescrv): It will always be a valid mapping.
        unsafe {
            libc::munmap(self.data, self.size);
        }
    }
}

unsafe impl<'a> Send for CompressedTagIndex<'a> {}
unsafe impl<'a> Sync for CompressedTagIndex<'a> {}

///////////////////////////////////////// InvertedTagIndex /////////////////////////////////////////

#[derive(Default)]
pub struct InvertedTagIndex {
    by_tag: Mutex<BTreeMap<String, Arc<List<Arc<Tags<'static>>>>>>,
}

impl InvertedTagIndex {
    pub fn insert(&self, tags: Tags) {
        let tags = Arc::new(tags.into_owned());
        let mut by_tag = self.by_tag.lock().unwrap();
        for tag in tags.tags() {
            let list = by_tag.entry(tag.to_string()).or_default();
            list.prepend(Arc::clone(&tags));
        }
    }
}

impl TagIndex for InvertedTagIndex {
    fn search<'a>(self: &'a Pin<Box<Self>>, tags: &[Tag]) -> Result<Vec<Tags<'a>>, std::io::Error> {
        let tags = tags.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        let mut lists = vec![];
        {
            let by_tag = self.by_tag.lock().unwrap();
            for tag in tags {
                if let Some(ptr) = by_tag.get(&tag) {
                    lists.push(Arc::clone(ptr));
                }
            }
        }
        let mut first = true;
        let mut records: HashSet<Arc<Tags>> = HashSet::new();
        for list in lists {
            let mut new_records = HashSet::new();
            for record in list.iter() {
                if first || records.contains(record) {
                    new_records.insert(Arc::clone(record));
                }
            }
            first = false;
            records = new_records;
        }
        let mut tags = Vec::with_capacity(records.len());
        for record in records {
            tags.push((*record).clone());
        }
        Ok(tags)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{CompressedTagIndex, Tag, TagIndex, Tags};

    #[test]
    fn tag_into_owned() {
        let tag = Tag::new("key", "value").unwrap();
        let tag = tag.into_owned();
        assert!(matches!(tag.key, Cow::Owned(_)));
        assert!(matches!(tag.value, Cow::Owned(_)));
        assert_eq!("key", tag.key);
        assert_eq!("value", tag.value);
    }

    #[test]
    fn invalid_tags() {
        assert!(Tags::new(":").is_none());
        assert!(Tags::new("::").is_none());
        assert!(Tags::new(":::").is_none());
        assert!(Tags::new(":foo:").is_none());
        assert!(Tags::new(":foo:=:").is_none());
        assert!(Tags::new(":foo=bar::").is_none());
        assert!(Tags::new(":foo=bar:=baz:").is_none());
        assert!(Tags::new("foo=bar:").is_none());
        assert!(Tags::new(":foo=bar").is_none());
    }

    #[test]
    fn tags_into_owned() {
        let tags = Tags::new(":tag1=foo:tag2=bar:").unwrap();
        let tags = tags.into_owned();
        assert!(matches!(tags.tags, Cow::Owned(_)));
        assert_eq!(":tag1=foo:tag2=bar:", tags.tags);
    }

    #[test]
    fn tags_search() {
        let path = format!("search_{}.tags", line!());
        let tags1 = Tags::new(":tag1=foo:tag2=bar:").unwrap();
        let tags2 = Tags::new(":tag1=foo:tag2=baz:").unwrap();
        let tags3 = Tags::new(":tag1=bar:tag2=quux:").unwrap();
        CompressedTagIndex::create(&[tags1, tags2, tags3], &path).unwrap();
        let index = CompressedTagIndex::open(path).unwrap();
        let mut results = index.search(&[Tag::new("tag1", "foo").unwrap()]).unwrap();
        results.sort();
        assert_eq!(
            vec![
                Tags::new(":tag1=foo:tag2=bar:").unwrap(),
                Tags::new(":tag1=foo:tag2=baz:").unwrap()
            ],
            results
        );
    }

    #[test]
    fn tags_search_intersection() {
        let path = format!("search_{}.tags", line!());
        let tags1 = Tags::new(":tag1=foo:tag2=bar:").unwrap();
        let tags2 = Tags::new(":tag1=foo:tag2=baz:").unwrap();
        let tags3 = Tags::new(":tag1=bar:tag2=quux:").unwrap();
        CompressedTagIndex::create(&[tags1, tags2, tags3], &path).unwrap();
        let index = CompressedTagIndex::open(path).unwrap();
        let results = index
            .search(&[
                Tag::new("tag1", "foo").unwrap(),
                Tag::new("tag2", "bar").unwrap(),
            ])
            .unwrap();
        assert_eq!(vec![Tags::new(":tag1=foo:tag2=bar:").unwrap()], results);
    }
}
