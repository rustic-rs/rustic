<p align="center">
<img src="https://raw.githubusercontent.com/rustic-rs/assets/main/logos/readme_header_config.png" height="400" />
</p>

# Rustic Configuration Specification

`rustic` is a backup tool that allows users to define their backup options in
profiles using TOML files. A configuration profile consists of various sections
and attributes that control the behavior of `rustic` for different commands and
sources.

This specification covers all the available sections and attributes in the
`rustic` configuration profile file and includes their corresponding environment
variable names. Users can customize their backup behavior by modifying these
attributes according to their needs.

## Merge Precedence

The merge precedence for values is:

    Commandline Arguments >> Environment Variables >> Configuration Profile

Values parsed from the `configuration profile` can be overwritten by
`environment variables`, which can be overwritten by `commandline arguments`
options. Therefore `commandline arguments` have the highest precedence.

**NOTE**: There are the following restrictions:

- You can overwrite values, but for most values, you cannot "unset" them on a
  higher priority level.

- For some integer values, you cannot even overwrite with the value `0`, e.g.
  `keep-weekly = 5` in the `[forget]` section of the config file cannot be
  overwritten by `--keep-weekly 0`.

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

### Global Options

| Attribute         | Description                                                                       | Default Value | Example Value            | Environment Variable     |
| ----------------- | --------------------------------------------------------------------------------- | ------------- | ------------------------ | ------------------------ |
| check-index       | If true, check the index and read pack headers if index information is missing.   | false         |                          | RUSTIC_CHECK_INDEX       |
| dry-run           | If true, performs a dry run without making any changes.                           | false         |                          | RUSTIC_DRY_RUN           |
| log-level         | Logging level. Possible values: "off", "error", "warn", "info", "debug", "trace". | "info"        |                          | RUSTIC_LOG_LEVEL         |
| log-file          | Path to the log file.                                                             | No log file   | "/log/rustic.log"        | RUSTIC_LOG_FILE          |
| no-progress       | If true, disables progress indicators.                                            | false         |                          | RUSTIC_NO_PROGRESS       |
| progress-interval | The interval at which progress indicators are shown.                              | "100ms"       | "1m"                     | RUSTIC_PROGRESS_INTERVAL |
| use-profile       | Profile or array of profiles to use. Allows to recursely use other profiles.      | Empty array   | "other" , ["2nd", "3rd"] | RUSTIC_USE_PROFILE       |

### Global Options - env variables

All given environment variables are set before processing. This is handy to
configure e.g. the `rclone`-backend or some commands which will be called by
rustic.

**Important**: Please do not forget to include environment variables set in the
config profile as a possible source of errors if you encounter problems. They
could possibly shadow other values that you have already set.

### Repository Options

| Attribute        | Description                                                | Default Value            | Example Value          | Environment Variable    |
| ---------------- | ---------------------------------------------------------- | ------------------------ | ---------------------- | ----------------------- |
| cache-dir        | Path to the cache directory.                               | ~/.cache/rustic/$REPO_ID | ~/.cache/my_own_cache/ | RUSTIC_CACHE_DIR        |
| no-cache         | If true, disables caching.                                 | false                    |                        | RUSTIC_NO_CACHE         |
| repository       | The path to the repository. Required.                      | Not set                  | "/tmp/rustic"          | RUSTIC_REPOSITORY       |
| repo-hot         | The path to the hot repository.                            | Not set                  |                        | RUSTIC_REPO_HOT         |
| password         | The password for the repository.                           | Not set                  | "mySecretPassword"     | RUSTIC_PASSWORD         |
| password-file    | Path to a file containing the password for the repository. | Not set                  |                        | RUSTIC_PASSWORD_FILE    |
| password-command | Command to retrieve the password for the repository.       | Not set                  |                        | RUSTIC_PASSWORD_COMMAND |
| warm-up          | If true, warms up the repository by file access.           | false                    |                        |                         |
| warm-up-command  | Command to warm up the repository.                         | Not set                  |                        |                         |
| warm-up-wait     | The wait time for warming up the repository.               | Not set                  |                        |                         |

