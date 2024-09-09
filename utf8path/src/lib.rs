#![doc = include_str!("../README.md")]

use std::borrow::{Borrow, Cow};
use std::path::PathBuf;

/////////////////////////////////////////////// Path ///////////////////////////////////////////////

/// Path provides a copy-on-write-style path that is built around UTF8 strings.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Path<'a> {
    path: Cow<'a, str>,
}

impl<'a> Path<'a> {
    /// Create a new path that borrows the provided string.
    pub const fn new(s: &'a str) -> Self {
        Self {
            path: Cow::Borrowed(s),
        }
    }

    /// Convert the path into an owned path.
    pub fn into_owned(self) -> Path<'static> {
        Path {
            path: Cow::Owned(self.path.into_owned()),
        }
    }

    /// Convert the path into a std::path::PathBuf.
    pub fn into_std(&self) -> &std::path::Path {
        std::path::Path::new::<str>(self.path.as_ref())
    }

    /// Convert the path to a str.
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Is the path a directory?
    pub fn is_dir(&self) -> bool {
        std::path::Path::new(self.path.as_ref()).is_dir()
    }

    /// Compute the basename of the path.  This is guaraneed to be a non-empty path component
    /// (falling back to "." for paths that end with "/").
    pub fn basename(&self) -> Path<'_> {
        self.split().1
    }

    /// Compute the dirname of the path.  This is guaranteed to be a non-empty path component
    /// (falling back to "." or "/" for single-component paths).
    pub fn dirname(&self) -> Path<'_> {
        self.split().0
    }

    /// True if the path exists.
    pub fn exists(&self) -> bool {
        let path: &str = &self.path;
        PathBuf::from(path).exists()
    }

    /// True if the path begins with some number of slashes, other than the POSIX-exception of //.
    pub fn has_root(&self) -> bool {
        self.path.starts_with('/') && !self.has_app_defined()
    }

    /// True if the path begins with //, but not ///.
    pub fn has_app_defined(&self) -> bool {
        self.path.starts_with("//") && (self.path.len() == 2 || &self.path[2..3] != "/")
    }

    /// True if the path is absolute.
    pub fn is_abs(&self) -> bool {
        self.has_root() || self.has_app_defined()
    }

    /// True if the path contains no "." components; and, is absolute and has no ".." components,
    /// or is relative and has all ".." components at the start.
    pub fn is_normal(&self) -> bool {
        let start = if self.path.starts_with("//") {
            2
        } else if self.path.starts_with('/') {
            1
        } else {
            0
        };
        if self.path[start..].is_empty() {
            return start > 0;
        }
        let limit = if self.path[start..].ends_with('/') {
            self.path.len() - 1
        } else {
            self.path.len()
        };
        let components: Vec<_> = self.path[start..limit].split('/').collect();
        let mut parent_allowed = start == 0;
        for component in components {
            if parent_allowed {
                if matches!(component, "." | "") {
                    return false;
                }
                parent_allowed = component == "..";
            } else if matches!(component, ".." | "." | "") {
                return false;
            }
        }
        true
    }

    /// Join to this path another path.  Follows standard path rules where if the joined-with path
    /// is absolute, the first path is discarded.
    pub fn join<'b, 'c>(&self, with: impl Into<Path<'b>>) -> Path<'c>
    where
        'a: 'c,
        'b: 'c,
    {
        let with = with.into();
        if with.is_abs() {
            with.clone()
        } else {
            Path::from(format!("{}/{}", self.path, with.path))
        }
    }

    /// Strip a prefix from the path.  The prefix and path are allowed to be non-normal and will
    /// have "." components dropped from consideration.
    pub fn strip_prefix<'b>(&self, prefix: impl Into<Path<'b>>) -> Option<Path> {
        let prefix = prefix.into();
        // NOTE(rescrv):  You might be tempted to use components() and zip() to solve and/or
        // simplify this.  That fails for one reason:  "components()" intentionally rewrites `foo/`
        // as `foo/.`, but this method should preserve the path that remains as much as possible,
        // including `.` components.
        if self.has_root() && !prefix.has_root() {
            return None;
        }
        if self.has_app_defined() && !prefix.has_app_defined() {
            return None;
        }
        let mut path = self.path[..].trim_start_matches('/');
        let mut prefix = prefix.path[..].trim_start_matches('/');
        loop {
            if let Some(prefix_slash) = prefix.find('/') {
                let path_slash = path.find('/')?;
                if prefix[..prefix_slash] != path[..path_slash] {
                    return None;
                }
                path = path[path_slash + 1..].trim_start_matches('/');
                prefix = prefix[prefix_slash + 1..].trim_start_matches('/');
            } else if prefix == path {
                return Some(Path::new("."));
            } else if let Some(path) = path.strip_prefix(prefix) {
                let path = path.trim_start_matches('/');
                if path.is_empty() {
                    return Some(Path::new("."));
                } else {
                    return Some(Path::new(path));
                }
            } else if prefix.starts_with("./") {
                prefix = prefix[2..].trim_start_matches('/');
            } else if path.starts_with("./") {
                path = path[2..].trim_start_matches('/');
            } else if prefix.is_empty() || prefix == "." {
                if path.is_empty() {
                    return Some(Path::new("."));
                } else {
                    return Some(Path::new(path));
                }
            }
        }
    }

    /// Split the path into basename and dirname components.
    pub fn split(&self) -> (Path, Path) {
        if let Some(index) = self.path.rfind('/') {
            let dirname = if index == 0 {
                Path::new("/")
            } else if index == 1 && self.path.starts_with("//") {
                Path::new("//")
            } else if self.path[..index].chars().all(|c| c == '/') {
                Path::new("/")
            } else {
                Path::new(self.path[..index].trim_end_matches('/'))
            };
            let basename = if index + 1 == self.path.len() {
                Path::new(".")
            } else {
                Path::new(&self.path[index + 1..])
            };
            (dirname, basename)
        } else {
            (Path::new("."), Path::new(&self.path))
        }
    }

    /// Return an iterator ovre the path components.  A path with a basename of "." will always end
    /// with Component::CurDir.
    pub fn components(&self) -> impl Iterator<Item = Component<'_>> {
        let mut components = vec![];
        let mut limit = self.path.len();
        while let Some(slash) = self.path[..limit].rfind('/') {
            if slash + 1 == limit {
                components.push(Component::CurDir);
            } else if &self.path[slash + 1..limit] == ".." {
                components.push(Component::ParentDir);
            } else if &self.path[slash + 1..limit] == "." {
                components.push(Component::CurDir);
            } else {
                components.push(Component::Normal(Path::new(&self.path[slash + 1..limit])));
            }
            if slash == 0 {
                components.push(Component::RootDir);
                limit = 0;
            } else if slash == 1 && self.path.starts_with("//") {
                components.push(Component::AppDefined);
                limit = 0;
            } else if self.path[..slash].chars().all(|c| c == '/') {
                components.push(Component::RootDir);
                limit = 0;
            } else {
                limit = slash;
                while limit > 0 && self.path[..limit].ends_with('/') {
                    limit -= 1;
                }
            }
        }
        if limit > 0 {
            if &self.path[..limit] == ".." {
                components.push(Component::ParentDir);
            } else if &self.path[..limit] == "." {
                components.push(Component::CurDir);
            } else {
                components.push(Component::Normal(Path::new(&self.path[..limit])));
            }
        }
        components.reverse();
        components.into_iter()
    }

    /// Return the current working directory, if it can be fetched and converted to unicode without
    /// error.
    pub fn cwd() -> Option<Path<'a>> {
        Path::try_from(std::env::current_dir().ok()?).ok()
    }
}

