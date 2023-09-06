# Rustic Configuration Specification

`rustic` is a backup tool that allows users to define their backup options using
a TOML configuration file. The configuration file consists of various sections
and attributes that control the behavior of `rustic` for different commands and
sources.

This specification covers all the available sections and attributes in the
`rustic` configuration file and includes their corresponding environment
variable names. Users can customize their backup behavior by modifying these
attributes according to their needs.

## Merge Precedence

The merge precedence for values is:

    Commandline Arguments >> Environment Variables >> Configuration File

Values parsed from the `configuration file` can be overwritten by
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
`~/.config/rustic/`. You can use a different config files, e.g. `myconfig.toml`
and use the `-P` option to specify the profile name, e.g.
`rustic -P myconfig.toml`. Examples for different configuration files can be
found here in the [/config/](/config) directory.

## Sections and Attributes

### Global Options

| Attribute         | Description                                                                       | Default Value | Example Value     | Environment Variable     |
| ----------------- | --------------------------------------------------------------------------------- | ------------- | ----------------- | ------------------------ |
| dry-run           | If true, performs a dry run without making any changes.                           | false         |                   | RUSTIC_DRY_RUN           |
| log-level         | Logging level. Possible values: "off", "error", "warn", "info", "debug", "trace". | "info"        |                   | RUSTIC_LOG_LEVEL         |
| log-file          | Path to the log file.                                                             | No log file   | "/log/rustic.log" | RUSTIC_LOG_FILE          |
| no-progress       | If true, disables progress indicators.                                            | false         |                   | RUSTIC_NO_PROGRESS       |
| progress-interval | The interval at which progress indicators are shown.                              | "100ms"       | "1m"              | RUSTIC_PROGRESS_INTERVAL |
| use-profile       | An array of profiles to use.                                                      | Empty array   |                   | RUSTIC_USE_PROFILE       |

### Global Options - env variables

All given environment variables are set before processing. This is handy to
configure e.g. the rclone-backend or some commands which will be called by
rustic.

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

| Attribute    | Description                           | Default Value   | Example Value |
| ------------ | ------------------------------------- | --------------- | ------------- |
| filter-fn    | Custom filter function for snapshots. | Not set         |               |
| filter-host  | Array of hosts to filter snapshots.   | Not set         | ["myhost"]    |
| filter-label | Array of labels to filter snapshots.  | No label filter |               |
| filter-paths | Array of paths to filter snapshots.   | No paths filter |               |
| filter-tags  | Array of tags to filter snapshots.    | No tags filter  |               |

### Backup Options

**Note**: Some options are not source-specific, but if set here, they apply for
all sources, although they can be overwritten in the source-specifc
configuration.

| Attribute        | Description                                               | Default Value | Example Value |
| ---------------- | --------------------------------------------------------- | ------------- | ------------- |
| description      | Description for the backup.                               | Not set       |               |
| description-from | Path to a file containing the description for the backup. | Not set       |               |
| delete-never     | If true, never delete the backup.                         | false         |               |
| delete-after     | Time duration after which the backup will be deleted.     | Not set       |               |

### Backup Sources

| Attribute | Description                          | Default Value | Example Value         |
| --------- | ------------------------------------ | ------------- | --------------------- |
| source    | Source directory or file to back up. | Not set       | "/tmp/dir/to_backup/" |

#### Source-specific options

**Note**: The following options can be specified for each source individually in
the source-individual section, see below. If they are specified here, they
provide default values for all sources but can still be overwritten in the
source-individual section.

| Attribute          | Description                                                                             | Default Value |
| ------------------ | --------------------------------------------------------------------------------------- | ------------- |
| as-path            | Specifies the path for the backup when the source contains a single path.               | Not set       |
| exclude-if-present | Array of filenames to exclude from the backup if they are present.                      | Not set       |
| force              | If true, forces the backup even if no changes are detected.                             | Not set       |
| git-ignore         | If true, use .gitignore rules to exclude files from the backup in the source directory. | true          |
| glob-file          | Array of glob files specifying additional files to include in the backup.               | Not set       |
| group-by           | Grouping strategy for the backup.                                                       | Not set       |
| host               | Host name for the backup.                                                               | Not set       |
| ignore-ctime       | If true, ignores file change time (ctime) for the backup.                               | Not set       |
| ignore-inode       | If true, ignores file inode for the backup.                                             | Not set       |
| label              | Label for the backup.                                                                   | Not set       |
| one-file-system    | If true, only backs up files from the same filesystem as the source.                    | Not set       |
| parent             | Parent snapshot ID for the backup.                                                      | Not set       |
| stdin-filename     | File name to be used when reading from stdin.                                           | Not set       |
| tag                | Array of tags for the backup.                                                           | Not set       |
| with-atime         | If true, includes file access time (atime) in the backup.                               | Not set       |

### Forget Options

| Attribute         | Description                                                | Default Value | Example Value  |
| ----------------- | ---------------------------------------------------------- | ------------- | -------------- |
| filter-host       | Array of hosts to filter snapshots.                        | Not set       | ["forgethost"] |
| keep-daily        | Number of daily backups to keep.                           | Not set       |                |
| keep-within-daily | The time duration within which daily backups will be kept. | Not set       | "7 days"       |
| keep-hourly       | Number of hourly backups to keep.                          | Not set       |                |
| keep-monthly      | Number of monthly backups to keep.                         | Not set       |                |
| keep-weekly       | Number of weekly backups to keep.                          | Not set       |                |
| keep-yearly       | Number of yearly backups to keep.                          | Not set       |                |
| keep-tags         | Array of tags to keep.                                     | Not set       | ["mytag"]      |

### Copy Targets

**Note**: Copy-targets are simply repositories with the same defaults as within
the repository section.

| Attribute           | Description                                                            | Default Value            | Example Value          |
| ------------------- | ---------------------------------------------------------------------- | ------------------------ | ---------------------- |
| cache-dir           | Path to the cache directory for the target repository.                 | ~/.cache/rustic/$REPO_ID | ~/.cache/my_own_cache/ |
| no-cache            | If true, disables caching for the target repository.                   | false                    |                        |
| password            | The password for the target repository.                                | Not set                  |                        |
| password-file       | Path to a file containing the password for the target repository.      | Not set                  |                        |
| password-command    | Command to retrieve the password for the target repository.            | Not set                  |                        |
| post-create-command | Command to execute after creating a snapshot in the target repository. | Not set                  |                        |
| post-delete-command | Command to execute after deleting a snapshot in the target repository. | Not set                  |                        |
| repository          | The path or URL to the target repository.                              | Not set                  |                        |
| repo-hot            | The path or URL to the hot target repository.                          | Not set                  |                        |
| warm-up             | If true, warms up the target repository by file access.                | Not set                  |                        |
| warm-up-command     | Command to warm up the target repository.                              | Not set                  |                        |
| warm-up-wait        | The wait time for warming up the target repository.                    | Not set                  |                        |
