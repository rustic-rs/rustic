# Operational production-readiness checklist

This checklist is for operators who intend to use `rustic` for unattended or
otherwise important backups. It records practical checks that a deployment
needs in addition to a successful `backup` command. It does not certify a
configuration, backend, or deployment as safe, and it cannot prevent data
loss.

Adapt the frequency and depth of the checks to the data, recovery-time goal,
backend, and operational requirements of the environment.

## Before relying on a repository

- [ ] Define the data to protect, retention expectations, recovery-time goal,
  and the acceptable amount of data that could be lost between backups.
- [ ] Test the exact profiles, source paths, filters, path remapping, and
  backend settings that the scheduled job will use. Inspect the resulting
  snapshot to confirm that it contains the expected sources.
- [ ] Restore representative files and directories into an isolated location.
  Check the recovered content and any metadata that matters for the workload.
- [ ] Run `rustic check` against the repository. If the integrity policy must
  read pack data as well, schedule `--read-data` with a suitable
  `--read-data-subset` policy and test it against the chosen backend.
- [ ] Exercise the credentials and storage operations needed for backup,
  inspection, and restore. For cold or archival storage, include the warm-up
  delay, lifecycle rules, retrieval cost, and restore process in that test.
- [ ] Store repository passwords and backend credentials so they remain
  available to authorized recovery operators if the normal automation host is
  unavailable.
- [ ] Test an upgrade in a representative environment before changing the
  version used by scheduled jobs. Review the release notes for changes that
  affect the selected backend or command options.

## Keep the deployment observable

- [ ] Treat a non-zero command exit as a failed job and retain enough logs to
  investigate it.
- [ ] Alert when a scheduled job does not produce the expected recent snapshot,
  not only when it exits unsuccessfully. Compare its source paths, size, and
  file counts with expectations where those signals are useful.
- [ ] Review filters, source paths, retention rules, and backend lifecycle
  settings whenever the workload or storage policy changes.
- [ ] Periodically restore current data to a separate destination and verify
  that the recovery procedure, permissions, and application-level data are
  usable.
- [ ] Run repository checks on a schedule appropriate for the data and storage
  provider. A check can validate repository data, but it does not prove that
  the backup selection matches all of an application's recovery requirements.

## Keep a recovery procedure

- [ ] Document the profile selection, required environment variables, credential
  recovery path, repository location, and restore destination for the people
  who will perform recovery.
- [ ] Keep a separate copy or recovery path where the loss or unavailability of
  the primary backend would be unacceptable.
- [ ] Rehearse recovery after meaningful changes to the operating system,
  backend, credentials, or application data layout.

See the [configuration reference](../config/README.md) and the
[user documentation](https://rustic.cli.rs/docs) for command and option
details.
