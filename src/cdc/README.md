cdc
========

A library for performing *Content-Defined Chunking* (CDC) on data streams. Implemented using generic iterators, very easy to use.

- [API Documentation](https://docs.rs/cdc/)

## Example

```rust
  let reader: BufReader<File> = BufReader::new(file);
  let byte_iter = reader.bytes().map(|b| b.unwrap());

  // Finds and iterates on the separators.
  for separator in SeparatorIter::new(byte_iter) {
    println!("Index: {}, hash: {:016x}", separator.index, separator.hash);
  }
```

Each module is documented via an example which you can find in the `examples/` folder.

To run them, use a command like:

    cargo run --example separator --release

**Note:** Some examples are looking for a file named `myLargeFile.bin` which I didn't upload to Github. Please use your own files for testing.

## What's in the crate

From low level to high level:

* A `RollingHash64` trait, for rolling hash with a 64 bits hash value.

* `Rabin64`, an implementation of the Rabin Fingerprint rolling hash with a 64 bits hash value.

* `Separator`, a struct which describes a place in a data stream identified as a separator.

* `SeparatorIter`, an adaptor which takes an `Iterator<Item=u8>` as input and which enumerates all the separators found.

* `Chunk`, a struct which describes a piece of the data stream (index and size).

* `ChunkIter`, an adaptor which takes an `Iterator<Item=Separator>` as input and which enumerates chunks.

## Implementation details

* The library is not cutting any files, it only provides information on how to do it.

* You can change the default window size used by `Rabin64`, and how the `SeparatorIter` is choosing the separator.

* The design of this crate may be subject to changes sometime in the future. I am waiting for some features of `Rust` to mature up, specially the [`impl Trait`](https://github.com/rust-lang/rust/issues/34511) feature.

## Performance

There is a **huge** difference between the debug build and the release build in terms of performance. Remember that when you test the lib, use `cargo run --release`.

I may try to improve the performance of the lib at some point, but for now it is good enough for most usages.

## License

Coded with ❤️ , licensed under the terms of the [MIT license](LICENSE.txt).