impl<'a> AsRef<std::path::Path> for Path<'a> {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}

impl<'a> Borrow<std::path::Path> for Path<'a> {
    fn borrow(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}

impl<'a> std::fmt::Debug for Path<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.path)
    }
}

impl<'a> std::fmt::Display for Path<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{}", self.path)
    }
}

impl<'a> From<String> for Path<'a> {
    fn from(s: String) -> Self {
        Self {
            path: Cow::Owned(s),
        }
    }
}

impl<'a> From<Path<'a>> for String {
    fn from(path: Path<'a>) -> Self {
        path.path.into_owned()
    }
}

impl<'a> From<&'a String> for Path<'a> {
    fn from(s: &'a String) -> Self {
        Self {
            path: Cow::Borrowed(s),
        }
    }
}

impl<'a> From<&'a str> for Path<'a> {
    fn from(s: &'a str) -> Self {
        Self {
            path: Cow::Borrowed(s),
        }
    }
}

impl<'a> From<&'a Path<'a>> for &'a str {
    fn from(path: &'a Path<'a>) -> Self {
        &path.path
    }
}

impl<'a> TryFrom<&'a std::path::Path> for Path<'a> {
    type Error = std::str::Utf8Error;

    fn try_from(p: &'a std::path::Path) -> Result<Self, Self::Error> {
        Ok(Self {
            path: Cow::Borrowed(<&str>::try_from(p.as_os_str())?),
        })
    }
}

