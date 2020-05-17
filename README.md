This crate implements a map of unsigned 64-bit keys into strings.

The map is optimized for creating it once, and then reading many times. The struct `Builder` is
used to build the map, and the struct `Map` is used for lookups.

The special property of the implementation is that it encodes all the data needed for the
lookup in a single sequence of bytes.  This makes it rather interesting for dynamic loading of
data that can then be placed in an operating system's read only memory.  The internal structure
requires no decoding when it is loaded (say from a file).

The map is internally represented as a trie with each level of the trie being indexed by a
number of bits of the key, starting from the least-significant bit side.  So for example, when
creating the builder with 2 bits, then 2 bits will be chopped off the provided key for each
descent into the trie by one level.

Example:

```rust
use sequence_map::{Builder, Map};

const BITS: usize = 2;

let mut builder = Builder::new(BITS);

builder.insert(42, "Hello!");

// Note: a second insert under the same key does *not* replace the
// previously inserted key.
builder.insert(42, "Wonderful!");
builder.insert(84, "World!");

// This is the resulting byte sequence.
let bytes: Vec<u8> = builder.build();

// Now, look up some keys.
let lookup = Map::new(&bytes);
assert_eq!("Hello!", lookup.get(42).unwrap());
assert_eq!("World!", lookup.get(84).unwrap());
assert!(lookup.get(100).is_none());
```

> This is not an officially supported Google product.

