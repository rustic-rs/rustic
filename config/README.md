# Rustic Configuration Specification

Rustic is a backup tool that allows users to define their backup options using a TOML configuration file. The configuration file consists of various sections and attributes that control the behavior of Rustic for different commands and sources.

This specification covers all the available sections and attributes in the Rustic configuration file. Users can customize their backup behavior by modifying these attributes according to their needs.

## Merge Precedence

The merge precedence for values is:

    Commandline Arguments >> Environment Variables >> Configuration File

Values parsed from the `configuration file` can be overwritten by `environment variables`, which can be overwritten by `commandline arguments` options. Therefore `commandline arguments` have the highest precedence.

**NOTE**: There are the following restrictions:

- You can overwrite values, but for most values, you cannot "unset" them on a higher priority level.

- For some integer values, you cannot even overwrite with the value `0`, e.g. `keep-weekly = 5` in the `[forget]` section of the config file cannot be overwritten by `--keep-weekly 5`.
This is not relevant for env variables, only for some values available in the `config` and as `CLI` option.

## Sections and Attributes

### Global Options

| Attribute         | Description                                            | Default Value  | Example Value | Environment Variable |
|-------------------|--------------------------------------------------------|----------------|---------------|---------------|
| dry-run           | If true, performs a dry run without making any changes. | false       || RUSTIC_DRY_RUN |
| log-level         | Logging level. Possible values: "off", "error", "warn", "info", "debug", "trace". | "debug" || RUSTIC_LOG_LEVEL |
| log-file          | Path to the log file.                                 | TODO | "/log/rustic.log" | RUSTIC_LOG_FILE |
| no-progress       | If true, disables progress indicators.               | false          || RUSTIC_NO_PROGRESS |
| progress-interval | The interval at which progress indicators are shown. | "100ms"        || RUSTIC_PROGRESS_INTERVAL |
| use-profile       | An array of profiles to use.                           | Empty array    || RUSTIC_USE_PROFILE |

### Repository Options

| Attribute         | Description                                            | Default Value  | Example Value | Environment Variable |
|-------------------|--------------------------------------------------------|----------------|---------------|---------------|
| cache-dir         | Path to the cache directory.                          | TODO        | Default cache dir, e.g., ~/.cache/rustic | RUSTIC_CACHE_DIR |
| no-cache          | If true, disables caching.                            | false          || RUSTIC_NO_CACHE |
| repository        | The path to the repository. Required.                 | Not set        | "/tmp/rustic" | RUSTIC_REPOSITORY |
| repo-hot          | The path to the hot repository.                       | Not set        || RUSTIC_REPO_HOT |
| password          | The password for the repository.                      | Not set        | "mySecretPassword" | RUSTIC_PASSWORD |
| password-file     | Path to a file containing the password for the repository. | Not set     || RUSTIC_PASSWORD_FILE |
| password-command  | Command to retrieve the password for the repository.   | Not set        || RUSTIC_PASSWORD_COMMAND |
| warm-up           | If true, warms up the repository by file access.      | false          ||
| warm-up-command   | Command to warm up the repository.                    | Not set        ||
| warm-up-wait      | The wait time for warming up the repository.          | Not set        ||

### Repository Options (Additional)

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| post-create-command   | Command to execute after creating a snapshot in the local backend. | Not set        | "par2create -qq -n1 -r5 %file" |
| post-delete-command   | Command to execute after deleting a snapshot in the local backend. | Not set        | "sh -c \"rm -f %file*.par2\"" |

### Snapshot-Filter Options

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| filter-fn         | Custom filter function for snapshots.                 | Not set        ||
| filter-host       | Array of hosts to filter snapshots.                   | Not set        | ["myhost"]     |
| filter-label      | Array of labels to filter snapshots.                  | No label filter ||
| filter-paths      | Array of paths to filter snapshots.                   | No paths filter ||
| filter-tags       | Array of tags to filter snapshots.                    | No tags filter  ||

### Backup Options

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| as-path                | Specifies the path for the backup when the source contains a single path. | Not set (Source-specific option) ||
| description            | Description for the backup.                           | Not set        ||
| description-from       | Path to a file containing the description for the backup. | Not set    ||
| delete-never           | If true, never delete the backup.                     | false          ||
| delete-after           | Time duration after which the backup will be deleted. | Not set        ||
| exclude-if-present     | Array of filenames to exclude from the backup if they are present. | Not set (Source-specific option) ||
| force                  | If true, forces the backup even if no changes are detected. | Not set (Source-specific option) ||
| glob-file              | Array of glob files specifying additional files to include in the backup. | Not set (Source-specific option) ||
| group-by               | Grouping strategy for the backup.                     | Not set (Source-specific option) ||
| host                   | Host name for the backup.                             | Not set (Source-specific option) ||
| ignore-ctime           | If true, ignores file change time (ctime) for the backup. | Not set (Source-specific option) ||
| ignore-inode           | If true, ignores file inode for the backup.           | Not set (Source-specific option) ||
| label                  | Label for the backup.                                 | Not set        ||
| one-file-system        | If true, only backs up files from the same filesystem as the source. | Not set (Source-specific option) ||
| parent                 | Parent snapshot ID for the backup.                    | Not set (Source-specific option) ||
| stdin-filename         | File name to be used when reading from stdin.         | Not set (Source-specific option) ||
| tag                    | Array of tags for the backup.                         | Not set        ||
| with-atime             | If true, includes file access time (atime) in the backup. | Not set (Source-specific option) ||

### Backup Sources

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| git-ignore             | If true, use .gitignore rules to exclude files from the backup in the source directory. | true ||
| source                 | Source directory or file to back up.                  | Not set        ||

### Forget Options

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| filter-host            | Array of hosts to filter snapshots.                   | Not set        | ["forgethost"] |
| keep-daily             | Number of daily backups to keep.                      | Not set        ||
| keep-within-daily      | The time duration within which daily backups will be kept. | TODO      | "7 days"     |
| keep-hourly            | Number of hourly backups to keep.                     | Not set        ||
| keep-monthly           | Number of monthly backups to keep.                    | Not set        ||
| keep-weekly            | Number of weekly backups to keep.                     | Not set        ||
| keep-yearly            | Number of yearly backups to keep.                     | Not set        ||
| keep-tags              | Array of tags to keep.                                | Not set        | ["mytag"]      |

### Copy Targets

| Attribute         | Description                                            | Default Value  | Example Value |
|-------------------|--------------------------------------------------------|----------------|---------------|
| cache-dir              | Path to the cache directory for the target repository. | TODO | Default cache dir, e.g., ~/.cache/rustic |
| no-cache               | If true, disables caching for the target repository.  | false          ||
| password               | The password for the target repository.               | Not set        ||
| password-file          | Path to a file containing the password for the target repository. | Not set   ||
| password-command       | Command to retrieve the password for the target repository. | Not set   ||
| post-create-command    | Command to execute after creating a snapshot in the target repository. | Not set   ||
| post-delete-command    | Command to execute after deleting a snapshot in the target repository. | Not set   ||
| repository             | The path or URL to the target repository.             | Not set        ||
| repo-hot               | The path or URL to the hot target repository.         | Not set        ||
| warm-up                | If true, warms up the target repository by file access. | Not set     ||
| warm-up-command        | Command to warm up the target repository.            | Not set        ||
| warm-up-wait           | The wait time for warming up the target repository.   | Not set        ||
