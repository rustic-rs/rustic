# Repository Fixtures

This directory contains fixtures for testing the `rustic` and `restic`
repositories.

The `rustic` repository is a repository that is used to test the `rustic`
binary. The `restic` repository is a repository created by the `restic` backup
tool.
The latter is used to ensure that `rustic` can read and write to a repository
created by `restic`.

## Accessing the Repositories

The `rustic` repository is located at `./rustic-repo`. The `restic` repository
is located at `./restic-repo`. There is an empty repository located at
`./rustic-copy-repo` that can be used to test the copying of snapshots between
repositories.

## Repository Layout

The `rustic` repository contains the following snapshots:

```console
| ID       | Time                | Host    | Label | Tags | Paths | Files | Dirs |      Size |
|----------|---------------------|---------|-------|------|-------|-------|------|-----------|
| 31d477a2 | 2024-10-08 08:11:00 | TowerPC |       |      | src   |    51 |    7 | 240.5 kiB |
| 86371783 | 2024-10-08 08:13:12 | TowerPC |       |      | src   |    50 |    7 | 238.6 kiB |
```

The `restic` repository contains the following snapshots:

```console
ID        Time                 Host        Tags        Paths
---------------------------------------------------------------------------------------------------------------------------------------------------------------------
9305509c  2024-10-08 08:14:50  TowerPC                 src
af05ecb6  2024-10-08 08:15:05  TowerPC                 src
```

The difference between the two snapshots is that the `lib.rs` file in the `src`
directory was removed between the two snapshots.

The `rustic-copy-repo` repository is empty and contains no snapshots.

### Passwords

The `rustic` repository is encrypted with the password `rustic`. The `restic`
repository is encrypted with the password `restic`.
