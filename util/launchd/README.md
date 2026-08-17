# Scheduling backups with `launchd`

`rustic-backup.plist` is a user-agent template for a daily macOS backup. It
runs `rustic --use-profile <profile> backup` at 03:00 local time. It is
intentionally a template: it does not include credentials and every path is
explicit, because `launchd` does not inherit an interactive shell's `PATH`.
As a `LaunchAgent`, it runs in the logged-in user's context; a system-wide
`LaunchDaemon` needs a separate, deliberately privileged setup.

1. Make a personal copy, replacing `example` in the destination filename with
   a meaningful name:

   ```sh
   cp util/launchd/rustic-backup.plist \
     "$HOME/Library/LaunchAgents/org.rustic.backup.example.plist"
   ```

2. In that copy, replace all `REPLACE_WITH_*` values:

   - `REPLACE_WITH_ABSOLUTE_PATH_TO_RUSTIC`: the output of `command -v rustic`
     (commonly `/opt/homebrew/bin/rustic` or `/usr/local/bin/rustic`).
   - `REPLACE_WITH_PROFILE_NAME`: the profile passed to `rustic --use-profile`.
   - `REPLACE_WITH_ABSOLUTE_PATH_TO_LOG_DIRECTORY`: an existing writable
     directory, for example `$HOME/Library/Logs/rustic`. Do not put passwords
     in this plist or its log paths.
   - Change the label and `StartCalendarInterval` if the default daily 03:00
     schedule is not suitable.

3. Create the log directory and validate the edited plist:

   ```sh
   mkdir -p "$HOME/Library/Logs/rustic"
   plutil -lint "$HOME/Library/LaunchAgents/org.rustic.backup.example.plist"
   ```

4. Load it into the current user's launchd domain:

   ```sh
   launchctl bootstrap "gui/$(id -u)" \
     "$HOME/Library/LaunchAgents/org.rustic.backup.example.plist"
   ```

   To run one test backup immediately, use the label from the plist:

   ```sh
   launchctl kickstart -k "gui/$(id -u)/org.rustic.backup.example"
   ```

   Inspect `rustic-backup.out.log` and `rustic-backup.err.log` afterwards.

To stop and remove the agent without deleting the plist, run:

```sh
launchctl bootout "gui/$(id -u)" \
  "$HOME/Library/LaunchAgents/org.rustic.backup.example.plist"
```
