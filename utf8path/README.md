utf8path
========

utf8path provides a Path object that is guaranteed to be convertible to and from UTF8 with sane semantics.  Further
restrictions, such as those imposed by the filesystem/kernel, are not enforced by this library.

Status
------

New.  This library is new, so it will likely see changes in the near future as it sees more use.

Scope
-----

This library provides the Path object with sane dirname and basename methods.

Warts
-----

The implementations provided don't use the `components` method.  Ideally, we could use them, but it was easier to
manually juggle path separations.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/utf8path/latest/utf8path/).
