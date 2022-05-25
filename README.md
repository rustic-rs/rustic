# rustic - a restic-compatible backup tool written in pure Rust

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
[![Crates.io Downloads][downloads-image]][crate-link]

Rustic is a backup tool that provides fast, encrypted, deduplicated backups.
It reads and writes the [restic][1] repo format desribed in the [design document][2]
and can therefore be used as a complete replacement for restic.

<img src="https://github.com/rustic-rs/rustic/blob/main/screenshots/rustic.png">

Note that rustic currently is in an beta release and misses tests.
It is not yet considered to be ready for use in a production environment.

## Have a question?

Look at the [FAQ][3] or open an issue!

## Comparison with restics:

Improvements:
 * Completely lock-free
 * Huge decrease in memory requirement
 * Already much faster than restic for most operations (but not yet fully speed optimized)
 * Can use `.gitignore` files
 * Snapshots save much more information
 * New command `repo-info`
 * cat tree command accepts a snapshot and path to cat the tree blob

Differences:
 * backup uses glob patterns to include/exclude instead of exclude files

Current limitations:
 * Backup source and restore destinations only on local file system
 * Backup backends: So far only local disc and REST backends supported (others using rclone as REST backend)
 * Runs so far only on linux; help appreciated to add support for other OSes
 
## Open points:
 * [ ] Add tests and benchmarks
 * [ ] Add CI
 * [ ] Implement a local cache
 * [ ] Add more backends, backup-sources and restore-destinations
 * [ ] Add missing commands: copy, dump, find, mount
 * [ ] Improve error handling
 * [ ] Parallelize the code even more and optimize for speed where useful

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
[3]: https://github.com/rustic-rs/rustic/blob/main/FAQ.md
