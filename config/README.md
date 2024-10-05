<p align="center">
<img src="https://raw.githubusercontent.com/rustic-rs/assets/main/logos/readme_header_config.png" height="400" />
</p>

# rustic Configuration Specification

`rustic` is a backup tool that allows users to define their backup options in
profiles using TOML files. A configuration profile consists of various sections
and attributes that control the behavior of `rustic` for different commands and
sources.

This specification covers all the available sections and attributes in the
`rustic` configuration profile file and includes their corresponding environment
variable names. Users can customize their backup behavior by modifying these
attributes according to their needs.

## Table of Contents

- [Merge Precedence](#merge-precedence)
- [Profiles](#profiles)
- [Sections and Attributes](#sections-and-attributes)
  - [Global Options `[global]`](#global-options-global)
  - [Global Hooks `[global.hooks]`](#global-hooks-globalhooks)
  - [Global Options - env variables `[global.env]`](#global-options---env-variables-globalenv)
  - [Repository Options `[repository]`](#repository-options-repository)
  - [Repository Options (Additional) `[repository.options]`](#repository-options-additional-repositoryoptions)
  - [Repository Options for cold repo (Additional) `[repository.options-cold]`](#repository-options-for-cold-repo-additional-repositoryoptions-cold)
  - [Repository Options for hot repo (Additional) `[repository.options-hot]`](#repository-options-for-hot-repo-additional-repositoryoptions-hot)
  - [Repository Hooks `[repository.hooks]`](#repository-hooks-repositoryhooks)
  - [Snapshot-Filter Options `[snapshot-filter]`](#snapshot-filter-options-snapshot-filter)
  - [Backup Options `[backup]`](#backup-options-backup)
  - [Backup Hooks `[backup.hooks]`](#backup-hooks-backuphooks)
  - [Backup Snapshots `[[backup.snapshots]]`](#backup-snapshots-backupsnapshots)
  - [Forget Options `[forget]`](#forget-options-forget)
  - [Copy Targets `[copy]`](#copy-targets-copy)
  - [WebDAV Options `[webdav]`](#webdav-options-webdav)

## Merge Precedence

The merge precedence for values is:

    Commandline Arguments >> Environment Variables >> Configuration Profile

Values parsed from the `configuration profile` can be overwritten by
`environment variables`, which can be overwritten by `commandline arguments`
options. Therefore `commandline arguments` have the highest precedence.

**NOTE**: There are the following restrictions:

- Not all options are available as environment variables or commandline
  arguments. There are also commandline options which cannot be set in the
  profile TOML files.
- You can overwrite values, but for most values, you cannot "unset" them on a
  higher priority level.

## Profiles

Configuration files can be placed in the user's local config directory, e.g.
`~/.config/rustic/` or in the global config dir, e.g. `/etc/rustic/`. You can
use different config files, e.g. `myconfig.toml` and use the `-P` option to
specify the profile name, e.g. `rustic -P myconfig`. Examples for different
configuration files can be found here in the [/config/](/config) directory.

## Services

We have collected some examples how to configure `rustic` for various services
in the [services/](/config/services/) subdirectory. Please note that these
examples are not complete and may not work out of the box. They are intended to
give you a starting point for your own configuration.

If you want to contribute your own configuration, please
[open a pull request](https://rustic.cli.rs/dev-docs/contributing-to-rustic.html#submitting-pull-requests).

## Sections and Attributes

### Global Options `[global]`

| Attribute         | Description                                                                       | Default Value | Example Value     | Environment Variable     | CLI Option          |
| ----------------- | --------------------------------------------------------------------------------- | ------------- | ----------------- | ------------------------ | ------------------- |
| check-index       | If true, check the index and read pack headers if index information is missing.   | false         |                   | RUSTIC_CHECK_INDEX       | --check-index       |
| dry-run           | If true, performs a dry run without making any changes.                           | false         |                   | RUSTIC_DRY_RUN           | --dry-run, -n       |
| log-level         | Logging level. Possible values: "off", "error", "warn", "info", "debug", "trace". | "info"        |                   | RUSTIC_LOG_LEVEL         | --log-level         |
| log-file          | Path to the log file.                                                             | No log file   | "/log/rustic.log" | RUSTIC_LOG_FILE          | --log-file          |
| no-progress       | If true, disables progress indicators.                                            | false         |                   | RUSTIC_NO_PROGRESS       | --no-progress       |
| progress-interval | The interval at which progress indicators are shown.                              | "100ms"       | "1m"              | RUSTIC_PROGRESS_INTERVAL | --progress-interval |
| use-profiles      | Array of profiles to use. Allows to recursively use other profiles.               | Empty array   | ["2nd", "3rd"]    | RUSTIC_USE_PROFILE       | --use-profile, -P   |

### Global Hooks `[global.hooks]`

These external commands are run before and after each commands, respectively.

**Note**: There are also repository hooks, which should be used for commands
needed to set up the repository (like mounting the repo dir), see below.

| Attribute   | Description                                       | Default Value | Example Value | Environment Variable |
| ----------- | ------------------------------------------------- | ------------- | ------------- | -------------------- |
| run-before  | Run the given commands before execution           | not set       | ["echo test"] |                      |
| run-after   | Run the given commands after successful execution | not set       | ["echo test"] |                      |
| run-failed  | Run the given commands after failed execution     | not set       | ["echo test"] |                      |
| run-finally | Run the given commands after every execution      | not set       | ["echo test"] |                      |

### Global Options - env variables `[global.env]`

All given environment variables are set before processing. This is handy to
configure e.g. the `rclone`-backend or some commands which will be called by
rustic.

**Important**: Please do not forget to include environment variables set in the
config profile as a possible source of errors if you encounter problems. They
could possibly shadow other values that you have already set.

### Repository Options `[repository]`

| Attribute        | Description                                                | Default Value            | Example Value          | Environment Variable    | CLI Option          |
| ---------------- | ---------------------------------------------------------- | ------------------------ | ---------------------- | ----------------------- | ------------------- |
| cache-dir        | Path to the cache directory.                               | ~/.cache/rustic/$REPO_ID | ~/.cache/my_own_cache/ | RUSTIC_CACHE_DIR        | --cache-dir         |
| no-cache         | If true, disables caching.                                 | false                    |                        | RUSTIC_NO_CACHE         | --no-cache          |
| repository       | The path to the repository. Required.                      | Not set                  | "/tmp/rustic"          | RUSTIC_REPOSITORY       | --repositoy, -r     |
| repo-hot         | The path to the hot repository.                            | Not set                  |                        | RUSTIC_REPO_HOT         | --repo-hot          |
| password         | The password for the repository.                           | Not set                  | "mySecretPassword"     | RUSTIC_PASSWORD         | --password          |
| password-file    | Path to a file containing the password for the repository. | Not set                  |                        | RUSTIC_PASSWORD_FILE    | --password-file, -p |
| password-command | Command to retrieve the password for the repository.       | Not set                  |                        | RUSTIC_PASSWORD_COMMAND | --password-command  |
| warm-up          | If true, warms up the repository by file access.           | false                    |                        |                         | ---warm-up          |
| warm-up-command  | Command to warm up the repository.                         | Not set                  |                        |                         | --warm-up-command   |
| warm-up-wait     | The wait time for warming up the repository.               | Not set                  |                        |                         | --warm-up-wait      |

### Repository Options (Additional) `[repository.options]`

Additional repository options - depending on backend. These can be only set in
the config file or using env variables. For env variables use upper snake case
and prefix with "RUSTIC_REPO_OPT_", e.g. `use-password = "true"` becomes
`RUSTIC_REPO_OPT_USE_PASSWORD=true`

| Attribute           | Description                                                        | Default Value | Example Value                  |
| ------------------- | ------------------------------------------------------------------ | ------------- | ------------------------------ |
| post-create-command | Command to execute after creating a snapshot in the local backend. | Not set       | "par2create -qq -n1 -r5 %file" |
| post-delete-command | Command to execute after deleting a snapshot in the local backend. | Not set       | "sh -c \"rm -f %file*.par2\""  |

### Repository Options for cold repo (Additional) `[repository.options-cold]`

Additional repository options for cold repository - depending on backend. These
can be only set in the config file or using env variables. For env variables use
upper snake case and prefix with "RUSTIC_REPO_OPTCOLD_".

### Repository Options for hot repo (Additional) `[repository.options-hot]`

Additional repository options for hot repository - depending on backend. These
can be only set in the config file or using env variables. For env variables use
upper snake case and prefix with "RUSTIC_REPO_OPTHOT_".

see Repository Options

### Repository Hooks `[repository.hooks]`

These external commands are run before and after each repository-accessing
commands, respectively.

See [Global Hooks](#global-hooks-globalhooks).

### Snapshot-Filter Options `[snapshot-filter]`

| Attribute          | Description                                                            | Default Value | Example Value            | CLI Option           |
| ------------------ | ---------------------------------------------------------------------- | ------------- | ------------------------ | -------------------- |
| filter-hosts       | Array of hosts to filter snapshots.                                    | Not set       | ["myhost", "host2"]      | --filter-host        |
| filter-labels      | Array of labels to filter snapshots.                                   | Not set       | ["mylabal"]              | --filter-label       |
| filter-paths       | Array of pathlists to filter snapshots.                                | Not set       | ["/home,/root"]          | --filter-paths       |
| filter-paths-exact | Array or string of paths to filter snapshots. Exact match.             | Not set       | ["path1,path2", "path3"] | --filter-paths-exact |
| filter-tags        | Array of taglists to filter snapshots.                                 | Not set       | ["tag1,tag2"]            | --filter-tags        |
| filter-tags-exact  | Array or string of tags to filter snapshots. Exact match.              | Not set       | ["tag1,tag2", "tag3"]    | --filter-tags-exact  |
| filter-before      | Filter snapshots before the given date/time                            | Not set       | "2024-01-01"             | --filter-before      |
| filter-after       | Filter snapshots after the given date/time                             | Not set       | "2023-01-01 11:15:23"    | --filter-after       |
| filter-size        | Filter snapshots for a total size in the size range.                   | Not set       | "1MB..1GB"               | --filter-size        |
|                    | If a single value is given, this is taken as lower bound.              |               | "500 k"                  |                      |
| filter-size-added  | Filter snapshots for a size added to the repository in the size range. | Not set       | "1MB..1GB"               | --filter-size-added  |
|                    | If a single value is given, this is taken as lower bound.              |               | "500 k"                  |                      |
| filter-fn          | Custom filter function for snapshots.                                  | Not set       |                          | --filter-fn          |

### Backup Options `[backup]`

**Note**: If set here, the backup options apply for all sources, although they
can be overwritten in the source-specific configuration, see below.

| Attribute             | Description                                                                             | Default Value         | Example Value | CLI Option              |
| --------------------- | --------------------------------------------------------------------------------------- | --------------------- | ------------- | ----------------------- |
| as-path               | Specifies the path for the backup when the source contains a single path.               | Not set               |               | --as-path               |
| command               | Set the command saved in the snapshot.                                                  | The full command used |               | --command               |
| custom-ignorefiles    | Array of names of custom ignorefiles which will be used to exclude files.               | []                    |               | --custom-ignorefile     |
| description           | Description for the snapshot.                                                           | Not set               |               | --description           |
| description-from      | Path to a file containing the description for the snapshot.                             | Not set               |               | --description-from      |
| delete-never          | If true, never delete the snapshot.                                                     | false                 |               | --delete-never          |
| delete-after          | Time duration after which the snapshot be deleted.                                      | Not set               |               | --delete-after          |
| exclude-if-present    | Array of filenames to exclude from the backup if they are present.                      | []                    |               | --exclude-if-present    |
| force                 | If true, forces the backup even if no changes are detected.                             | false                 |               | --force                 |
| git-ignore            | If true, use .gitignore rules to exclude files from the backup in the source directory. | false                 |               | --git-ignore            |
| globs                 | Array of globs specifying what to include/exclude in the backup.                        | []                    |               | --glob                  |
| glob-files            | Array or string of glob files specifying what to include/exclude in the backup.         | []                    |               | --glob-file             |
| group-by              | Grouping strategy to find parent snapshot.                                              | "host,label,paths"    |               | --group-by              |
| host                  | Host name used in the snapshot.                                                         | local hostname        |               | --host                  |
| iglobs                | Like glob, but apply case-insensitive                                                   | []                    |               | --iglob                 |
| iglob-files           | Like glob-file, but apply case-insensitive                                              | []                    |               | --iglob-file            |
| ignore-devid          | If true, don't save device ID.                                                          | false                 |               | --ignore-devid          |
| ignore-ctime          | If true, ignore file change time (ctime).                                               | false                 |               | --ignore-ctime          |
| ignore-inode          | If true, ignore file inode for the backup.                                              | false                 |               | --ignore-inode          |
| init                  | If true, initialize repository if it doesn't exist, yet.                                | false                 |               | --init                  |
| json                  | If true, returns output of the command as json.                                         | false                 |               | --json                  |
| label                 | Set label fot the snapshot.                                                             | Not set               |               | --label                 |
| no-require-git        | (with git-ignore:) Apply .git-ignore files even if they are not in a git repository.    | false                 |               | --no-require-git        |
| no-scan               | Don't scan the backup source for its size (disables ETA).                               | false                 |               | --no-scan               |
| one-file-system       | If true, only backs up files from the same filesystem as the source.                    | false                 |               | --one-file-system       |
| parent                | Parent snapshot ID for the backup.                                                      | Not set               |               | --parent                |
| quiet                 | Don't output backup summary.                                                            | false                 |               | --quiet                 |
| skip-identical-parent | Skip saving of the snapshot if it is identical to the parent.                           | false                 |               | --skip-identical-parent |
| stdin-filename        | File name to be used when reading from stdin.                                           | Not set               |               | --stdin-filename        |
| tags                  | Array of tags for the backup.                                                           | []                    |               | --tag                   |
| time                  | Set the time saved in the snapshot.                                                     | current time          |               | --time                  |
| with-atime            | If true, includes file access time (atime) in the backup.                               | false                 |               | --with-atime            |

### Backup Hooks `[backup.hooks]`

These external commands are run before and after each backup, respectively.

**Note**: Global hooks and repository hooks are run additionaly.

See [Global Hooks](#global-hooks-globalhooks).

### Backup Snapshots `[[backup.snapshots]]`

**Note**: All of the backup options mentioned before can also be used as
snapshot-specific option and then only apply to this snapshot.

| Attribute | Description                                        | Default Value | Example Value                                                          |
| --------- | -------------------------------------------------- | ------------- | ---------------------------------------------------------------------- |
| sources   | Array of source directories or file(s) to back up. | []            | ["/dir1", "/dir2"]                                                     |
| hooks     | Hooks to run before and after the backup.          | Not set       | { run-before = [], run-after = [], run-failed = [], run-finally = [] } |

Source-specific hooks are called additionally to global, repository and backup
hooks when backing up the defined sources into a snapshot.

### Forget Options `[forget]`

**Note**: At lest on of the `keep-*` options must be given. Use
`keep-none = true` if you want to remove all snapshots.

| Attribute                  | Description                                                             | Default Value      | Example Value          | CLI Option                   |
| -------------------------- | ----------------------------------------------------------------------- | ------------------ | ---------------------- | ---------------------------- |
| group-by                   | Group snapshots by given criteria before applying keep policies.        | "host,label,paths" |                        | --group-by                   |
| keep-last                  | Number of most recent snapshots to keep.                                | Not set            | 15                     | --keep-last, -l              |
| keep-hourly, -H            | Number of hourly snapshots to keep.                                     | Not set            |                        | --keep-hourly                |
| keep-daily, -d             | Number of daily snapshots to keep.                                      | Not set            | 8                      | --keep-daily                 |
| keep-weekly, -w            | Number of weekly snapshots to keep.                                     | Not set            |                        | --keep-weekly                |
| keep-monthly, -m           | Number of monthly snapshots to keep.                                    | Not set            |                        | --keep-monthly               |
| keep-quarter-yearly        | Number of quarter-yearly snapshots to keep.                             | Not set            |                        | --keep-quarter-yearly        |
| keep-half-yearly           | Number of half-yearly snapshots to keep.                                | Not set            |                        | --keep-half-yearly           |
| keep-yearly, -y            | Number of yearly snapshots to keep.                                     | Not set            |                        | --keep-yearly                |
| keep-within-hourly         | The time duration within which hourly snapshots will be kept.           | Not set            | "1 day"                | --keep-within-hourly         |
| keep-within-daily          | The time duration within which daily snapshots will be kept.            | Not set            | "7 days"               | --keep-within-daily          |
| keep-within-weekly         | The time duration within which weekly snapshots will be kept.           | Not set            |                        | --keep-within-weekly         |
| keep-within-monthly        | The time duration within which monthly snapshots will be kept.          | Not set            |                        | --keep-within-monthly        |
| keep-within-quarter-yearly | The time duration within which quarter-yearly snapshots will be kept.   | Not set            |                        | --keep-within-quarter-yearly |
| keep-within-half-yearly    | The time duration within which half-yearly snapshots will be kept.      | Not set            |                        | --keep-within-half-yearly    |
| keep-within-yearly         | The time duration within which yearly snapshots will be kept.           | Not set            |                        | --keep-within-yearly         |
| keep-tags                  | Keep snapshots containing one of these taglists.                        | []                 | ["keep", "important" ] | --keep-tags                  |
| keep-ids                   | Keep snapshots containing one of these IDs.                             | []                 | ["6e58f3d32" ]         | --keep-id                    |
| keep-none                  | Allow to keep no snapshots.                                             | false              | true                   | --keep-none                  |
| prune                      | If set to true, prune the repository after snapshots have been removed. | false              |                        | --prune                      |

Additionally extra snapshot filter options can be given for the `forget` command
here, see Snapshot-Filter options.

### Copy Targets `[copy]`

**Note**: Copy-targets must be defined in their own config profile files.

| Attribute | Description        | Default Value | Example Value            | CLI Option |
| --------- | ------------------ | ------------- | ------------------------ | ---------- |
| targets   | Targets to copy to | []            | ["profile1", "profile2"] | --target   |

### WebDAV Options `[webdav]`

`rustic` supports mounting snapshots via WebDAV. This is useful if you want to
access your snapshots via a file manager.

**Note**: `https://` and Authentication are not supported yet.

The following options are available to be used in your configuration file:

| Attribute     | Description                                                                                                                                               | Default Value                                                                     | Example Value | CLI Option      |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ------------- | --------------- |
| address       | Address of the WebDAV server.                                                                                                                             | localhost:8000                                                                    |               | --address       |
| path-template | The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced.      | `[{hostname}]/[{label}]/{time}`                                                   |               | --path-template |
| time-template | The time template to use to display times in the path template. See <https://docs.rs/chrono/latest/chrono/format/strftime/index.html> for format options. | `%Y-%m-%d_%H-%M-%S`                                                               |               | --time-template |
| symlinks      | If true, follows symlinks.                                                                                                                                | false                                                                             |               | --symlinks      |
| file-access   | How to handle access to files.                                                                                                                            | "forbidden" for hot/cold repositories, else "read"                                |               | --file-access   |
| snapshot-path | Specify directly which snapshot/path to serve                                                                                                             | Not set, this will generate a virtual tree with all snapshots using path-template |               | --snapshot-path |
