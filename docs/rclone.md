# Troubleshooting an rclone backend

If an rclone remote works in an interactive shell but a `rclone:` repository
fails through rustic, first compare the execution contexts. Rustic reports the
error returned by rclone; it cannot correct an rclone OAuth or remote
configuration error.

Run rclone from the same operating-system user that runs rustic (including a
service, cron job, or container), using the same rclone executable, remote and
repository path, config file, and environment. In particular, check values
such as `PATH`, `HOME`, `XDG_CONFIG_HOME`, `RCLONE_CONFIG`, and any variables
set by the rustic profile's `[global.env]`. Do not copy credential values into
shell history or support requests.

For a repository written as `rclone:remote:repository-path`, validate that
exact remote/path in that same context:

```console
rclone ls 'remote:repository-path'
```

For example, `rclone:unasirdive:wpmautic.net` maps to
`rclone ls 'unasirdive:wpmautic.net'`. A successful `rclone ls` checks remote
resolution and read access, but does not by itself prove that the remote has
the write access required for `rustic init` or backups.

To obtain temporary diagnostics from the rclone child process, rerun the same
rustic command with:

```console
RCLONE_VERBOSE=2 rustic [the original command and arguments]
```

Keep this diagnostic local and temporary. Before sharing output, redact access
and refresh tokens, client secrets, authorization headers, and any other
credentials. If rclone reports an OAuth error such as `unauthorized_client`,
use its message to investigate the rclone/client configuration in that same
context; this does not guarantee a provider-side fix.

For Google Drive, a direct `opendal:gdrive` backend is an alternative to
rclone, not a repair for an rclone OAuth failure. Its separate setup is shown
in the shipped [Google Drive profile](../config/services/gdrive.toml).
