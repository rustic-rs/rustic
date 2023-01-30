# rustic - fast, encrypted, deduplicated backups powered by pure Rust

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
[![Crates.io Downloads][downloads-image]][crate-link]

Rustic is a backup tool that provides fast, encrypted, deduplicated backups.
It reads and writes the [restic][1] repo format desribed in the [design document][2]
and can therefore be used as a complete replacement for restic.

<img src="https://github.com/rustic-rs/rustic/blob/main/screenshots/rustic.png">
<img src="https://github.com/rustic-rs/rustic/blob/main/screenshots/rustic-restore.png">

Note that rustic currently is in an beta release and misses tests.
It is not yet considered to be ready for use in a production environment.

## Are binaries available?
Sure. Check out the [releases](https://github.com/rustic-rs/rustic/releases).
Binaries for the latest development version are available [here](https://github.com/rustic-rs/rustic-beta).

## Have a question?

Look at the [FAQ][3] or ask in the [Discussions][4]. Also [opening issues][5] is highly welcomed if you want to report something
not working or if you would like to ask for a new feature!

## Comparison with restic:

Improvements:
 * Allows using cold storage (e.g. AWS Glacier) repos which are only read in the `restore` command + supports warm-up
 * All operations are completely lock-free as rustic supoorts two-phase-pruning (prune option `instant-delete` is available)
 * Supports configuration in a config file ([example config files](https://github.com/rustic-rs/rustic/tree/main/examples))
 * Huge decrease in memory requirement
 * Already faster than restic for most operations (but not yet fully speed optimized)
 * Cleaner concept of logging output; posibility to write logs to a log file
 * `rustic repair` command allows to repair some kinds of broken repositories
 * `backup` command can use `.gitignore` files
 * `restore` uses existing files; also option `--delete` available
 * Snapshots save much more information, available in `snapshots` command
 * Allows to save repository options in the repository config file via the command `config`
 * New command `merge`
 * New command `repo-info`
 * `check` command checks and uses cache; option `--trust-cache` is available
 * Option `prune --fast-repack` for faster repacking
 * Syntax `<SNAPSHOT>[:PATH]` is available for many commands
 
Current limitations:
 * Runs so far only on Linux and MacOS, Windows support is WIP
 
## Open points:
 * [ ] Add tests and benchmarks
 * [ ] Add missing commands: find, mount
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
[4]: https://github.com/rustic-rs/rustic/discussions
[5]: https://github.com/rustic-rs/rustic/issues/new/choose
