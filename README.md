# rustic - a restic-compatible backup tool written in pure Rust

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
[![Crates.io Downloads][downloads-image]][crate-link]

Rustic is a backup tool that provides fast, encrypted, deduplicated backups.
It can read the [restic][1] repo format desribed in the [design document][2] and writes a
compatible repo format which can also be read by restic.

Note that rustic currently is in an alpha release and misses functionalities and tests.
It is not yet considered to be ready for use in a production environment.

## Open points:
 * [ ] Add more backends, backup-sources and restore-destinations
 * [ ] Add missing commands
 * [ ] Allow for parallel repo access
 * [ ] Parallelize the code, maybe use async code
 * [ ] Improve error handling
 * [ ] Add tests and benchmarks
 * [ ] Add CI

## Open issues:
 * [ ] restore does not yet restore metadata

## Comparison with restics:

Improvements:
 * Huge decrease in memory requirement
 * Can use .gitignore files
 * Snapshots save total size and node count, this is also shown in the rustic snapshots command
 * cat tree command accepts a snapshot and path to cat the tree blob

Differences:
 * backup uses glob patterns to include/exclude instead of exclude files
 * file/dir permissions have different format in go and rust (but the important information is identical)

Current limitations:
 * No repo initialization implemented; create a repo with restic and use it with rustic
 * Missing commands, e.g.: init, key, mount, copy, dump, find, forget, prune
 * Backup location, backup source and restore destinations only on local file system
 * Runs only on linux
 * No parallel repo access implemented (no locking; duplicate blobs are not supported)
 * Not speed optimized (and no cache implemented)
 * ... and many more
 

## License

Licensed under either of:

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Contributions in form of issues or PRs are very welcome.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.


[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/rustic-rs.svg
[crate-link]: https://crates.io/crates/rustic-rs
[docs-image]: https://docs.rs/rustic-rs/badge.svg
[docs-link]: https://docs.rs/rustic-rs/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[downloads-image]: https://img.shields.io/crates/d/rustic-rs.svg

[//]: # (general links)

[1]: https://github.com/restic/restic
[2]: https://github.com/restic/restic/blob/master/doc/design.rst
