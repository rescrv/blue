#![doc = include_str!("../README.md")]

use std::borrow::{Borrow, Cow};
use std::ffi::OsStr;
use std::fs;
use std::io;
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

    fn try_from_std_path_buf(path: PathBuf) -> io::Result<Path<'static>> {
        Path::try_from(path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "path contains non-utf8 characters",
            )
        })
    }

    fn join_str<'b>(lhs: &str, rhs: &str) -> Path<'b> {
        if Path::new(rhs).is_abs() || lhs.is_empty() {
            Path::from(rhs.to_string())
        } else if lhs.ends_with('/') {
            Path::from(format!("{lhs}{rhs}"))
        } else {
            Path::from(format!("{lhs}/{rhs}"))
        }
    }

    fn split_str(&self) -> (&str, &str) {
        if self.path.is_empty() {
            return (".", ".");
        }
        if let Some(index) = self.path.rfind('/') {
            let dirname = if index == 0 {
                "/"
            } else if index == 1 && self.path.starts_with("//") {
                "//"
            } else if self.path[..index].chars().all(|c| c == '/') {
                "/"
            } else {
                self.path[..index].trim_end_matches('/')
            };
            let basename = if index + 1 == self.path.len() {
                "."
            } else {
                &self.path[index + 1..]
            };
            (dirname, basename)
        } else {
            (".", &self.path)
        }
    }

    fn file_stem_and_extension(&self) -> Option<(&str, Option<&str>)> {
        let file_name = self.file_name()?;
        if matches!(file_name, "." | "..") {
            return Some((file_name, None));
        }
        match file_name.rfind('.') {
            Some(0) | None => Some((file_name, None)),
            Some(dot) => Some((&file_name[..dot], Some(&file_name[dot + 1..]))),
        }
    }

    fn file_prefix_and_extension(&self) -> Option<(&str, Option<&str>)> {
        let file_name = self.file_name()?;
        if matches!(file_name, "." | "..") {
            return Some((file_name, None));
        }
        match file_name.as_bytes()[1..].iter().position(|b| *b == b'.') {
            Some(dot) => {
                let dot = dot + 1;
                Some((&file_name[..dot], Some(&file_name[dot + 1..])))
            }
            None => Some((file_name, None)),
        }
    }

    /// View the path as a std::path::Path.
    pub fn into_std(&self) -> &std::path::Path {
        std::path::Path::new::<str>(self.path.as_ref())
    }

    /// Convert the path to a std::path::Path.
    pub fn as_std_path(&self) -> &std::path::Path {
        self.into_std()
    }

    /// Convert the path to an OsStr.
    pub fn as_os_str(&self) -> &OsStr {
        OsStr::new(self.as_str())
    }

    /// Convert the path to a str.
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Convert the path to a str.
    pub fn to_str(&self) -> Option<&str> {
        Some(self.as_str())
    }

    /// Convert the path to a string, losslessly because this type is always UTF-8.
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }

    /// Convert the path to a std::path::PathBuf.
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.as_str())
    }

    /// Is the path a directory?
    pub fn is_dir(&self) -> io::Result<bool> {
        self.metadata().map(|metadata| metadata.is_dir())
    }

    /// Is the path a file?
    pub fn is_file(&self) -> io::Result<bool> {
        self.metadata().map(|metadata| metadata.is_file())
    }

    /// Is the path a symbolic link?
    pub fn is_symlink(&self) -> io::Result<bool> {
        self.symlink_metadata()
            .map(|metadata| metadata.is_symlink())
    }

    /// Compute the basename of the path.  This is guaranteed to be a non-empty path component
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
    pub fn exists(&self) -> io::Result<bool> {
        self.try_exists()
    }

    /// Return whether the path exists, preserving I/O errors.
    pub fn try_exists(&self) -> io::Result<bool> {
        match self.metadata() {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Query the filesystem for metadata, following symbolic links.
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        fs::metadata(self)
    }

    /// Query the filesystem for metadata without following symbolic links.
    pub fn symlink_metadata(&self) -> io::Result<fs::Metadata> {
        fs::symlink_metadata(self)
    }

    /// Read the target of a symbolic link.
    pub fn read_link(&self) -> io::Result<Path<'static>> {
        Self::try_from_std_path_buf(fs::read_link(self)?)
    }

    /// Return an iterator over the entries in this directory.
    pub fn read_dir(&self) -> io::Result<fs::ReadDir> {
        fs::read_dir(self)
    }

    /// True if the path begins with some number of slashes, other than the POSIX-exception of //.
    pub fn has_root(&self) -> bool {
        self.path.starts_with('/') && !self.has_app_defined()
    }

    /// True if the path begins with //, but not ///.
    pub fn has_app_defined(&self) -> bool {
        self.path.starts_with("//") && self.path.as_bytes().get(2) != Some(&b'/')
    }

    /// True if the path is absolute.
    pub fn is_abs(&self) -> bool {
        self.has_root() || self.has_app_defined()
    }

    /// True if the path is absolute.
    pub fn is_absolute(&self) -> bool {
        self.is_abs()
    }

    /// True if the path is relative.
    pub fn is_relative(&self) -> bool {
        !self.is_abs()
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
        Self::join_str(&self.path, &with.path)
    }

    /// Strip a prefix from the path.  The prefix and path are allowed to be non-normal and will
    /// have "." components dropped from consideration.
    pub fn strip_prefix<'b>(&self, prefix: impl Into<Path<'b>>) -> Option<Path<'_>> {
        let prefix = prefix.into();
        // NOTE(rescrv):  You might be tempted to use components() and zip() to solve and/or
        // simplify this.  That fails for one reason:  "components()" intentionally rewrites `foo/`
        // as `foo/.`, but this method should preserve the path that remains as much as possible,
        // including `.` components.
        #[derive(Clone, Copy, Eq, PartialEq)]
        enum RootKind {
            Relative,
            Regular,
            AppDefined,
        }

        fn root(path: &Path) -> RootKind {
            if path.has_app_defined() {
                RootKind::AppDefined
            } else if path.has_root() {
                RootKind::Regular
            } else {
                RootKind::Relative
            }
        }

        fn without_root(path: &str, root: RootKind) -> &str {
            match root {
                RootKind::Relative => path,
                RootKind::Regular => &path[1..],
                RootKind::AppDefined => &path[2..],
            }
        }

        fn next_component(path: &str) -> Option<(&str, &str)> {
            let path = path.trim_start_matches('/');
            if path.is_empty() {
                return None;
            }
            if let Some(slash) = path.find('/') {
                Some((&path[..slash], &path[slash + 1..]))
            } else {
                Some((path, ""))
            }
        }

        fn skip_path_dots(mut path: &str) -> &str {
            loop {
                let trimmed = path.trim_start_matches('/');
                if let Some(rest) = trimmed.strip_prefix("./") {
                    path = rest;
                } else if trimmed == "." {
                    return "";
                } else {
                    return trimmed;
                }
            }
        }

        fn consume_path_dot(path: &str) -> &str {
            let trimmed = path.trim_start_matches('/');
            if let Some(rest) = trimmed.strip_prefix("./") {
                rest
            } else if trimmed == "." {
                ""
            } else {
                trimmed
            }
        }

        let path_root = root(self);
        let prefix_root = root(&prefix);
        if path_root != prefix_root {
            return None;
        }
        let mut path = without_root(&self.path, path_root);
        let mut prefix = without_root(&prefix.path, prefix_root);
        loop {
            if let Some((prefix_component, prefix_rest)) = next_component(prefix) {
                prefix = prefix_rest;
                if prefix_component == "." {
                    path = consume_path_dot(path);
                    continue;
                }
                path = skip_path_dots(path);
                if let Some((path_component, path_rest)) = next_component(path) {
                    if path_component != prefix_component {
                        return None;
                    }
                    path = path_rest;
                } else {
                    return None;
                }
            } else {
                path = path.trim_start_matches('/');
                if path.is_empty() {
                    return Some(Path::new("."));
                } else {
                    return Some(Path::new(path));
                }
            }
        }
    }

    /// True if the path starts with the provided base path.
    pub fn starts_with<'b>(&self, base: impl Into<Path<'b>>) -> bool {
        self.strip_prefix(base).is_some()
    }

    /// True if the path ends with the provided child path.
    pub fn ends_with<'b>(&self, child: impl Into<Path<'b>>) -> bool {
        let child = child.into();
        let path_components = self.components().collect::<Vec<_>>();
        let child_components = child.components().collect::<Vec<_>>();
        path_components.get(path_components.len().saturating_sub(child_components.len())..)
            == Some(child_components.as_slice())
    }

    /// Return the path without its final component, if any.
    pub fn parent(&self) -> Option<Path<'_>> {
        if self.path.is_empty() {
            return None;
        }
        let parent = self.dirname();
        if parent.as_str() == self.as_str() {
            None
        } else {
            Some(parent)
        }
    }

    /// Iterate over the path and its parents.
    pub fn ancestors(&self) -> impl Iterator<Item = Path<'static>> + '_ {
        std::iter::successors(Some(self.clone().into_owned()), |path| {
            path.parent().map(|parent| parent.into_owned())
        })
    }

    /// Return the final path component, if it is a normal file name.
    pub fn file_name(&self) -> Option<&str> {
        Some(self.split_str().1)
    }

    /// Return the file name without its final extension.
    pub fn file_stem(&self) -> Option<&str> {
        self.file_stem_and_extension().map(|(stem, _)| stem)
    }

    /// Return the file name without its first extension.
    pub fn file_prefix(&self) -> Option<&str> {
        self.file_prefix_and_extension().map(|(prefix, _)| prefix)
    }

    /// Return the final file extension without the leading dot.
    pub fn extension(&self) -> Option<&str> {
        self.file_stem_and_extension()
            .and_then(|(_, extension)| extension)
    }

    /// Create a path like this one, but with its file name replaced.
    pub fn with_file_name(&self, file_name: impl AsRef<str>) -> Path<'static> {
        Self::join_str(self.dirname().as_str(), file_name.as_ref())
    }

    /// Create a path like this one, but with its extension replaced.
    pub fn with_extension(&self, extension: impl AsRef<str>) -> Path<'static> {
        let Some((stem, old_extension)) = self.file_stem_and_extension() else {
            return self.clone().into_owned();
        };
        if matches!(stem, "." | "..") {
            return self.clone().into_owned();
        }
        let extension = extension.as_ref();
        let file_name = match (old_extension, extension.is_empty()) {
            (Some(_), true) => stem.to_string(),
            (Some(_), false) | (None, false) => format!("{stem}.{extension}"),
            (None, true) => return self.clone().into_owned(),
        };
        self.with_file_name(file_name)
    }

    /// Create a path like this one, but with an extension added.
    pub fn with_added_extension(&self, extension: impl AsRef<str>) -> Path<'static> {
        let extension = extension.as_ref();
        let Some(file_name) = self.file_name() else {
            return self.clone().into_owned();
        };
        if extension.is_empty() || matches!(file_name, "." | "..") {
            return self.clone().into_owned();
        }
        self.with_file_name(format!("{file_name}.{extension}"))
    }

    /// Split the path into basename and dirname components.
    pub fn split(&self) -> (Path<'_>, Path<'_>) {
        let (dirname, basename) = self.split_str();
        (Path::new(dirname), Path::new(basename))
    }

    /// Return an iterator over the path components.  A path with a basename of "." will always end
    /// with Component::CurDir.
    pub fn components(&self) -> impl Iterator<Item = Component<'_>> {
        let mut components = vec![];
        if self.path.is_empty() {
            components.push(Component::CurDir);
            return components.into_iter();
        }
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

    /// Iterate over the path components as strings.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.components().map(|component| match component {
            Component::RootDir => "/",
            Component::AppDefined => "//",
            Component::CurDir => ".",
            Component::ParentDir => "..",
            Component::Normal(path) => match path.path {
                Cow::Borrowed(path) => path,
                Cow::Owned(_) => unreachable!("components always borrow from self"),
            },
        })
    }

    /// Return a display adapter for the path.
    pub fn display(&self) -> impl std::fmt::Display + '_ {
        self
    }

    /// Return the current working directory, if it can be fetched and converted to unicode without
    /// error.
    pub fn cwd() -> Option<Path<'a>> {
        Path::try_from(std::env::current_dir().ok()?).ok()
    }

    /// Return the canonicalized path with all intermediate components normalized and symbolic
    /// links resolved.
    pub fn canonicalize(&self) -> Result<Path<'static>, std::io::Error> {
        Self::try_from_std_path_buf(fs::canonicalize(self)?)
    }
}

