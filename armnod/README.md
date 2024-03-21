armnod
======

Armnod is an anagram for "random"; Armnod is a library for working with random strings.

Each [Armnod] consists of a [SeedChooser], [LengthChooser], and [CharacterChooser] which
compose a set of random strings.  The SeedChooser picks the element of the set.  It may say to
stop iterating (enough items have been chosen), it may say to seek to a particular offset in
another guacamole generator (there's a finite number of seeds), or it may say to not seek at
all (an "infinite" number of strings are possible).

The [SeedChooser] and [LengthChooser] both pull from a [guacamole::Guacamole] stream to
generate the seed and a u32 for the string's length.  It's easy to see that when the `guac` is
positioned at the same point in the stream, the seed and length will be the same.

[CharacterChooser] pulls bytes from the string and maps them to characters to create a string.
Essentially mapping the binary data to ASCII data.  UTF-8 marginally supported.

Status
------

Passive development.  The warts pulled it from being maintenance track on 2023-09-19

Scope
-----

This library provides the armnod type and an embeddable command-line interface.

Warts
-----

- The [ArmnodOptions] does not create the Armnod instance; it should.

Documentation
-------------

The latest documentation is always available at [docs.rs](https://docs.rs/armnod/latest/armnod/).
