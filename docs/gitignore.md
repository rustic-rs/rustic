# Git ignore rules for backup sources

Set `git-ignore = true` in the backup configuration, or pass `--git-ignore`,
to apply `.gitignore` rules while walking a local source. A rule in a parent
directory of a Git worktree applies to matching descendants: it does not need
to be copied into every child directory.

For example, this worktree ignores the log file even when the backup source is
`projects/service`:

```text
projects/
├── .git/
├── .gitignore       # contains: *.log
└── service/
    ├── kept.txt
    └── cache/
        └── build.log
```

```toml
[backup]
git-ignore = true

[[backup.snapshots]]
sources = ["/srv/projects/service"]
```

`build.log` is excluded, while `kept.txt` is included. The `.gitignore` file
itself is not excluded merely because its rules are used.

## Sources outside a Git worktree

By default, `git-ignore` applies `.gitignore` rules only when the source is
inside a Git worktree. Set `no-require-git = true` when a non-Git directory
deliberately uses `.gitignore` files as backup filters:

```toml
[backup]
git-ignore = true
no-require-git = true
```

This option changes when `.gitignore` files are considered; it does not create
or otherwise manage a Git repository. Use it only when treating `.gitignore`
as a general-purpose filter is intentional.

See the [backup configuration reference](../config/README.md) for the related
options, including explicit glob and custom ignore-file settings.
