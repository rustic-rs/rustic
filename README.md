# rustic - fast, encrypted, deduplicated backups powered by Rust

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
[![Crates.io Downloads][downloads-image]][crate-link]

Rustic is a backup tool that provides fast, encrypted, deduplicated backups written in [Rust](https://www.rust-lang.org/).
It reads and writes the [restic][1] repo format described in the [design document][2]
and can be used as a restic replacement in most cases.

Rustic supports the major operating systems (Linux, MacOs, *BSD), Windows support is experimental.

Note that rustic currently is in a beta release and misses regression tests.

You can ask questions in the [Discussions][3] or have a look at the [FAQ](docs/FAQ.md)

## Features

- Backup data is deduplicated and encrypted.
- Backup storage can be local or cloud storages, including cold storages.
- Allows multiple clients to concurrently access a backup repository using lock-free operations.
- Backups by default are append-only on the repository.
- The operations are robustly designed and can be safely aborted and efficiently resumed.
- Snapshot organization is possible by hostname, backup paths, label and tags. Also a rich set of metadata is saved with each snapshot.
- Retention policies and cleaning of old backups can be highly customized.
- Follow-up backups only process changed files, but still create a complete backup snapshot.
- In-place restore only modifies files which are changed.
- Can use config files for easy configuration of all every-day commands, see [example config files](config/).

## Quick start

![rustic init](https://github.com/rustic-rs/rustic/blob/main/docs/screenshots/rustic.png?raw=true)

![rustic restore](https://github.com/rustic-rs/rustic/blob/main/docs/screenshots/rustic-restore.png?raw=true)

## Are binaries available?

Sure. Check out the [releases](https://github.com/rustic-rs/rustic/releases).
Binaries for the latest development version are available [here](https://github.com/rustic-rs/rustic-beta).

## What is the difference between rustic and restic?

See the [Comparison between rustic and restic](docs/comparison-restic.md).

## License

Licensed under either of:

- [Apache License, Version 2.0](./LICENSE-APACHE)
- [MIT license](./LICENSE-MIT)

at your option.

### Contribution

Contributions in form of [issues][4] or PRs are very welcome.

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
[3]: https://github.com/rustic-rs/rustic/discussions
[4]: https://github.com/rustic-rs/rustic/issues/new/choose
