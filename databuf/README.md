Databuf
=======

Databuf is a type system-evolved from protobuf intended for running on top of a key-value store.  The general idea of
the type system is to turn a byte-oriented key-value store into a rich key-value store with support for complex objects
and interactions.

Run
    PYTHONPATH=.:${PYTHONPATH} ./databuf.bin
for a demo that simulates some python of what it could look like.

# Databuf Types

A databuf object is a key-value pair comprised solely of fields like protobuf fields.  The key is composed of one or
more fields of the object that are extracted from the object when the object gets serialized to disk.  The value is a
blob of protobuf-encoded data.  Together they compose a key-value pair or record.

The fields may be comprised of the following types:

- int32:  An int32 encodes to an unsigned varint on the wire.  This means that it will likely not work well for negative
  numbers as the sign bit will extend them to be large varints.
- uint32:  An unsigned 32-bit integer type.  Use when the number will always be unsigned.
- sint32:  A signed 32-bit integer type.  Use when numbers will be small and possibly negative.
- int64:  An int64 encodes to an unsigned varint on the wire.
- uint64:  An unsigned 64-bit integer type. 
- sint64:  A signed 64-bit integer type.
- bool:  A boolean value.
- fixed32:  A 32-bit value intended for when values will likely use bits above the 28th.
- fixed64:  A 64-bit value intended for when values will likely use the upper bits.
- sfixed32:  A 32-bit value intended for when values will likely use bits above the 28th.  Use this variant when needing
  signed numbers in order.
- sfixed64:  A 64-bit value intended for when values will likely use the upper bits.
- float:  IEEE754 32-bit floating point value.
- double:  IEEE754 64-bit floating point value.
- bytes:  An untyped slice of bytes.
- string:  A UTF8-encoded string.
- message:  A nested object.

# General Key-Value Store Friendliness

Databuf is intended to put a veneer on top of existing key-value stores by specifying the format on two sides of a
transformation.  On the human side are the types that databuf presents to the user.  On the key-value store side of the
transformation are pure bytes ready to be stored.  The trick to this transformation is to specify databuf types in a
format that allows for rich keys and simple values.  Rich keys are the core of databuf.

Keys are an ordered set of non-optional, non-repeating fields of an object.  The format is a tuple-like construction.
Each field independently encodes an ordering among values of that field, and the tuple-construction takes care of
providing lexicographic sort across the key-value store.

Each element encodes to its own sorted byte string, and the general key-value format takes care of encoding the rest.
It is sufficien to talk about keys as tuples, much like Python tuples, using notation like `(A, B, C)`.  Or even, `((A,
B), C)`.  The actual conversion matters, but not to this document; for the rest of the document, keys will always be
presented as tuples.

# A General Type System

The databuf type system is about encoding everything into the key, so that rich key-value structures may be made on top
of structures built into the key.  It's invariant that any key can be appeneded to, to make additional, nested keys.
From the key-value store's perspective, this gives locality to the prefix of keys.  From our perspective, it makes it
easy to talk about any data type and know that it's arbitrarily nested inside a typed object.

## Scalar fields, blobs, basically everything terminal

These fields end in data.  There's no more typing beyond describing what the field is.  These fields take the key-form
`(tag,)` where tag is simply the protobuf field number, encoded according to the databuf type system.  The value is the
databuf-encoded form on whatever value corresponds to the tag.

## Maps

Maps are not special within the protobuf encoding, but are a protobuf sugar that many languages provide.  In databuf,
maps take the key-form `(tag, map_key)` so that each map that's chosen to be stored as a map instead of a scalar can
have its individual values accessed and mutated using the properties of the underlying key-value store.

# Security and macaroons

The tuple-orientedness of databuf allows for some neat security tricks.  A prefix (e.g. the first four elements) can be
designated as the security domain for a given value, enabling capability-based security over those prefixes.  To access
anything under the prefix requires that you have been given `(capability,)` for prefix, or a suitably-caveated macaroon.

Overlays work wonderfully with macaroons, allowing for a mount to allow data to inherit from its entwined types.  For
example, consider a core user profile object that gets augmented with an avatar object.  They can share the same key
prefix so that a mount present the two as the same object.  The capabilities necessary to answer the object would then
be the capabilities necessary to access each individual part, but databuf takes care of returning the object.

# Relational Algebra

Checkout databuf/__init__.py for a list of easy-to-support primitives.