impl AsRef<std::ffi::OsStr> for Path<'_> {
    fn as_ref(&self) -> &std::ffi::OsStr {
        let path: &std::ffi::OsStr = self.as_str().as_ref();
        path
    }
}

impl AsRef<std::path::Path> for Path<'_> {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}

impl Borrow<std::path::Path> for Path<'_> {
    fn borrow(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}

impl std::fmt::Debug for Path<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.path)
    }
}

impl std::fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{}", self.path)
    }
}

impl From<String> for Path<'_> {
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

impl TryFrom<std::path::PathBuf> for Path<'_> {
    type Error = std::str::Utf8Error;

    fn try_from(p: std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(Self {
            path: Cow::Owned(<&str>::try_from(p.as_os_str())?.to_string()),
        })
    }
}

impl TryFrom<std::ffi::OsString> for Path<'_> {
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
            path: Path::new(""),
            basename: Path::new("."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "C",
        },
        TestCase {
            path: Path::new("."),
            basename: Path::new("."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: false,
            components: "C",
        },
        TestCase {
            path: Path::new(".."),
            basename: Path::new(".."),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: true,
            components: "P",
        },
        TestCase {
            path: Path::new("foo"),
            basename: Path::new("foo"),
            dirname: Path::new("."),
            is_abs: false,
            is_normal: true,
            components: "N",
        },
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
            for (lhs, rhs) in std::iter::zip(tc.components.chars(), components) {
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

    #[test]
    fn has_app_defined_handles_non_ascii_paths() {
        let path = Path::new("//\u{e9}");
        assert!(path.has_app_defined());
        assert!(!path.has_root());
        assert!(path.is_abs());

        let path = Path::new("///\u{e9}");
        assert!(!path.has_app_defined());
        assert!(path.has_root());
        assert!(path.is_abs());
    }

    #[test]
    fn join_has_utf8path_edge_case_values() {
        for (base, child, expected) in [
            ("", "", ""),
            ("", "bar", "bar"),
            (".", "", "./"),
            (".", "bar", "./bar"),
            ("/", "", "/"),
            ("/", "bar", "/bar"),
            ("//", "", "//"),
            ("//", "bar", "//bar"),
            ("foo", "", "foo/"),
            ("foo", "bar", "foo/bar"),
            ("foo/", "", "foo/"),
            ("foo/", "bar", "foo/bar"),
            ("foo/bar", "", "foo/bar/"),
            ("foo/bar", "baz", "foo/bar/baz"),
            ("foo/bar", "/abs", "/abs"),
            ("foo/bar", "//app", "//app"),
        ] {
            assert_eq!(
                Path::new(expected),
                Path::new(base).join(child),
                "base: {base:?} child: {child:?}"
            );
        }
    }

    #[test]
    fn strip_prefix_matches_whole_components() {
        assert_eq!(None, Path::new("foobar").strip_prefix("foo"));
        assert_eq!(
            Some(Path::new("bar")),
            Path::new("foo/bar").strip_prefix("foo")
        );
        assert_eq!(
            Some(Path::new("./bar")),
            Path::new("foo/./bar").strip_prefix("foo")
        );
        assert_eq!(
            Some(Path::new(".")),
            Path::new("foo/./bar").strip_prefix("foo/bar")
        );
        assert_eq!(None, Path::new("/foo").strip_prefix("foo"));
        assert_eq!(None, Path::new("//foo").strip_prefix("/"));
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ObserverOutputs {
        as_str: String,
        to_str: Option<String>,
        to_string_lossy: String,
        to_path_buf: String,
        is_absolute: bool,
        is_relative: bool,
        parent: Option<String>,
        ancestors: Vec<String>,
        file_name: Option<String>,
        file_stem: Option<String>,
        file_prefix: Option<String>,
        extension: Option<String>,
        iter: Vec<String>,
        display: String,
    }

    fn observe(path: Path) -> ObserverOutputs {
        ObserverOutputs {
            as_str: path.as_str().to_string(),
            to_str: path.to_str().map(str::to_string),
            to_string_lossy: path.to_string_lossy().to_string(),
            to_path_buf: path.to_path_buf().to_string_lossy().to_string(),
            is_absolute: path.is_absolute(),
            is_relative: path.is_relative(),
            parent: path.parent().map(|parent| parent.as_str().to_string()),
            ancestors: path
                .ancestors()
                .map(|ancestor| ancestor.as_str().to_string())
                .collect(),
            file_name: path.file_name().map(str::to_string),
            file_stem: path.file_stem().map(str::to_string),
            file_prefix: path.file_prefix().map(str::to_string),
            extension: path.extension().map(str::to_string),
            iter: path.iter().map(str::to_string).collect(),
            display: path.display().to_string(),
        }
    }

    #[test]
    fn path_named_observers_have_expected_values() {
        assert_eq!(
            ObserverOutputs {
                as_str: "".to_string(),
                to_str: Some("".to_string()),
                to_string_lossy: "".to_string(),
                to_path_buf: "".to_string(),
                is_absolute: false,
                is_relative: true,
                parent: None,
                ancestors: vec!["".to_string()],
                file_name: Some(".".to_string()),
                file_stem: Some(".".to_string()),
                file_prefix: Some(".".to_string()),
                extension: None,
                iter: vec![".".to_string()],
                display: "".to_string(),
            },
            observe(Path::new(""))
        );
        assert_eq!(
            ObserverOutputs {
                as_str: ".".to_string(),
                to_str: Some(".".to_string()),
                to_string_lossy: ".".to_string(),
                to_path_buf: ".".to_string(),
                is_absolute: false,
                is_relative: true,
                parent: None,
                ancestors: vec![".".to_string()],
                file_name: Some(".".to_string()),
                file_stem: Some(".".to_string()),
                file_prefix: Some(".".to_string()),
                extension: None,
                iter: vec![".".to_string()],
                display: ".".to_string(),
            },
            observe(Path::new("."))
        );
        assert_eq!(
            ObserverOutputs {
                as_str: "/tmp/foo.tar.gz".to_string(),
                to_str: Some("/tmp/foo.tar.gz".to_string()),
                to_string_lossy: "/tmp/foo.tar.gz".to_string(),
                to_path_buf: "/tmp/foo.tar.gz".to_string(),
                is_absolute: true,
                is_relative: false,
                parent: Some("/tmp".to_string()),
                ancestors: vec![
                    "/tmp/foo.tar.gz".to_string(),
                    "/tmp".to_string(),
                    "/".to_string(),
                ],
                file_name: Some("foo.tar.gz".to_string()),
                file_stem: Some("foo.tar".to_string()),
                file_prefix: Some("foo".to_string()),
                extension: Some("gz".to_string()),
                iter: vec!["/".to_string(), "tmp".to_string(), "foo.tar.gz".to_string()],
                display: "/tmp/foo.tar.gz".to_string(),
            },
            observe(Path::new("/tmp/foo.tar.gz"))
        );
        assert_eq!(
            ObserverOutputs {
                as_str: "foo.txt/..".to_string(),
                to_str: Some("foo.txt/..".to_string()),
                to_string_lossy: "foo.txt/..".to_string(),
                to_path_buf: "foo.txt/..".to_string(),
                is_absolute: false,
                is_relative: true,
                parent: Some("foo.txt".to_string()),
                ancestors: vec![
                    "foo.txt/..".to_string(),
                    "foo.txt".to_string(),
                    ".".to_string()
                ],
                file_name: Some("..".to_string()),
                file_stem: Some("..".to_string()),
                file_prefix: Some("..".to_string()),
                extension: None,
                iter: vec!["foo.txt".to_string(), "..".to_string()],
                display: "foo.txt/..".to_string(),
            },
            observe(Path::new("foo.txt/.."))
        );
        assert_eq!(
            ObserverOutputs {
                as_str: ".config.toml".to_string(),
                to_str: Some(".config.toml".to_string()),
                to_string_lossy: ".config.toml".to_string(),
                to_path_buf: ".config.toml".to_string(),
                is_absolute: false,
                is_relative: true,
                parent: Some(".".to_string()),
                ancestors: vec![".config.toml".to_string(), ".".to_string()],
                file_name: Some(".config.toml".to_string()),
                file_stem: Some(".config".to_string()),
                file_prefix: Some(".config".to_string()),
                extension: Some("toml".to_string()),
                iter: vec![".config.toml".to_string()],
                display: ".config.toml".to_string(),
            },
            observe(Path::new(".config.toml"))
        );
    }

    #[test]
    fn starts_with_and_ends_with_have_utf8path_values() {
        for (path, other, starts_with, ends_with) in [
            ("/etc/passwd", "/etc", true, false),
            ("/etc/passwd", "/e", false, false),
            ("/etc/passwd", "passwd", false, true),
            ("/etc/passwd", "etc/passwd", false, true),
            ("/etc/passwd", "/etc/passwd", true, true),
            ("foo/bar/baz", "foo/bar", true, false),
            ("foo/bar/baz", "bar/baz", false, true),
            ("foo/bar/baz", "baz", false, true),
            ("foo/bar/baz", "az", false, false),
            ("foo/./bar", "foo/bar", true, false),
            ("foo/bar/", "foo/bar", true, false),
            ("//app/path", "/app", false, false),
            ("//app/path", "//app", true, false),
        ] {
            assert_eq!(
                starts_with,
                Path::new(path).starts_with(other),
                "path: {path:?} other: {other:?}"
            );
            assert_eq!(
                ends_with,
                Path::new(path).ends_with(other),
                "path: {path:?} other: {other:?}"
            );
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct BuilderOutputs {
        join_empty: Path<'static>,
        join_relative: Path<'static>,
        join_absolute: Path<'static>,
        join_app_defined: Path<'static>,
        with_file_name: Path<'static>,
        with_extension: Path<'static>,
        without_extension: Path<'static>,
        with_added_extension: Path<'static>,
        with_added_empty_extension: Path<'static>,
    }

    fn build(path: Path<'static>) -> BuilderOutputs {
        BuilderOutputs {
            join_empty: path.join(""),
            join_relative: path.join("child"),
            join_absolute: path.join("/absolute"),
            join_app_defined: path.join("//app"),
            with_file_name: path.with_file_name("name.txt"),
            with_extension: path.with_extension("rs"),
            without_extension: path.with_extension(""),
            with_added_extension: path.with_added_extension("bak"),
            with_added_empty_extension: path.with_added_extension(""),
        }
    }

    #[test]
    fn path_named_builders_have_expected_values() {
        assert_eq!(
            BuilderOutputs {
                join_empty: Path::from(""),
                join_relative: Path::from("child"),
                join_absolute: Path::from("/absolute"),
                join_app_defined: Path::from("//app"),
                with_file_name: Path::from("./name.txt"),
                with_extension: Path::from(""),
                without_extension: Path::from(""),
                with_added_extension: Path::from(""),
                with_added_empty_extension: Path::from(""),
            },
            build(Path::new(""))
        );
        assert_eq!(
            BuilderOutputs {
                join_empty: Path::from("./"),
                join_relative: Path::from("./child"),
                join_absolute: Path::from("/absolute"),
                join_app_defined: Path::from("//app"),
                with_file_name: Path::from("./name.txt"),
                with_extension: Path::from("."),
                without_extension: Path::from("."),
                with_added_extension: Path::from("."),
                with_added_empty_extension: Path::from("."),
            },
            build(Path::new("."))
        );
        assert_eq!(
            BuilderOutputs {
                join_empty: Path::from("/tmp/foo.tar.gz/"),
                join_relative: Path::from("/tmp/foo.tar.gz/child"),
                join_absolute: Path::from("/absolute"),
                join_app_defined: Path::from("//app"),
                with_file_name: Path::from("/tmp/name.txt"),
                with_extension: Path::from("/tmp/foo.tar.rs"),
                without_extension: Path::from("/tmp/foo.tar"),
                with_added_extension: Path::from("/tmp/foo.tar.gz.bak"),
                with_added_empty_extension: Path::from("/tmp/foo.tar.gz"),
            },
            build(Path::new("/tmp/foo.tar.gz"))
        );
        assert_eq!(
            BuilderOutputs {
                join_empty: Path::from("foo.txt/../"),
                join_relative: Path::from("foo.txt/../child"),
                join_absolute: Path::from("/absolute"),
                join_app_defined: Path::from("//app"),
                with_file_name: Path::from("foo.txt/name.txt"),
                with_extension: Path::from("foo.txt/.."),
                without_extension: Path::from("foo.txt/.."),
                with_added_extension: Path::from("foo.txt/.."),
                with_added_empty_extension: Path::from("foo.txt/.."),
            },
            build(Path::new("foo.txt/.."))
        );
    }

    #[derive(Debug, Eq, PartialEq)]
    struct PrefixOutputs {
        strip: Option<Path<'static>>,
        starts_with: bool,
        ends_with: bool,
    }

    fn prefix_outputs(path: &str, other: &str) -> PrefixOutputs {
        PrefixOutputs {
            strip: Path::new(path)
                .strip_prefix(other)
                .map(|path| path.into_owned()),
            starts_with: Path::new(path).starts_with(other),
            ends_with: Path::new(path).ends_with(other),
        }
    }

    #[test]
    fn prefix_methods_have_expected_values() {
        assert_eq!(
            PrefixOutputs {
                strip: Some(Path::from("bar")),
                starts_with: true,
                ends_with: false,
            },
            prefix_outputs("foo/bar", "foo")
        );
        assert_eq!(
            PrefixOutputs {
                strip: None,
                starts_with: false,
                ends_with: false,
            },
            prefix_outputs("foobar", "foo")
        );
        assert_eq!(
            PrefixOutputs {
                strip: Some(Path::from("./bar")),
                starts_with: true,
                ends_with: false,
            },
            prefix_outputs("foo/./bar", "foo")
        );
        assert_eq!(
            PrefixOutputs {
                strip: Some(Path::from(".")),
                starts_with: true,
                ends_with: false,
            },
            prefix_outputs("foo/./bar", "foo/bar")
        );
        assert_eq!(
            PrefixOutputs {
                strip: None,
                starts_with: false,
                ends_with: true,
            },
            prefix_outputs("/etc/passwd", "passwd")
        );
        assert_eq!(
            PrefixOutputs {
                strip: None,
                starts_with: false,
                ends_with: false,
            },
            prefix_outputs("//app/path", "/app")
        );
    }

    #[test]
    fn conversions_accept_utf8_and_reject_non_utf8() {
        let borrowed = Path::try_from(std::path::Path::new("foo/bar")).unwrap();
        assert_eq!(Path::new("foo/bar"), borrowed);
        let owned = Path::try_from(std::path::PathBuf::from("foo/bar")).unwrap();
        assert_eq!(Path::from("foo/bar"), owned);
        let os_owned = Path::try_from(std::ffi::OsString::from("foo/bar")).unwrap();
        assert_eq!(Path::from("foo/bar"), os_owned);

        #[cfg(unix)]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;

            let invalid = OsString::from_vec(vec![0xff, b'f', b'o', b'o']);
            assert!(Path::try_from(invalid).is_err());
            let invalid = OsString::from_vec(vec![0xff, b'f', b'o', b'o']);
            assert!(Path::try_from(std::path::PathBuf::from(invalid)).is_err());
        }
    }

    #[test]
    fn trait_views_match_std_path() {
        let path = Path::new("foo/bar");
        let os_str: &std::ffi::OsStr = path.as_ref();
        let std_path: &std::path::Path = path.as_ref();
        let borrowed: &std::path::Path = std::borrow::Borrow::borrow(&path);
        assert_eq!(std::ffi::OsStr::new("foo/bar"), os_str);
        assert_eq!(std::path::Path::new("foo/bar"), std_path);
        assert_eq!(std::path::Path::new("foo/bar"), borrowed);
        assert_eq!(std::path::Path::new("foo/bar"), path.as_std_path());
        assert_eq!(std::path::Path::new("foo/bar"), path.into_std());
    }

    #[test]
    fn filesystem_methods_report_existing_paths() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");
        assert!(src.exists().unwrap());
        assert!(src.try_exists().unwrap());
        assert!(src.is_dir().unwrap());
        assert!(!src.is_file().unwrap());
        assert!(!src.is_symlink().unwrap());
        assert!(src.metadata().unwrap().is_dir());
        assert!(src.symlink_metadata().unwrap().is_dir());
        assert!(src.read_dir().unwrap().next().is_some());

        assert!(lib.exists().unwrap());
        assert!(lib.try_exists().unwrap());
        assert!(lib.is_file().unwrap());
        assert!(!lib.is_dir().unwrap());
        assert!(!lib.is_symlink().unwrap());
        assert!(lib.metadata().unwrap().is_file());
        assert!(lib.symlink_metadata().unwrap().is_file());
        assert!(lib.canonicalize().unwrap().is_abs());
    }

    #[test]
    fn filesystem_methods_report_missing_paths() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("definitely-not-present-utf8path-test");
        assert!(!missing.exists().unwrap());
        assert!(!missing.try_exists().unwrap());
        assert!(missing.is_dir().is_err());
        assert!(missing.is_file().is_err());
        assert!(missing.is_symlink().is_err());
        assert!(missing.metadata().is_err());
        assert!(missing.symlink_metadata().is_err());
        assert!(missing.read_dir().is_err());
        assert!(missing.read_link().is_err());
        assert!(missing.canonicalize().is_err());
    }

    #[test]
    fn filesystem_predicates_report_invalid_input_errors() {
        let invalid = Path::new("\0");
        assert!(invalid.exists().is_err());
        assert!(invalid.try_exists().is_err());
        assert!(invalid.is_dir().is_err());
        assert!(invalid.is_file().is_err());
        assert!(invalid.is_symlink().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn read_link_returns_utf8_path_for_symlinks() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utf8path-read-link-{nonce}"));
        let root_path = Path::try_from(root.clone()).unwrap();
        let target = root_path.join("target.txt");
        let link = root_path.join("link.txt");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(&target, b"contents").unwrap();
        symlink(target.as_std_path(), link.as_std_path()).unwrap();

        assert!(link.is_symlink().unwrap());
        assert_eq!(target, link.read_link().unwrap());

        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