### Repository Options (Additional)

| Attribute           | Description                                                        | Default Value | Example Value                  |
| ------------------- | ------------------------------------------------------------------ | ------------- | ------------------------------ |
| post-create-command | Command to execute after creating a snapshot in the local backend. | Not set       | "par2create -qq -n1 -r5 %file" |
| post-delete-command | Command to execute after deleting a snapshot in the local backend. | Not set       | "sh -c \"rm -f %file*.par2\""  |

### Snapshot-Filter Options

| Attribute    | Description                                    | Default Value | Example Value                |
| ------------ | ---------------------------------------------- | ------------- | ---------------------------- |
| filter-host  | Array or string of hosts to filter snapshots.  | Not set       | ["myhost", "host2"] / "host" |
| filter-label | Array or string of labels to filter snapshots. | Not set       |                              |
| filter-paths | Array or string of paths to filter snapshots.  | Not set       |                              |
| filter-tags  | Array or string of tags to filter snapshots.   | Not set       |                              |
| filter-fn    | Custom filter function for snapshots.          | Not set       |                              |

### Backup Options

**Note**: If set here, the backup options apply for all sources, although they
can be overwritten in the source-specifc configuration, see below.

| Attribute             | Description                                                                             | Default Value         | Example Value |
| --------------------- | --------------------------------------------------------------------------------------- | --------------------- | ------------- |
| as-path               | Specifies the path for the backup when the source contains a single path.               | Not set               |               |
| command               | Set the command saved in the snapshot.                                                  | The full command used |               |
| custom-ignorefile     | Name of custom ignorefiles which will be used to exclude files.                         | Not set               |               |
| description           | Description for the snapshot.                                                           | Not set               |               |
| description-from      | Path to a file containing the description for the snapshot.                             | Not set               |               |
| delete-never          | If true, never delete the snapshot.                                                     | false                 |               |
| delete-after          | Time duration after which the snapshot be deleted.                                      | Not set               |               |
| exclude-if-present    | Array of filenames to exclude from the backup if they are present.                      | Not set               |               |
| force                 | If true, forces the backup even if no changes are detected.                             | false                 |               |
| git-ignore            | If true, use .gitignore rules to exclude files from the backup in the source directory. | false                 |               |
| glob                  | Array of globs specifying what to include/exclude in the backup.                        | Not set               |               |
| glob-file             | Array or string of glob files specifying what to include/exclude in the backup.         | Not set               |               |
| group-by              | Grouping strategy to find parent snapshot.                                              | "host,label,paths"    |               |
| host                  | Host name used in the snapshot.                                                         | Not set               |               |
| iglob                 | Like glob, but apply case-insensitve                                                    | Not set               |               |
| iglob-file            | Like glob-file, but apply case-insensitve                                               | Not set               |               |
| ignore-devid          | If true, don't save device ID.                                                          | false                 |               |
| ignore-ctime          | If true, ignore file change time (ctime).                                               | false                 |               |
| ignore-inode          | If true, ignore file inode for the backup.                                              | false                 |               |
| init                  | If true, initialize repository if it doesn't exist, yet.                                | false                 |               |
| json                  | If true, returns output of the command as json.                                         | false                 |               |
| label                 | Set label fot the snapshot.                                                             | Not set               |               |
| no-require-git        | (with git-ignore:) Apply .git-ignore files even if they are not in a git repository.    | false                 |               |
| no-scan               | Don't scan the backup source for its size (disables ETA).                               | false                 |               |
| one-file-system       | If true, only backs up files from the same filesystem as the source.                    | false                 |               |
| parent                | Parent snapshot ID for the backup.                                                      | Not set               |               |
| quiet                 | Don't output backup summary.                                                            | false                 |               |
| skip-identical-parent | Skip saving of the snapshot if it is identical to the parent.                           | false                 |               |
| stdin-filename        | File name to be used when reading from stdin.                                           | Not set               |               |
| tag                   | Array of tags for the backup.                                                           | Not set               |               |
| time                  | Set the time saved in the snapshot.                                                     | Not set               |               |
| with-atime            | If true, includes file access time (atime) in the backup.                               | false                 |               |

