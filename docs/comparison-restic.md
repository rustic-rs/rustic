# Comparison rustic vs. restic

Improvements implemented in rustic:

* Allows using cold storage (e.g. AWS Glacier) repos which are only read in the `restore` command + supports warm-up
* All operations are completely lock-free as rustic supports two-phase-pruning (prune option `instant-delete` is available)
* Supports configuration in a config file
([example config files](https://github.com/rustic-rs/rustic/tree/main/config))
* Huge decrease in memory requirement
* Already faster than restic for most operations (but not yet fully speed optimized)
* Cleaner concept of logging output; possibility to write logs to a log file
* `rustic repair` command allows to repair some kinds of broken repositories
* `backup` command can use `.gitignore` files
* `restore` uses existing files; also option `--delete` available
* Snapshots save much more information, available in `snapshots` command
* Integrates the [Rhai](https://rhai.rs/) script language for snapshot filtering
* Allows to save repository options in the repository config file via the command `config`
* New command `merge`
* New command `repo-info`
* `check` command checks and uses cache; option `--trust-cache` is available
* Option `prune --fast-repack` for faster repacking
* Syntax `<SNAPSHOT>[:PATH]` is available for many commands

## Missing points

* [ ] tests and benchmarks
* [ ] missing commands: find, mount
