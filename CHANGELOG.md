# Changelog

All notable changes to this project will be documented in this file.

## [0.9.6](https://github.com/rustic-rs/rustic/compare/v0.9.5...v0.9.6) - 2025-03-20

### Added

- push prometheus metrics to pushgateway on-demand ([#1404](https://github.com/rustic-rs/rustic/pull/1404))
- Allow to use OPENDAL_HOT_* and OPENDAL_COLD_* env variables ([#1392](https://github.com/rustic-rs/rustic/pull/1392))

### Fixed

- *(commands)* remove SIGPIPE default handler ([#1431](https://github.com/rustic-rs/rustic/pull/1431))
- fix clippy lints
- *(config)* set a non-zero default progress interval for progress options ([#1378](https://github.com/rustic-rs/rustic/pull/1378))

### Other

- *(deps)* bump zip from 2.2.1 to 2.3.0 ([#1432](https://github.com/rustic-rs/rustic/pull/1432))
- *(deps)* bump ring from 0.17.8 to 0.17.13 ([#1424](https://github.com/rustic-rs/rustic/pull/1424))
- Add Homebrew instructions ([#1416](https://github.com/rustic-rs/rustic/pull/1416))

## [0.9.5](https://github.com/rustic-rs/rustic/compare/v0.9.4...v0.9.5) - 2024-12-02

### Added

- *(commands)* More dump options ([#1339](https://github.com/rustic-rs/rustic/pull/1339))
- shut down gracefully with ctrl+c ([#1364](https://github.com/rustic-rs/rustic/pull/1364))
- Add --filter-jq option ([#1372](https://github.com/rustic-rs/rustic/pull/1372))
- *(commands)* Add `mount` command ([#973](https://github.com/rustic-rs/rustic/pull/973))
- Error messages are now much improve 
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.6.0))

### Fixed

- *(commands)* run backup hooks before checking source dir ([#1374](https://github.com/rustic-rs/rustic/pull/1374))
- *(commands)* Use spawn_blocking in webdav when calling rustic_core ([#1365](https://github.com/rustic-rs/rustic/pull/1365))
- *(forget)* Add minutely timeline
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.7.2))
- *(init)* Prevent overwriting hot repository
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.6.0))

### Other

- update snapshots to include minutely configuration options
- *(deps)* update rustic_core, bytes, and libc dependencies to latest versions
- simplify lifetime annotations in OpenFileReader and TreeIterItem implementations
- clean up whitespace and update clippy linting allowances
- *(deps)* update dependencies to latest versions
- *(deps)* update lockfile to get rid of vulnerable `url` version
- *(mount)* rename fields for clarity, add user options for mount ([#1353](https://github.com/rustic-rs/rustic/pull/1353))
- *(deps)* update dependencies
- *(deps)* don't use rustic_core webdav feature ([#1367](https://github.com/rustic-rs/rustic/pull/1367))
- move `webdavfs` from `rustic_core` to `rustic-rs` ([#1363](https://github.com/rustic-rs/rustic/pull/1363))
- *(clippy)* comment out unused lints in lib.rs
- *(clippy)* apply fixes automatically
- use BTreeMap for env in global options ([#1360](https://github.com/rustic-rs/rustic/pull/1360))
- add tiny framework for testing rustic's compat with latest restic ([#1303](https://github.com/rustic-rs/rustic/pull/1303))
- use snapshot tests for default config, show-config and completions ([#1359](https://github.com/rustic-rs/rustic/pull/1359))
- *(deps)* update dependencies rustic_core, rustic_backend, rustic_testing, and migrate to conflate 0.3 ([#1357](https://github.com/rustic-rs/rustic/pull/1357))
- fix typos
- *(build)* add platform-dependent settings and remove ci flag for extra features
- clarify `--use-profile` command in config by using long form ([#1344](https://github.com/rustic-rs/rustic/pull/1344))
- *(deps)* update core and testing crates ([#1340](https://github.com/rustic-rs/rustic/pull/1340))

## [0.9.4](https://github.com/rustic-rs/rustic/compare/v0.9.3...v0.9.4) - 2024-10-24

### Added

- *(commands)* Add tar output to dump command ([#1328](https://github.com/rustic-rs/rustic/pull/1328))

### Fixed

- clippy lints for new Rust version ([#1329](https://github.com/rustic-rs/rustic/pull/1329))
- *(deps)* downgrade self-update to fix problems finding right target ([#1323](https://github.com/rustic-rs/rustic/pull/1323))

### Other

- *(deps)* remove once_cell and replace with std::sync::LazyLock, increase MSRV to 1.80.0 ([#1337](https://github.com/rustic-rs/rustic/pull/1337))
- *(deps)* update tokio, ratatui, and tui-textarea ([#1336](https://github.com/rustic-rs/rustic/pull/1336))
- *(deps)* update rustic_core and rustic_backend ([#1334](https://github.com/rustic-rs/rustic/pull/1334))
- *(deps)* update abscissa framework ([#1330](https://github.com/rustic-rs/rustic/pull/1330))
- introduce a new feature 'release' that includes the 'self-update' feature ([#1307](https://github.com/rustic-rs/rustic/pull/1307))

## [0.9.3](https://github.com/rustic-rs/rustic/compare/v0.9.2...v0.9.3) - 2024-10-10

### Fixed

- *(deps)* update rustic_core to version 0.5.3 ([#1314](https://github.com/rustic-rs/rustic/pull/1314))

### Other

- add status badge for docker image build and shorten workflow name ([#1311](https://github.com/rustic-rs/rustic/pull/1311))

## [0.9.2](https://github.com/rustic-rs/rustic/compare/v0.9.1...v0.9.2) - 2024-10-09

### Added

- *(config)* Add hooks ([#1218](https://github.com/rustic-rs/rustic/pull/1218))

### Other

- *(deps)* update rustic_core ([#1309](https://github.com/rustic-rs/rustic/pull/1309))
- build and publish docker image on release ([#1297](https://github.com/rustic-rs/rustic/pull/1297))

## [0.9.1](https://github.com/rustic-rs/rustic/compare/v0.9.0...v0.9.1) - 2024-10-03

### Added

- *(config)* add more filters ([#1263](https://github.com/rustic-rs/rustic/pull/1263))
- *(check)* Allow to only check trees+packs for given snapshots ([#1230](https://github.com/rustic-rs/rustic/pull/1230))
- *(commands)* add a `docs` command to easily access the user, dev and config documentation ([#1276](https://github.com/rustic-rs/rustic/pull/1276))

### Fixed

- *(docs/cli)* improve the descriptions of the CLI commands ([#1277](https://github.com/rustic-rs/rustic/pull/1277))
- *(deps)* update rustic_core and other dependencies and fix merge precedence ([#1282](https://github.com/rustic-rs/rustic/pull/1282))
- *(docs)* update configuration documentation to align with recent changes ([#1280](https://github.com/rustic-rs/rustic/pull/1280))

### Other

- *(deps)* upgrade dependencies ([#1289](https://github.com/rustic-rs/rustic/pull/1289))
- add triage label to new issues only if no label has been set when creating it ([#1287](https://github.com/rustic-rs/rustic/pull/1287))
- *(interactive)* use update methods for refreshing snapshots ([#1285](https://github.com/rustic-rs/rustic/pull/1285))

## [0.9.0](https://github.com/rustic-rs/rustic/compare/v0.8.1...v0.9.0) - 2024-09-29

### Bug Fixes

- [**breaking**] use multiple options only as array in config profile
  ([#1240](https://github.com/rustic-rs/rustic/pull/1240))
- Allow snapshots to be modified and marked to forget
  ([#1253](https://github.com/rustic-rs/rustic/pull/1253))
- make ls and find show the year of mtime date
  ([#1249](https://github.com/rustic-rs/rustic/pull/1249))
- ls: Remove printing trailing space
  ([#1247](https://github.com/rustic-rs/rustic/pull/1247))
- webdav/forget: correctly use application config
  ([#1241](https://github.com/rustic-rs/rustic/pull/1241))

### Features

- [**breaking**] copy: Use config profile as target
  ([#1131](https://github.com/rustic-rs/rustic/pull/1131))
- backup: Add option `stdin-command`
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- Add list indexpacks and list indexcontent commands
  ([#1254](https://github.com/rustic-rs/rustic/pull/1254))
- Add option `--only-identical` for `diff` to allow for bitrot check
  ([#1250](https://github.com/rustic-rs/rustic/pull/1250))
- ls: Add option --json ([#1251](https://github.com/rustic-rs/rustic/pull/1251))
- backup: Add option `--long`
  ([#1159](https://github.com/rustic-rs/rustic/pull/1159))

### Documentation

- update installation instructions in readme to use `--locked` flag for install
  from crates.io
- update RepositoryErrorKind rustdoc following rustic_core change
  ([#1237](https://github.com/rustic-rs/rustic/pull/1237))

### Other

- Remove self-update from default crate features
  ([#1139](https://github.com/rustic-rs/rustic/pull/1139))
- Reduce memory usage of restore
  ([#1069](https://github.com/rustic-rs/rustic/pull/1069))
- *(deps)* update rust crate libc to v0.2.159
  ([#1257](https://github.com/rustic-rs/rustic/pull/1257))
- *(deps)* lock file maintenance
  ([#1269](https://github.com/rustic-rs/rustic/pull/1269))
- *(deps)* update rust crate rstest to 0.23
  ([#1267](https://github.com/rustic-rs/rustic/pull/1267))
- *(deps)* update rust crate tempfile to v3.13.0
  ([#1266](https://github.com/rustic-rs/rustic/pull/1266))
- *(deps)* update marcoieni/release-plz-action digest to 8b0f89a
  ([#1265](https://github.com/rustic-rs/rustic/pull/1265))
- *(deps)* update embarkstudios/cargo-deny-action action to v2
  ([#1259](https://github.com/rustic-rs/rustic/pull/1259))
- *(deps)* update rustsec/audit-check action to v2
  ([#1260](https://github.com/rustic-rs/rustic/pull/1260))
- *(deps)* update softprops/action-gh-release action to v2
  ([#1258](https://github.com/rustic-rs/rustic/pull/1258))
- *(deps)* update embarkstudios/cargo-deny-action digest to 3f4a782
  ([#1228](https://github.com/rustic-rs/rustic/pull/1228))

## [0.8.1] - 2024-09-08

### Bug Fixes

- Allow to compile without tui feature
  ([#1208](https://github.com/rustic-rs/rustic/issues/1208))
- Use cargo --locked in CI pipeline
  ([#1207](https://github.com/rustic-rs/rustic/issues/1207))
- Return exitcode ([#1220](https://github.com/rustic-rs/rustic/issues/1220))
- "Incorrect Password" error is now only shown if password is really incorrect.
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.1))
- Group by now works as expected
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.1))
- A bug in `keep-tags` and `filter-tags` has been fixed.
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.1))
- Building OpenBSD platform target is now possible again
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.1))

### Documentation

- Update config profile readme
  ([#1221](https://github.com/rustic-rs/rustic/issues/1221))

### Features

- Add autocompletion hints
  ([#1225](https://github.com/rustic-rs/rustic/issues/1225))
- Allow to modify filters
  ([#1210](https://github.com/rustic-rs/rustic/issues/1210))
- Allow to view text files
  ([#1216](https://github.com/rustic-rs/rustic/issues/1216))

### Generated

- Updated Completions fixtures

### Miscellaneous Tasks

- Bump quinn-proto from 0.11.6 to 0.11.8
  ([#1223](https://github.com/rustic-rs/rustic/issues/1223))
- Dependency updates ([#1227](https://github.com/rustic-rs/rustic/issues/1227))

## [0.8.0] - 2024-08-21

### Bug Fixes

- Add comments for owncloud and nextcloud dependent settings
- Rename service examples
- Ask for password in backup and copy command if it is missing
  ([#1061](https://github.com/rustic-rs/rustic/issues/1061))
- Ask for missing password in copy when initializing
  ([#1063](https://github.com/rustic-rs/rustic/issues/1063))
- Fix possible overflow in progress bar ETA
  ([#1079](https://github.com/rustic-rs/rustic/issues/1079))
- Correct b2.toml ([#1072](https://github.com/rustic-rs/rustic/issues/1072))
- Show log filename if open/creation failed
  ([#1111](https://github.com/rustic-rs/rustic/issues/1111))
- [**breaking**] Multiple paths in config profile as array
  ([#1124](https://github.com/rustic-rs/rustic/issues/1124))
- Respect delete-protection when running forget with ids
  ([#1149](https://github.com/rustic-rs/rustic/issues/1149))
- Reset terminal no matter what
  ([#1175](https://github.com/rustic-rs/rustic/issues/1175))
- Allow missing fields in snapshot summary (to support restic 0.17.0)
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- Allow non-value/null xattr fields
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- Backup file if listing xattrs fails
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- limit memory usage for restore when having large pack files
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- prune: correct number of packs to repack
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))

### Features

- [**breaking**] Show-config now outputs toml
  ([#1095](https://github.com/rustic-rs/rustic/issues/1095))
- [**breaking**] Allow specifying many options in config profile without array
  ([#1130](https://github.com/rustic-rs/rustic/issues/1130))
- Add interactive snapshots mode
  ([#1114](https://github.com/rustic-rs/rustic/issues/1114))
- The find command has been added
  ([#1136](https://github.com/rustic-rs/rustic/issues/1136))
- Allow setting extra repository options via env variables
  ([#1081](https://github.com/rustic-rs/rustic/issues/1081))
- Add --check-index option
  ([#1078](https://github.com/rustic-rs/rustic/issues/1078))
- Add extra check before writing data and add --set-extra-check config option
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- Add append-only repository mode
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- forget: Enforce to have a --keep-* option and add --keep-none.
  ([rustic_core](https://github.com/rustic-rs/rustic_core/releases/tag/rustic_core-v0.3.0))
- Add s3_idrive config and move configs to a services subdirectory
  ([#1048](https://github.com/rustic-rs/rustic/issues/1048))
- Add example config for owncloud and nextcloud
  ([#1052](https://github.com/rustic-rs/rustic/issues/1052))
- Use human-panic to print better error messages in case rustic panics
  ([#1065](https://github.com/rustic-rs/rustic/issues/1065))
- Prune: Add more debug output
  ([#1064](https://github.com/rustic-rs/rustic/issues/1064))
- Add interactive ls mode
  ([#1117](https://github.com/rustic-rs/rustic/issues/1117))
- Add interactive restore
  ([#1123](https://github.com/rustic-rs/rustic/issues/1123))
- Interactive Ls: remember parent position
  ([#1126](https://github.com/rustic-rs/rustic/issues/1126))
- Use RFC3339 time format in logfile
  ([#1133](https://github.com/rustic-rs/rustic/issues/1133))
- Add possibility to change snapshot description
  ([#1137](https://github.com/rustic-rs/rustic/issues/1137))
- Interactive: Allow to delete snapshots
  ([#1143](https://github.com/rustic-rs/rustic/issues/1143))
- Interactive: Prompt before exiting
  ([#1146](https://github.com/rustic-rs/rustic/issues/1146))
- Document opendal options connections and throttle
- Add better progress bars
  ([#1152](https://github.com/rustic-rs/rustic/issues/1152))
- Show diff statistics
  ([#1178](https://github.com/rustic-rs/rustic/issues/1178))

### Documentation

- Update configuration README
  ([#1088](https://github.com/rustic-rs/rustic/issues/1088))
- Fix typo in find.rs ([#1187](https://github.com/rustic-rs/rustic/issues/1187))

### Miscellaneous Tasks

- Fix cargo-binstall metadata
- Move rustic_testing into rustic_core
- Break old ci jobs when new commits are pushed so we don't fill up the queue
- Bump mio from 0.8.10 to 0.8.11
  ([#1089](https://github.com/rustic-rs/rustic/issues/1089))
- Update deps and adapt to rustic_core changes
- Bump h2 from 0.3.25 to 0.3.26
  ([#1113](https://github.com/rustic-rs/rustic/issues/1113))
- Bump rustls from 0.21.10 to 0.21.11
  ([#1127](https://github.com/rustic-rs/rustic/issues/1127))
- Update rustic_core and rustic_backend
  ([#1201](https://github.com/rustic-rs/rustic/issues/1201))

### Testing

- Replace missing crates folder with src
- Refactor integration tests to assert_cmd and predicates, test all configs in
  config subdirectory ([#1060](https://github.com/rustic-rs/rustic/issues/1060))

## [0.7.0] - 2024-02-03

### Packaging

- Enable RPM file build target
  ([#951](https://github.com/rustic-rs/rustic/issues/951))

### Bug Fixes

- Remove unmaintained `actions-rs` ci actions
- Remove unmaintained `actions-rs/cargo` ci action with cross.
- Remove unmaintained `actions-rs/toolchain` ci action
- Log config file logs after reading config files
  ([#961](https://github.com/rustic-rs/rustic/issues/961))
- Fix progress for copy command
  ([#965](https://github.com/rustic-rs/rustic/issues/965))
- Enable abscissa_core testing feature only for dev
  ([#976](https://github.com/rustic-rs/rustic/issues/976))
- Update github action to download artifacts, as upload/download actions from
  nightly workflow were incompatible with each other
- Update rust crate duct to 0.13.7
  ([#991](https://github.com/rustic-rs/rustic/issues/991))
- Update rust crate libc to 0.2.151
  ([#992](https://github.com/rustic-rs/rustic/issues/992))
- Diff: Add local: to path syntax
  ([#1000](https://github.com/rustic-rs/rustic/issues/1000))
- Update rust crate libc to 0.2.152
  ([#1016](https://github.com/rustic-rs/rustic/issues/1016))
- Error handling when entering passwords
  ([#963](https://github.com/rustic-rs/rustic/issues/963))
- Use hyphen in cli api for numeric-uid-gid

### Documentation

- Update changelog
- Fix new lines in changelog
- Update changelog

### Features

- Add --quiet option to backup and forget
  ([#964](https://github.com/rustic-rs/rustic/issues/964))
- Allow building without self-update feature
  ([#975](https://github.com/rustic-rs/rustic/issues/975))
- Add option --numeric-uid-gid to ls
  ([#1019](https://github.com/rustic-rs/rustic/issues/1019))
- Add colors to help texts
  ([#1007](https://github.com/rustic-rs/rustic/issues/1007))
- Add webdav command ([#1024](https://github.com/rustic-rs/rustic/issues/1024))

### Generated

- Updated Completions fixtures

### Miscellaneous Tasks

- Run actions that need secrets.GITHUB_TOKEN only on rustic-rs org
- Update dtolnay/rust-toolchain
- Update taiki-e/install-action
- Update rustsec/audit-check
- Netbsd nightly builds fail due to missing execinfo, so we don't build on it
  for now
- Upgrade dprint config
- Activate automerge for github action digest update
- Activate automerge for github action digest update
- Automerge lockfile maintenance
- Try to fix nightly build
- Display structure of downloaded artifact files
- Display structure of downloaded artifact files II
- Release
- Do not run twice on release branches
- Remove release workflow and fix release continuous deployment
- Run on tag push
- Add release candidates to CD
- Remove conditional for checking tags
- Fix path for release files for CD
- Fix path for release files for CD, second approach with full file name
- Fix binstall pkg-url
- Use tag version in directory names for automation to download new versions
- Set `max-parallel` to 1 for build matrix
- Replace max-parallel with an own job

### Refactor

- Adjust to changes in rustic_core for added rustic_backend
  ([#966](https://github.com/rustic-rs/rustic/issues/966))

### Testing

- Add missing powershell profile to completions test

### Build

- Bump zerocopy from 0.7.25 to 0.7.31
  ([#967](https://github.com/rustic-rs/rustic/issues/967))
- Bump h2 from 0.3.22 to 0.3.24
  ([#1009](https://github.com/rustic-rs/rustic/issues/1009))

### Diff

- Improve code (better lifetime handling)

### Ls

- Add alternative option name --numeric-id

## [0.6.0] - 2023-10-23

### Breaking Changes

- We refactored to
  [`rustic_core`](https://www.github.com/rustic-rs/rustic_core). This means that
  most of the underlying logic can now be used as a library. The CLI is now a
  thin wrapper around the library. This also means that the CLI is now much more
  customizable. Please check out the
  [documentation](https://rustic.cli.rs/docs/getting_started.html) for more
  information.

### Bug Fixes

- Retrying backend access didn't work for long operations. This has been fixed
  (and retries are now customizable)
- Prune did abort when no time was set for a pack-do-delete. This case is now
  handled correctly.
- The zstd compression library led to data corruption in very unlikely cases.
  This has been fixed by a dependency update.
- The glob option did only work with absolute files. This has been fixed.
- Non-unicode link targets are now correctly handled on Unix (after this has
  been added to the restic repo format).
- The `--dry-run` option now works as expected in the `init` command.

### Features

- New global configuration paths are available, located at `/etc/rustic/*.toml`
  or `%PROGRAMDATA%/rustic/config/*.toml`, depending on your platform.
- REST backend: Now allows to use custom TLS root certificates.
- Environment variables for programs called by rustic can now be set in the
  config files.
- Creation of new keys now enforces confirmation of entered key. This helps to
  prevent mistype of passwords during the initial entry
- Wait for password-command to successfully exit, allowing to input something
  into the command, and read password from stdout.
- backup: New option --init to initialize repository if it doesn't exist yet.
- backup: New option `no-require-git` - if enabled, a git repository is not
  required to apply `git-ignore` rule.
- restore: The restore algorithm has been improved and should now be faster for
  remote repositories.
- restore: Files are now allocated just before being first processed. This
  allows easier resumed restores.
- repoinfo: Added new options --json, --only-files, --only-index.
- check: Add check if time is set for packs-to-delete.
- ls: Options --long (-l) and --summary (-s) have been added.
- forget: Option --json has been added.

## [0.5.4] - 2023-06-05

### Bug Fixes

- backup crashed when there was a non-unicode link target. The crash has been
  fixed. However, non-unicode link targets are still unsupported.
- Extended attributes which were saved with value null couldn't be handled. This
  has been fixed.
- prune: --max-repack didn't work with a given percentage of repo size. This has
  been fixed.

### Features

- copy: Added --init option to initialize uninitialized target repos
- dependencies have been updated

### Miscellaneous Tasks

- Bump serde_with from 2.3.2 to 2.3.3
- Bump clap from 4.2.4 to 4.2.5
- Bump reqwest from 0.11.16 to 0.11.17

### Backup

- Don't crash on non-unicode link targets

### Comparison-restic

- Fix typo

### Copy

- Add --init option

### Prune

- Fix --max-repack

### Xattrs

- Allow null value in JSON

## [0.5.3] - 2023-04-25

### Breaking Changes

- config file: use-config now expects an array of config profiles to read.

### Bug Fixes

- The [[backup.sources]] section in the config file was ignored 0.5.2. This has
  been fixed.

### Features

- The show-config command has been added.

### Backup

- Fix omitting sources config from the config file

## [0.5.2] - 2023-04-24

### Breaking Changes

- The CLI option `--config-profile` was renamed into `--use-profile` (same
  shortcut `-P`).

### Bug Fixes

- restore: Warm-up options given by the command line didn't work. This has been
  fixed.
- backup: showed 1 dir as changed when backing up without parent. This has been
  fixed.
- diff: The options --no-atime and --ignore-devid had no effect and are now
  removed.
- Rustic's check of additional fields in the config file didn't work in edge
  cases. This has been fixed.

### Features

- backup: Backing up (small) files has been speed-optimized and is now much more
  parallelized.
- Config file: New field use-profile under [global] allows to merge options from
  other config profiles
- Option --dry-run is now a global option and can also be defined in the config
  file or via env variable
- forget: Using "-1" as value for --keep-* options will keep all snapshots of
  that interval
- prune: Added option --repack-all

### Documentation

- Add config file containing all options

### Miscellaneous Tasks

- Bump h2 from 0.3.16 to 0.3.17
- Bump aho-corasick from 0.7.20 to 1.0.0
- Bump clap from 4.2.2 to 4.2.3
- Bump clap from 4.2.3 to 4.2.4
- Bump dunce from 1.0.3 to 1.0.4
- Bump libc from 0.2.141 to 0.2.142
- Bump clap_complete from 4.2.0 to 4.2.1
- Bump aho-corasick from 1.0.0 to 1.0.1
- Parallelize processing (especially for small files)

### Backup

- Fix dir stats

### Diff

- Remove unwanted options

### Forget

- Interpret '--keep-* -1' as 'keep all'

### Prune

- Add option --repack-all

## [0.5.1] - 2023-04-13

### Breaking Changes

- ls: Added option `--recursive`, note: default is now non-recursive if a path
  is given.

### Bug Fixes

- Fixed compilation on OpenBSD.
- Fixed shell completions.
- REST backend displayed the connection password in the log. This has been
  changed.
- restore: Existing symlinks displayed an error. This is now corrected if the
  `--delete` option is used.
- restore: Setting ownership/permissons/times for symlinks failed. This has been
  fixed.
- Spaces in paths did not work when given in the config file. This has been
  fixed.
- backup --stdin-filename did not use the given filename. This has been fixed.
- backup always displayed at least 1 dir as changed. This has been corrected.
- Windows: Backup of the path prefix (e.g. C: -> C/) did not work. This has been
  fixed.

### Features

- REST backend: Set User-Agent header.
- ls: Added option `--recursive`.
- ls: Added glob options to exclude/include.
- restore: Added glob options to exclude/include.
- restore: xattrs treatment has been improved.
- Dependencies have been updated.

### Miscellaneous Tasks

- Bump serde_json from 1.0.94 to 1.0.95
- Bump reqwest from 0.11.15 to 0.11.16
- Bump serde from 1.0.158 to 1.0.159
- Bump serde-aux from 4.1.2 to 4.2.0
- Bump libc from 0.2.140 to 0.2.141
- Bump filetime from 0.2.20 to 0.2.21
- Bump serde_with from 2.3.1 to 2.3.2
- Bump serde from 1.0.159 to 1.0.160
- Bump serde_json from 1.0.95 to 1.0.96

### Windows

- Backup path prefix

### Backup

- Allow to treat whitespaces in paths in config file
- Fix --stdin-filename
- Only show changed dirs if there are changes

### Ls

- Add option --recursive

### Repository

- Use location in log

### Restore

- Treat all existing contents correctly
- Add glob options to include/exclude patterns
- Don't follow symlinks when setting time/modes

### Restore/ls

- Add glob options to include/exclude patterns

### Restore/xattr

- Improve implementation and errors

## [0.5.0] - 2023-03-24

### Breaking Changes

- Repository options in the config file can no longer be given under the
  `[global]` section. Use `[repository]` instead.
- Backing up multiple sources on the command line now results in one instead of
  several snapshots.

### Bug Fixes

- `restore` command did not restore empty files. This is fixed.
- `config` command did save the config file compressed which violates the repo
  design. This is fixed.
- rustic did panic when files with missing `content` field are stored in a tree.
  This is fixed.

### Features

- Experimental windows support has been added.
- New option --filter-fn allows to implement your own snapshot filter using the
  Rhai language.
- New command dump has been added.
- New command merge has been added.
- Support for extended file attributes has been added.
- REST/Rclone backend: Allow to set the request timeout.
- Extra or wrong fields in the config file now lead to rustic complaining and
  aborting.
- New option --no-progress has been added.
- Option --progress-interval can now also be given as command argument and in
  the config file.
- backup: Paths are now sanitized from command arguments and config file before
  matching and applying the configuration.
- restore: Add --no-ownership option
- check --read-data: progress bar now also shows total bytes to check and ETA.
- The archiver implementation has been reworked. This will allow more backup
  sources in future.
- Updated to Rust 1.68 and many dependency updates

### Miscellaneous Tasks

- Bump simplelog from 0.12.0 to 0.12.1
- Bump rayon from 1.6.1 to 1.7.0
- Bump serde_json from 1.0.93 to 1.0.94
- Bump thiserror from 1.0.38 to 1.0.39
- Bump serde from 1.0.152 to 1.0.153
- Bump serde from 1.0.153 to 1.0.154
- Bump libc from 0.2.139 to 0.2.140
- Bump serde_with from 2.2.0 to 2.3.1
- Bump scrypt from 0.10.0 to 0.11.0
- Bump chrono from 0.4.23 to 0.4.24
- Bump semver from 1.0.16 to 1.0.17
- Bump toml from 0.7.2 to 0.7.3
- Bump serde from 1.0.154 to 1.0.156
- Bump enum-map from 2.4.2 to 2.5.0
- Bump walkdir from 2.3.2 to 2.3.3
- Bump directories from 4.0.1 to 5.0.0
- Bump rstest from 0.16.0 to 0.17.0
- Bump dirs from 4.0.0 to 5.0.0

### Windows

- Allow repos to start with drive letter

### Archiver

- Rework implementation

### Backup

- Fix problem with multiple sources in config
- Separate creating of common snapshot info

### Config

- Save config file uncompressed

### Keyfile

- Use serde_with::base64

### Merge

- Respect delete-never and delete-after options
- Set timestamp

### Restore

- Add --no-ownership option
- Fix restoring of empty files

### Windows

- Treat UNC paths
- Treat path prefixes

## [0.4.4] - 2023-02-28

### Bug Fixes

- Integrated the cdc crate as it currently doesn't compile with current Rust.
  This allows to upload rustic to crates.io.
- restore: Don't abort on errors, but print a warning and continue
- REST backend now ignores extra files in repository, as local backend does.
- init did not work for hot/cold repos. This is fixed.
- A password file without a newline didn't work. This is fixed.
- Removed error in case of password in file not ending with \n

### Features

- diff/restore: Allow to use a single file as target and treat it correctly
- local backend: Added possibility to add hooks. This can be used e.g. to
  automatically generate .par2 files for your local repo.
- backup: Added option --json
- The chunker implementation has been optimized
- Default grouping now includes grouping by labels
- Added OpenBSD as platform
- Many version updates of dependencies

### Miscellaneous Tasks

- Bump nix from 0.26.1 to 0.26.2
- Bump reqwest from 0.11.13 to 0.11.14
- Bump toml from 0.5.10 to 0.5.11
- Bump toml from 0.5.11 to 0.7.0
- Bump toml from 0.7.0 to 0.7.1
- Bump bytes from 1.3.0 to 1.4.0
- Bump zstd from 0.12.2+zstd.1.5.2 to 0.12.3+zstd.1.5.2
- Bump tokio from 1.24.1 to 1.25.0
- Bump anyhow from 1.0.68 to 1.0.69
- Bump binrw from 0.10.0 to 0.11.1
- Bump serde_json from 1.0.91 to 1.0.92
- Bump toml from 0.7.1 to 0.7.2
- Bump filetime from 0.2.19 to 0.2.20
- Bump serde_json from 1.0.92 to 1.0.93
- Bump self_update from 0.34.0 to 0.35.0
- Bump self_update from 0.35.0 to 0.36.0
- Bump bytesize from 1.1.0 to 1.2.0
- Bump base64 from 0.20.0 to 0.21.0

### REST

- Use only valid ids when listing names

### Backup

- Add option --json

### Chunker

- Optimizations

### Diff/restore

- Treat single file destination properly

### Group-by

- Default to host,label,path

### Helpers

- Remove unnecessay mut

### Init

- Fix creating hot/cold repo

### Restore

- Don't abort on delete errors

## [0.4.3] - 2023-01-17

### Bug Fixes

- A bug in `prune` could lead to removal of needed data in the case of duplicate
  blobs within one pack. This is fixed.
- An inaccuracy in the packer could lead to identical blobs saved within the
  same pack. This is fixed.
- check: Reported errors when the cache contained more pack files than the
  repository. This is fixed.
- password-command didn't work correctly when calling a shell with an argument.
  This is fixed.

### Features

- warm-up options can now be configured in the config file.
- repair index: Added better debug output and error handling.
- Added better error handling when opening a repository.
- Improved allocations when parsing/printing ids.

### Miscellaneous Tasks

- Bump ignore from 0.4.18 to 0.4.19
- Bump serde_with from 2.1.0 to 2.2.0
- Bump zstd from 0.12.1+zstd.1.5.2 to 0.12.2+zstd.1.5.2
- Bump nom from 7.1.2 to 7.1.3

### Packer

- Add checks to avoid saving duplicate blobs

### Prune

- Fix check for needed packs

### Repair

- Better debug info and error handling
- Add more checks for edge cases

### Repository

- Integrate warm-up options

## [0.4.2] - 2023-01-04

### Bug Fixes

- rclone backend did not work with unexpected version output. This is now fixed,
  also support for rclone > 1.61 is added.
- restore: restore with existing files/dirs but wrong type did not succeed. This
  is fixed now.
- All command except `backup` and `prune` did not compress snapshot and index
  files, even for v2 repos. This is now fixed.

### Features

- Added the `copy` command: Many targets are supported and a nice output table
  shows which snapshots are to be copied. See also #358.
- The syntax <SNAPSHOT>:<PATH> now also works if <PATH> is a file, e.g. in the
  `restore` command.
- restore: Existing files with correct size and mtime are not read by default;
  new option --verify-existing.
- restore: Improved output of what restore is about to do (also in --dry-run
  mode).
- diff: Make output more similar to `restic diff`; added option `--metadata`.
- diff: When diffing with a local dir, local files are now read and the content
  is compared; new option --no-content.
- backup: Improved parallelization.
- Updated to Rust 1.66 and many updates of dependent crate versions.
- Some minor code and performance improvements.

### Miscellaneous Tasks

- Bump serde from 1.0.148 to 1.0.149
- Bump zstd from 0.12.0+zstd.1.5.2 to 0.12.1+zstd.1.5.2
- Bump filetime from 0.2.18 to 0.2.19
- Bump rayon from 1.6.0 to 1.6.1
- Bump serde from 1.0.149 to 1.0.150
- Bump base64 from 0.13.1 to 0.20.0
- Bump toml from 0.5.9 to 0.5.10
- Bump serde from 1.0.150 to 1.0.151
- Bump semver from 1.0.14 to 1.0.16
- Bump enum-map from 2.4.1 to 2.4.2
- Bump serde_json from 1.0.89 to 1.0.91
- Bump enum-map-derive from 0.10.0 to 0.11.0
- Bump thiserror from 1.0.37 to 1.0.38
- Bump anyhow from 1.0.66 to 1.0.68
- Bump libc from 0.2.138 to 0.2.139
- Bump serde from 1.0.151 to 1.0.152
- Bump self_update from 0.32.0 to 0.33.0
- Bump self_update from 0.33.0 to 0.34.0
- Bump comfy-table from 6.1.3 to 6.1.4

### Backup

- Use rayon to parallelize hashing

### Diff

- Add options --metadata and --no-content

### Restore

- Overwork treatment of existing files
- Rename option --ignore-mtime into --verify-existing

## [0.4.1] - 2022-12-03

### Bug Fixes

- Fixed a possible deadlock in the archiver which could cause `rustic backup` to
  hang.
- Piping output no longer panices (this allows e.g. to pipe into `head`).
- Fixed progress bar showing 0B/s instead of real rate.

### Features

- backup: Errors reading the parent now print a warning instead of being
  silently ignored.
- forget: Allow to keep quarter- and half-yearly.
- Improved the error handling for some situations.

### DecryptBackend

- Better error handling

### Miscellaneous Tasks

- Bump zstd from 0.11.2+zstd.1.5.2 to 0.12.0+zstd.1.5.2
- Bump rpassword from 7.1.0 to 7.2.0
- Bump rstest from 0.15.0 to 0.16.0
- Bump serde from 1.0.147 to 1.0.148
- Bump nix from 0.25.0 to 0.26.1
- Bump gethostname from 0.4.0 to 0.4.1

### Build.sh

- Add optional parameters

### Forget

- Add options to keep snapshots quarter-yearly and half-yearly

### Parent

- Improve error handling

### Snapshots

- Simplify grouping

## [0.4.0] - 2022-11-23

### Bug Fixes

- Fixed a bug in the CI which sometimes made building beta executables fail.

### Features

- Snapshots now allow to use a label, to add a description and save the program
  version used.
- diff: diff can now compare snapshots with local dirs.
- backup: Added option --as-path.
- backup: Allow to use and save relative paths.
- backup: Added option --ignore-devid.
- backup: Now uses more parallelization.
- prune: Repacking is now parallel.
- New commands repair index/snapshots.
- Better support for using latest as snapshot.
- UI/progress bars: Added support for env variable RUSTIC_PROGRESS_INTERVAL.
- Simplified the code in some places.

### Other Changes

- rustic no longer uses async Rust.
- Replaced prettytables by comfytable. (Thanks @JMarkin)

### CI

- Fix typo

### Miscellaneous Tasks

- Bump serde_json from 1.0.85 to 1.0.86
- Bump gethostname from 0.2.3 to 0.3.0
- Bump path-absolutize from 3.0.13 to 3.0.14
- Bump async-trait from 0.1.57 to 0.1.58
- Bump serde_json from 1.0.86 to 1.0.87
- Bump rpassword from 7.0.0 to 7.1.0
- Bump anyhow from 1.0.65 to 1.0.66
- Bump filetime from 0.2.17 to 0.2.18
- Bump serde from 1.0.145 to 1.0.147
- Bump base64 from 0.13.0 to 0.13.1
- Bump clap from 3.2.22 to 3.2.23
- Bump gethostname from 0.3.0 to 0.4.0
- Bump serde-aux from 4.0.0 to 4.1.0
- Bump indicatif from 0.17.1 to 0.17.2
- Bump chrono from 0.4.22 to 0.4.23
- Bump reqwest from 0.11.12 to 0.11.13
- Bump serde_with from 2.0.1 to 2.1.0
- Bump Swatinem/rust-cache from 1 to 2
- Bump rayon from 1.5.3 to 1.6.0
- Bump serde_json from 1.0.87 to 1.0.88

### Progress

- Add support for env variable RUSTIC_PROGRESS_INTERVAL

### Archiver

- Parallelize packing

### Backup

- Add --as-path option
- Add option --ignore-devid
- Allow relative paths
- Add option --group-by and use it for parent detection

### Cat/ls/restore

- Add filtering for latest snapshot

### Diff

- Allow to diff with local path
- Allow to use latest when diffing with local dir

### Forget

- Fix table header

### Index

- Parallelize sorting the index

### Prune

- Parallelize repacking

### Snapshot

- Add program version
- Add label
- Add description field

## [0.3.2] - 2022-10-07

### Breaking changes

- Logging is completely reworked. New option --log-level replaces --verbose and
  --quiet

# Fixes

- Fixed broken error handling in REST/rclone backend some error kinds.
- Don't prompt for password in init command if it is given.

### Features

- New option --log-file allows logging to a file
- New command completions to generate shell completions
- check: Added --read-data option
- check: Improved error handling and error messages
- rest/rclone backend: Abort immediately at permanent errors.
- restore: better debug output to see what restore exactly will do
- rclone backend no longer needs a temp dir. This meas rustic now doesn't need a
  temp dir at all.
- Nicer display of snapshot groups
- Added blackbox test using bats
- Shell completions ([#195](https://github.com/rustic-rs/rustic/issues/195))

### Miscellaneous Tasks

- Bump self_update from 0.31.0 to 0.32.0
- Bump sha2 from 0.10.5 to 0.10.6
- Bump sha1 from 0.10.4 to 0.10.5
- Bump clap from 3.2.21 to 3.2.22
- Bump binrw from 0.9.2 to 0.10.0
- Bump itertools from 0.10.4 to 0.10.5
- Bump reqwest from 0.11.11 to 0.11.12
- Bump serde from 1.0.144 to 1.0.145
- Bump semver from 1.0.13 to 1.0.14
- Bump tokio from 1.21.1 to 1.21.2
- Bump thiserror from 1.0.35 to 1.0.37

### README

- Update to match restic 0.14

### Backup

- Add --host option

### Check

- Optimize error handling and messages
- Add --read-data

### Init

- Use password if given

### Restore

- Print what will be done in debug log

## [0.3.1] - 2022-09-15

### Note

Changing the binary name to rustic is a breaking change with respect to the
self-update command. This means rustic 0.3.0 can *NOT* be updated using
self-update. Please download the binaries manually instead.

### Bug Fixes

- change escaping of filename to get identical result as restic
- fix performance regression because of filename escaping
- chunker: Fixed chunker such that chunks of MINSIZE are possible.
- prune: Fix option --max-repack; now also works when resizing packs.

### Features

- Changed name of binary from rustic-rs to rustic
- Added config file support (see examples in `config/` dir)
- Added options --password and --password-command (and equivalents as env
  variables and config file options)
- snapshots: Summarize fully identical snapshots in snapshots command; added
  option --all.
- snapshots: Grouping by hosts and paths is now the default.
- snapshots: Added --json option
- backup: Allow backing up multiple source paths
- backup: Allow backup from stdin
- backup/parent detection now uses ctime and mtime; new options --ignore-mtime
  and --ignore-inode
- backup: Added option --exclude-larger-than
- forget: Always remove snapshots when ID is given
- prune: Only resize small packs when target packsize will be reached.
- prune: Added option --no-resize
- chunker: Increase buffer size to speed up chunking
- Added aarch64-apple-darwin as supported platform

### CI

- Add support for aarch64-apple-darwin

### Miscellaneous Tasks

- Bump serde_json from 1.0.83 to 1.0.85
- Bump serde from 1.0.143 to 1.0.144
- Bump clap from 3.2.17 to 3.2.18
- Bump futures from 0.3.23 to 0.3.24
- Bump sha1 from 0.10.1 to 0.10.2
- Bump clap from 3.2.18 to 3.2.19
- Bump sha2 from 0.10.2 to 0.10.3
- Bump thiserror from 1.0.32 to 1.0.33
- Bump anyhow from 1.0.62 to 1.0.63
- Bump clap from 3.2.19 to 3.2.20
- Bump sha1 from 0.10.2 to 0.10.4
- Bump sha2 from 0.10.3 to 0.10.5
- Bump serde-aux from 3.2.0 to 4.0.0
- Bump self_update from 0.30.0 to 0.31.0
- Bump serde_with from 2.0.0 to 2.0.1
- Strip via config, not manually
- Bump clap from 3.2.20 to 3.2.21
- Bump thiserror from 1.0.34 to 1.0.35
- Bump anyhow from 1.0.64 to 1.0.65
- Bump tokio from 1.21.0 to 1.21.1
- Bump itertools from 0.10.3 to 0.10.4
- Correct audit.yml

### Backup

- Speed up searching for parent node
- Add option --exclude-larger-than
- Better improve help text for exclude options
- Allow to use stdin as source
- Allow multiple sources

### Backup/parent

- Use ctime and mtime; add --ignore options

### Chunker

- Allow chunks of MIN_SIZE
- Increase buffer size to 64kiB

### Forget

- Don't apply keep policy for given ids

### Prune

- Fix max-repack option
- Only resize if target packsize is reached
- Add option --no-resize

### Snapshots

- Summarize snapshots with identical trees
- Group by hosts and paths as default
- Add --json option

## [0.3.0] - 2022-08-18

### Fixes

- config command could invalidate config file on local backend

### Features

- backup: Added escaping of filenames to be compatible with restic
- backup: Don't use temporary files, but save incomplete pack files in-memory
- Allow to limit pack sizes
- rest/rclone backend: Retry operations if they failing
- restore: Use existing files to speed up restore (also makes restore resumable)
- restore: Added --delete option to delete existing files not in snapshot
- restore/prune: Added warm-up possibilities for hot/cold repo
- prune: Remove unneeded packs from cache
- prune: Added repacking of packs which are too small or too large
- self-update: New command to update rustic
- Added syntax SNAPSHOT[:PATH] for many command to access sub-trees within
  snapshots
- Added support for environmental variables
- Improved help texts

### CI

- Release beta builds to github.com/rustic-rs/rustic-beta
- Correct beta builds
- Use direct shell script as action doesn't support macos
- Fix ssh key for beta releases
- Correct repo name for beta builds
- Update rust-cache
- Fix typo
- Fix errors with dependabot
- Fix dependabot PRs
- Fix typo

### Miscellaneous Tasks

- Allow to optionally specify a path within snapshot
- Allow to optionally specify a path within snapshot
- Bump actions/checkout from 2 to 3
- Bump clap from 3.2.16 to 3.2.17
- Bump serde_json from 1.0.82 to 1.0.83
- Bump anyhow from 1.0.58 to 1.0.61
- Bump nix from 0.24.2 to 0.25.0
- Bump serde from 1.0.140 to 1.0.143
- Bump thiserror from 1.0.31 to 1.0.32
- Bump rpassword from 6.0.1 to 7.0.0
- Bump async-trait from 0.1.56 to 0.1.57
- Bump futures from 0.3.21 to 0.3.23
- Bump anyhow from 1.0.61 to 1.0.62
- Bump chrono from 0.4.19 to 0.4.22
- Bump prettytable-rs from 0.8.0 to 0.9.0

### Tree

- Add function subtree_id and use in cat

### Index

- Add tests

### Packer

- Don't use temporary files

### Prune

- Add waiting options
- Remove unneeded pack files from cache and add option --cache-only
- Repack packs which are too small or too large

### Restore

- Use existing fileparts
- Add warm-up options
- Add --delete options
- Add option warm-up-wait

### Warmup

- Set retry to false

## [0.2.3] - 2022-07-28

### Prune

- Fixed a critical bug which corrupted the repo when repacking compressed data
- Add progress bar for repacking
- Fix repo corruption with compressed blobs

### Restore

- Improve progress bar

## [0.2.2] - 2022-07-26

- added possibility to specify a hot repo (added --repo-hot option)
- added rclone backend and made reading/writing remote repos with higher latency
  working
- new command config; added possibility to customize compression level
- added possibility to customize pack sizes. Also changed the standard settings
  for pack size.
- fixed erroneous caching of data pack files
- check: new option --trust-cache
- improved speed of packer
- prune: new options --instant-delete, --repack-uncompressed, --fast-repack
- prune: option --repack-cacheable-only now expects true/false and default to
  true for hot/cold repos
- snapshots: allow to specify "latest" which only displays the latest
  snapshot(s)
- restore: fixed order of setting permission; improved error handling and debug
  output

### Backend

- Add cacheable to remove()

### Backup/prune

- Use compression from config file

### Cat

- Don't require an id

### Check

- Add option --trust-cache

### Config

- Fix saving config file for hot/cold repo

### Init

- Add config options

### Prune

- Add option --instant-delete
- Add options --repack-uncompressed and --fast-repack
- Use Tree/Data.total_after_prune for repacking
- Default value for --repack-cacheable-only from config

### Rclone

- Fix url and allow debug output

### Repoinfo

- Add info about hot repo
- Add information about pack sizes

### Restore

- Be more verbose by default
- Restore metadata of dir after its contents
- Improve error handling
- Fix dir already exists error
- Print detailed information at high verbosity

### Snapshots

- Allow argument "latest"

## [0.2.1] - 2022-07-08

- add support for local cache (adds --no-cache and --cache-dir options)
- added --prune option to forget
- restore: display and ignore most errors during restore
- restore: handle much more cases
- fix chunker for empty files
- REST backend: fix url path
- Local backend: fix treatment of additional files
- added fully support special files
- Allow specifying global options with subommands

### CLI

- Make most options global; change texts

### Miscellaneous Tasks

- Add support for special files
- Create special files

### Backend

- Always use anyhow::Result

### Backup

- Always store uid/gid

### Cache

- Add Option --cache-dir and use restic/rustic cache dir
- Make options more obvious to work with

### Check

- Add check for valid cache files

### Chunker

- Correct treatment of empty files

### Forget

- Add --prune option

### Restore

- Use correct file modes when restoring
- Restore user/group
- Add option --numeric-id
- Restore times
- Add error handling

## [0.2.0-rc3] - 2022-06-13

### CI

- Use cache for tests

## [0.2.0-rc2] - 2022-06-13

### CI

- Don't accept clippy warnings
- Add automatic release builds

### Miscellaneous Tasks

- Add changed status for special files

### Prune

- Do not recover unnecessarily

## [0.2.0-rc1] - 2022-06-04

- new commands: init, forget, prune, repoinfo, tag, key
- allow parallel lock-free repo access including prune
- added REST backend
- add compression support
- add support for other unix OSes, e.g. macOS
- most operations are now parallelized (using async rust)
- added more statistical information to snapshots
- allow to mark snapshots as uneraseable or to be deleted at given time
- now uses the same JSON format for trees/nodes as restic
- better progress bars

### Archiver

- Add statistics
- Use Node from source instead of from parent

### Backup

- Also save metadata
- Add --with-atime option
- Actually only one source
- Add --force option
- Much more options
- Only open files when they are read
- Carve out source in LocalSource which implements ReadSource

### Cat

- Add tree subcommand
- Add more error messages

### Check

- Add check for offsets in IndexFile

### Check/prune

- Add progress bar

### Forget

- Allow giving snapshot IDs
- Fix --keep-last and add --keep-id
- Parallelize deletion

### Prune

- Add options --keep-delete and --keep-pack
- Fix option --repack-cacheable-only
- Add closure to print byte size
- Correct percentage unused space after prune
- Add more infos to output
- Improve algorithm
- Correct stats and parallelize deletion

### Repoinfo

- Fix ProgressBar

### Restore

- Parallelize and add progress bar

### Snapshots

- Allow giving snapshot IDs
- Add option --long

### Tag

- Fix bug, parallelize and add more options

<!-- generated by git-cliff -->