### Backup Sources

**Note**: All of the backup options mentioned before can also be used as
source-specific option and then only apply to this source.

| Attribute | Description                          | Default Value | Example Value               |
| --------- | ------------------------------------ | ------------- | --------------------------- |
| source    | Source directory or file to back up. | Not set       | "/dir" , ["/dir1", "/dir2"] |

### Forget Options

| Attribute                  | Description                                                             | Default Value      | Example Value          |
| -------------------------- | ----------------------------------------------------------------------- | ------------------ | ---------------------- |
| group-by                   | Group snapshots by given criteria before appling keep policies.         | "host,label,paths" |                        |
| keep-last                  | Number of most rescent snapshots to keep.                               | Not set            | 15                     |
| keep-hourly                | Number of hourly snapshots to keep.                                     | Not set            |                        |
| keep-daily                 | Number of daily snapshots to keep.                                      | Not set            | 8                      |
| keep-weekly                | Number of weekly snapshots to keep.                                     | Not set            |                        |
| keep-monthly               | Number of monthly snapshots to keep.                                    | Not set            |                        |
| keep-quarter-yearly        | Number of quarter-yearly snapshots to keep.                             | Not set            |                        |
| keep-half-yearly           | Number of half-yearly snapshots to keep.                                | Not set            |                        |
| keep-yearly                | Number of yearly snapshots to keep.                                     | Not set            |                        |
| keep-within-hourly         | The time duration within which hourly snapshots will be kept.           | Not set            | "1 day"                |
| keep-within-daily          | The time duration within which daily snapshots will be kept.            | Not set            | "7 days"               |
| keep-within-weekly         | The time duration within which weekly snapshots will be kept.           | Not set            |                        |
| keep-within-monthly        | The time duration within which monthly snapshots will be kept.          | Not set            |                        |
| keep-within-quarter-yearly | The time duration within which quarter-yearly snapshots will be kept.   | Not set            |                        |
| keep-within-half-yearly    | The time duration within which half-yearly snapshots will be kept.      | Not set            |                        |
| keep-within-yearly         | The time duration within which yearly snapshots will be kept.           | Not set            |                        |
| keep-tag                   | Keep snapshots containing one of these tags.                            | Not set            | ["keep", "important" ] |
| prune                      | If set to true, prune the repository after snapshots have been removed. | false              |                        |

### Copy Targets

**Note**: Copy-targets must be defined in their own config profile files.

| Attribute | Description        | Default Value | Example Value                            |
| --------- | ------------------ | ------------- | ---------------------------------------- |
| target    | One or more target | Not set       | "remote_host" / ["profile1", "profile2"] |

### WebDAV Options

`rustic` supports mounting snapshots via WebDAV. This is useful if you want to
access your snapshots via a file manager.

**Note**: `https://` and Authentication are not supported yet.

The following options are available to be used in your configuration file:

| Attribute     | Description                                                                                                                                               | Default Value                                                                     | Example Value |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ------------- |
| address       | Address of the WebDAV server.                                                                                                                             | localhost:8000                                                                    |               |
| path-template | The path template to use for snapshots. {id}, {id_long}, {time}, {username}, {hostname}, {label}, {tags}, {backup_start}, {backup_end} are replaced.      | `[{hostname}]/[{label}]/{time}`                                                   |               |
| time-template | The time template to use to display times in the path template. See <https://docs.rs/chrono/latest/chrono/format/strftime/index.html> for format options. | `%Y-%m-%d_%H-%M-%S`                                                               |               |
| symlinks      | If true, follows symlinks.                                                                                                                                | false                                                                             |               |
| file-access   | How to handle access to files.                                                                                                                            | "forbidden" for hot/cold repositories, else "read"                                |               |
| snapshot-path | Specify directly which snapshot/path to serve                                                                                                             | Not set, this will generate a virtual tree with all snapshots using path-template |               |
