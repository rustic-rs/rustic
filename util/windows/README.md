# Backing up locked Windows files with VSS

`rustic-vss-backup.ps1` is an administrator-run wrapper for backing up a
single local Windows path from a temporary Volume Shadow Copy Service (VSS)
snapshot. It is not native VSS support in rustic: the wrapper creates and
removes the snapshot using Windows' built-in `Win32_ShadowCopy` CIM interface.

The wrapper passes the shadow copy's `DeviceObject` path directly to rustic and
uses `--as-path` to retain the original path in the resulting snapshot. It
therefore does not need a symlink or a third-party VSS executable.

## Requirements and scope

- Run PowerShell as Administrator. VSS creation requires elevation.
- Configure rustic normally, for example through its profile or the
  `RUSTIC_REPOSITORY` and password environment variables. Do not put
  credentials in this script.
- The source must be a path on a local drive-letter volume, such as `C:\` or
  `C:\Users\alice`. UNC paths and mounted volumes are deliberately out of
  scope for this small example.
- Confirm the workflow on a non-critical path first. The script removes the
  shadow copy in a `finally` block, including when rustic fails, but a cleanup
  failure is still reported as a failed run so it can be investigated.

## Run it

With `rustic` on `PATH`:

```powershell
$env:RUSTIC_REPOSITORY = "C:\backup-repository"
$env:RUSTIC_PASSWORD_FILE = "C:\secure\rustic-password.txt"

.\rustic-vss-backup.ps1 `
    -Source "C:\Users\alice" `
    -RusticArgument @("--tag", "vss")
```

If the executable is not on `PATH`, give its full path:

```powershell
.\rustic-vss-backup.ps1 `
    -Source "C:\Users\alice" `
    -RusticExe "C:\Program Files\rustic\rustic.exe" `
    -RusticArgument @("--use-profile", "home", "--tag", "vss")
```

`-RusticArgument` is a PowerShell string array. Its values are passed to
`rustic backup`, so it can contain ordinary backup and global rustic options.

When using a source-specific profile section, make its `sources` entry match
the original path, not the temporary VSS device path. The wrapper's
`--as-path` lets rustic select that section while preserving the normal path in
the snapshot:

```toml
[[backup.snapshots]]
sources = ['C:\Users\alice']
tags = ["documents"]
```

Windows exposes the VSS snapshot through `Win32_ShadowCopy.DeviceObject`; the
wrapper creates it with the documented `ClientAccessible` context and removes
that exact object after the backup.
