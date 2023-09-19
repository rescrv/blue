buffertk
========

Buffertk provides tooling for serializing and deserializing data.

Status
------

Maintenance track.  The library is considered stable and will be put into maintenance mode if unchanged for one year.

Scope
-----

This library is about serialization and deserialization patterns that are common.  It is chiefly intended to provide the
primitives used by the [prototk](https://crates.io/crates/prototk) crate.

Example
-------

To pack, implement the [Packable] trait and use [stack_pack].

```
use buffertk::{v64, stack_pack};

let x = v64::from(42);
let buf: &[u8] = &stack_pack(x).to_vec();
assert_eq!(&[42u8], buf);
```

Unpacking uses the [Unpackable] trait or the [Unpacker].

```
use buffertk::{v64, Unpacker};

let mut up = Unpacker::new(&[42u8]);
let x: v64 = up.unpack().expect("[42] is a valid varint; something's wrong");
assert_eq!(42u64, x.into());
```


Warts
-----

- Some patterns are used frequently and could be abstracted better.  Given that most of this library is used with code
  generation this is not a concern.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/buffertk/latest/buffertk/).