impl<'a> TryFrom<std::path::PathBuf> for Path<'a> {
    type Error = std::str::Utf8Error;

    fn try_from(p: std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(Self {
            path: Cow::Owned(<&str>::try_from(p.as_os_str())?.to_string()),
        })
    }
}

impl<'a> TryFrom<std::ffi::OsString> for Path<'a> {
    type Error = std::str::Utf8Error;

    fn try_from(p: std::ffi::OsString) -> Result<Self, Self::Error> {
        Ok(Self {
            path: Cow::Owned(<&str>::try_from(p.as_os_str())?.to_string()),
        })
    }
}

impl<'a> From<Path<'a>> for std::path::PathBuf {
    fn from(path: Path<'a>) -> Self {
        PathBuf::from(path.path.to_string())
    }
}

///////////////////////////////////////////// Component ////////////////////////////////////////////

/// A component of a path.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Component<'a> {
    /// Signals the path component "/".
    RootDir,
    /// Signals the path component "//".
    AppDefined,
    /// Signals the "." path component.
    CurDir,
    /// Signals the ".." path component.
    ParentDir,
    /// Signals a component that doesn't match any of the special components.
    Normal(Path<'a>),
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::{Component, Path};

    struct TestCase<'a> {
        path: Path<'a>,
        basename: Path<'a>,
        dirname: Path<'a>,
        is_abs: bool,
        is_normal: bool,
        components: &'a str,
    }

    static TEST_CASES: &[TestCase] = &[
        TestCase {
            path: Path::new("//"),
            basename: Path::new("."),
            dirname: Path::new("//"),
            is_abs: true,
            is_normal: true,
            components: "AC",
        },
        TestCase {
            path: Path::new("//."),
            basename: Path::new("."),
            dirname: Path::new("//"),
            is_abs: true,
            is_normal: false,
            components: "AC",
        },
        TestCase {
            path: Path::new("//.."),
            basename: Path::new(".."),
            dirname: Path::new("//"),
            is_abs: true,
            is_normal: false,
            components: "AP",
        },
        TestCase {
            path: Path::new("//foo"),
            basename: Path::new("foo"),
            dirname: Path::new("//"),
            is_abs: true,
            is_normal: true,
            components: "AN",
        },
        TestCase {
            path: Path::new("/./"),
            basename: Path::new("."),
            dirname: Path::new("/."),
            is_abs: true,
            is_normal: false,
            components: "RCC",
        },
        TestCase {
            path: Path::new("/./."),
            basename: Path::new("."),
            dirname: Path::new("/."),
            is_abs: true,
            is_normal: false,
            components: "RCC",
        },
        TestCase {
            path: Path::new("/./.."),
            basename: Path::new(".."),
            dirname: Path::new("/."),
            is_abs: true,
            is_normal: false,
            components: "RCP",
        },
        TestCase {
            path: Path::new("/./foo"),
            basename: Path::new("foo"),
            dirname: Path::new("/."),
            is_abs: true,
            is_normal: false,
            components: "RCN",
        },
        TestCase {
            path: Path::new("/../"),
            basename: Path::new("."),
            dirname: Path::new("/.."),
            is_abs: true,
            is_normal: false,
            components: "RPC",
        },
        TestCase {
            path: Path::new("/../."),
            basename: Path::new("."),
            dirname: Path::new("/.."),
            is_abs: true,
            is_normal: false,
            components: "RPC",
        },
        TestCase {
            path: Path::new("/../.."),
            basename: Path::new(".."),
            dirname: Path::new("/.."),
            is_abs: true,
            is_normal: false,
            components: "RPP",
        },
        TestCase {
            path: Path::new("/../foo"),
            basename: Path::new("foo"),
            dirname: Path::new("/.."),
            is_abs: true,
            is_normal: false,
            components: "RPN",
        },
        TestCase {
            path: Path::new("/foo/"),
            basename: Path::new("."),
            dirname: Path::new("/foo"),
            is_abs: true,
            is_normal: true,
            components: "RNC",
        },
        TestCase {
            path: Path::new("/foo/."),
            basename: Path::new("."),
            dirname: Path::new("/foo"),
            is_abs: true,
            is_normal: false,
            components: "RNC",
        },
        TestCase {
            path: Path::new("/foo/.."),
            basename: Path::new(".."),
            dirname: Path::new("/foo"),
            is_abs: true,
            is_normal: false,
            components: "RNP",
        },
        TestCase {
            path: Path::new("/foo/foo"),
            basename: Path::new("foo"),
            dirname: Path::new("/foo"),
            is_abs: true,
            is_normal: true,
            components: "RNN",
        },
        TestCase {
            path: Path::new(".//"),
            basename: Path::new("."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "CC",
        },
        TestCase {
            path: Path::new(".//."),
            basename: Path::new("."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "CC",
        },
        TestCase {
            path: Path::new(".//.."),
            basename: Path::new(".."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "CP",
        },
        TestCase {
            path: Path::new(".//foo"),
            basename: Path::new("foo"),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "CN",
        },
        TestCase {
            path: Path::new("././"),
            basename: Path::new("."),
            dirname: Path::new("./."),
            is_abs: false,
            is_normal: false,
            components: "CCC",
        },
        TestCase {
            path: Path::new("././."),
            basename: Path::new("."),
            dirname: Path::new("./."),
            is_abs: false,
            is_normal: false,
            components: "CCC",
        },
        TestCase {
            path: Path::new("././.."),
            basename: Path::new(".."),
            dirname: Path::new("./."),
            is_abs: false,
            is_normal: false,
            components: "CCP",
        },
        TestCase {
            path: Path::new("././foo"),
            basename: Path::new("foo"),
            dirname: Path::new("./."),
            is_abs: false,
            is_normal: false,
            components: "CCN",
        },
        TestCase {
            path: Path::new("./../"),
            basename: Path::new("."),
            dirname: Path::new("./.."),
            is_abs: false,
            is_normal: false,
            components: "CPC",
        },
        TestCase {
            path: Path::new("./../."),
            basename: Path::new("."),
            dirname: Path::new("./.."),
            is_abs: false,
            is_normal: false,
            components: "CPC",
        },
        TestCase {
            path: Path::new("./../.."),
            basename: Path::new(".."),
            dirname: Path::new("./.."),
            is_abs: false,
            is_normal: false,
            components: "CPP",
        },
        TestCase {
            path: Path::new("./../foo"),
            basename: Path::new("foo"),
            dirname: Path::new("./.."),
            is_abs: false,
            is_normal: false,
            components: "CPN",
        },
        TestCase {
            path: Path::new("./foo/"),
            basename: Path::new("."),
            dirname: Path::new("./foo"),
            is_abs: false,
            is_normal: false,
            components: "CNC",
        },
        TestCase {
            path: Path::new("./foo/."),
            basename: Path::new("."),
            dirname: Path::new("./foo"),
            is_abs: false,
            is_normal: false,
            components: "CNC",
        },
        TestCase {
            path: Path::new("./foo/.."),
            basename: Path::new(".."),
            dirname: Path::new("./foo"),
            is_abs: false,
            is_normal: false,
            components: "CNP",
        },
        TestCase {
            path: Path::new("./foo/foo"),
            basename: Path::new("foo"),
            dirname: Path::new("./foo"),
            is_abs: false,
            is_normal: false,
            components: "CNN",
        },
        TestCase {
            path: Path::new("..//"),
            basename: Path::new("."),
            dirname: Path::new(".."),
            is_abs: false,
            is_normal: false,
            components: "PC",
        },
        TestCase {
            path: Path::new("..//."),
            basename: Path::new("."),
            dirname: Path::new(".."),
            is_abs: false,
            is_normal: false,
            components: "PC",
        },
        TestCase {
            path: Path::new("..//.."),
            basename: Path::new(".."),
            dirname: Path::new(".."),
            is_abs: false,
            is_normal: false,
            components: "PP",
        },
        TestCase {
            path: Path::new("..//foo"),
            basename: Path::new("foo"),
            dirname: Path::new(".."),
            is_abs: false,
            is_normal: false,
            components: "PN",
        },
        TestCase {
            path: Path::new(".././"),
            basename: Path::new("."),
            dirname: Path::new("../."),
            is_abs: false,
            is_normal: false,
            components: "PCC",
        },
        TestCase {
            path: Path::new(".././."),
            basename: Path::new("."),
            dirname: Path::new("../."),
            is_abs: false,
            is_normal: false,
            components: "PCC",
        },
        TestCase {
            path: Path::new(".././.."),
            basename: Path::new(".."),
            dirname: Path::new("../."),
            is_abs: false,
            is_normal: false,
            components: "PCP",
        },
        TestCase {
            path: Path::new(".././foo"),
            basename: Path::new("foo"),
            dirname: Path::new("../."),
            is_abs: false,
            is_normal: false,
            components: "PCN",
        },
        TestCase {
            path: Path::new("../../"),
            basename: Path::new("."),
            dirname: Path::new("../.."),
            is_abs: false,
            is_normal: true,
            components: "PPC",
        },
        TestCase {
            path: Path::new("../../."),
            basename: Path::new("."),
            dirname: Path::new("../.."),
            is_abs: false,
            is_normal: false,
            components: "PPC",
        },
        TestCase {
            path: Path::new("../../.."),
            basename: Path::new(".."),
            dirname: Path::new("../.."),
            is_abs: false,
            is_normal: true,
            components: "PPP",
        },
        TestCase {
            path: Path::new("../../foo"),
            basename: Path::new("foo"),
            dirname: Path::new("../.."),
            is_abs: false,
            is_normal: true,
            components: "PPN",
        },
        TestCase {
            path: Path::new("../foo/"),
            basename: Path::new("."),
            dirname: Path::new("../foo"),
            is_abs: false,
            is_normal: true,
            components: "PNC",
        },
        TestCase {
            path: Path::new("../foo/."),
            basename: Path::new("."),
            dirname: Path::new("../foo"),
            is_abs: false,
            is_normal: false,
            components: "PNC",
        },
        TestCase {
            path: Path::new("../foo/.."),
            basename: Path::new(".."),
            dirname: Path::new("../foo"),
            is_abs: false,
            is_normal: false,
            components: "PNP",
        },
        TestCase {
            path: Path::new("../foo/foo"),
            basename: Path::new("foo"),
            dirname: Path::new("../foo"),
            is_abs: false,
            is_normal: true,
            components: "PNN",
        },
        TestCase {
            path: Path::new("foo//"),
            basename: Path::new("."),
            dirname: Path::new("foo"),
            is_abs: false,
            is_normal: false,
            components: "NC",
        },
        TestCase {
            path: Path::new("foo//."),
            basename: Path::new("."),
            dirname: Path::new("foo"),
            is_abs: false,
            is_normal: false,
            components: "NC",
        },
        TestCase {
            path: Path::new("foo//.."),
            basename: Path::new(".."),
            dirname: Path::new("foo"),
            is_abs: false,
            is_normal: false,
            components: "NP",
        },
        TestCase {
            path: Path::new("foo//foo"),
            basename: Path::new("foo"),
            dirname: Path::new("foo"),
            is_abs: false,
            is_normal: false,
            components: "NN",
        },
        TestCase {
            path: Path::new("foo/./"),
            basename: Path::new("."),
            dirname: Path::new("foo/."),
            is_abs: false,
            is_normal: false,
            components: "NCC",
        },
        TestCase {
            path: Path::new("foo/./."),
            basename: Path::new("."),
            dirname: Path::new("foo/."),
            is_abs: false,
            is_normal: false,
            components: "NCC",
        },
        TestCase {
            path: Path::new("foo/./.."),
            basename: Path::new(".."),
            dirname: Path::new("foo/."),
            is_abs: false,
            is_normal: false,
            components: "NCP",
        },
        TestCase {
            path: Path::new("foo/./foo"),
            basename: Path::new("foo"),
            dirname: Path::new("foo/."),
            is_abs: false,
            is_normal: false,
            components: "NCN",
        },
        TestCase {
            path: Path::new("foo/../"),
            basename: Path::new("."),
            dirname: Path::new("foo/.."),
            is_abs: false,
            is_normal: false,
            components: "NPC",
        },
        TestCase {
            path: Path::new("foo/../."),
            basename: Path::new("."),
            dirname: Path::new("foo/.."),
            is_abs: false,
            is_normal: false,
            components: "NPC",
        },
        TestCase {
            path: Path::new("foo/../.."),
            basename: Path::new(".."),
            dirname: Path::new("foo/.."),
            is_abs: false,
            is_normal: false,
            components: "NPP",
        },
        TestCase {
            path: Path::new("foo/../foo"),
            basename: Path::new("foo"),
            dirname: Path::new("foo/.."),
            is_abs: false,
            is_normal: false,
            components: "NPN",
        },
        TestCase {
            path: Path::new("foo/foo/"),
            basename: Path::new("."),
            dirname: Path::new("foo/foo"),
            is_abs: false,
            is_normal: true,
            components: "NNC",
        },
        TestCase {
            path: Path::new("foo/foo/."),
            basename: Path::new("."),
            dirname: Path::new("foo/foo"),
            is_abs: false,
            is_normal: false,
            components: "NNC",
        },
        TestCase {
            path: Path::new("foo/foo/.."),
            basename: Path::new(".."),
            dirname: Path::new("foo/foo"),
            is_abs: false,
            is_normal: false,
            components: "NNP",
        },
        TestCase {
            path: Path::new("foo/foo/foo"),
            basename: Path::new("foo"),
            dirname: Path::new("foo/foo"),
            is_abs: false,
            is_normal: true,
            components: "NNN",
        },
    ];

    #[test]
    fn basename() {
        for tc in TEST_CASES.iter() {
            assert_eq!(tc.basename, tc.path.basename(), "path: {:?}", tc.path);
        }
    }

    #[test]
    fn dirname() {
        for tc in TEST_CASES.iter() {
            assert_eq!(tc.dirname, tc.path.dirname(), "path: {:?}", tc.path);
        }
    }

    #[test]
    fn is_abs() {
        for tc in TEST_CASES.iter() {
            assert_eq!(tc.is_abs, tc.path.is_abs(), "path: {:?}", tc.path);
        }
    }

    #[test]
    fn is_normal() {
        for tc in TEST_CASES.iter() {
            assert_eq!(tc.is_normal, tc.path.is_normal(), "path: {:?}", tc.path);
        }
    }

    #[test]
    fn strip_prefix() {
        for tc in TEST_CASES.iter() {
            assert_eq!(
                Some(tc.basename.clone()),
                tc.path.strip_prefix(tc.dirname.clone()),
                "path: {:?}",
                tc.path
            );
        }
    }

    #[test]
    fn split() {
        for tc in TEST_CASES.iter() {
            let (dirname, basename) = tc.path.split();
            assert_eq!(tc.basename, basename, "path: {:?}", tc.path);
            assert_eq!(tc.dirname, dirname, "path: {:?}", tc.path);
        }
    }

    #[test]
    fn components() {
        fn component_to_char(c: Component) -> char {
            match c {
                Component::AppDefined => 'A',
                Component::RootDir => 'R',
                Component::CurDir => 'C',
                Component::ParentDir => 'P',
                Component::Normal(_) => 'N',
            }
        }
        for tc in TEST_CASES.iter() {
            let components: Vec<_> = tc.path.components().collect();
            assert_eq!(
                tc.components.chars().count(),
                components.len(),
                "path: {:?}",
                tc.path
            );
            for (lhs, rhs) in std::iter::zip(tc.components.chars(), components.into_iter()) {
                assert_eq!(lhs, component_to_char(rhs), "path: {:?}", tc.path);
            }
        }
    }

    #[test]
    fn components_as_basename_dirname() {
        for tc in TEST_CASES.iter() {
            let mut components: Vec<_> = tc.path.components().collect();
            fn basename_to_component(path: Path) -> Component {
                if path.path == "." {
                    Component::CurDir
                } else if path.path == ".." {
                    Component::ParentDir
                } else {
                    Component::Normal(path)
                }
            }
            assert_eq!(
                Some(basename_to_component(tc.basename.clone())),
                components.pop()
            );
        }
    }
}
