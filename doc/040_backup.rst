#########
Backing up
##########

Now we're ready to backup some data. The contents of a directory at a
specific point in time is called a "snapshot" in rustic. Run the
following command and enter the repository password you chose above
again:

.. code-block:: console

    $ rustic -r /srv/rustic-repo --verbose backup ~/work
    open repository
    enter password for repository:
    password is correct
    lock repository
    load index files
    start scan
    start backup
    scan finished in 1.837s
    processed 1.720 GiB in 0:12
    Files:        5307 new,     0 changed,     0 unmodified
    Dirs:         1867 new,     0 changed,     0 unmodified
    Added:      1.200 GiB
    snapshot 40dc1520 saved

As you can see, rustic created a backup of the directory and was pretty
fast! The specific snapshot just created is identified by a sequence of
hexadecimal characters, ``40dc1520`` in this case.

You can see that rustic tells us it processed 1.720 GiB of data, this is the
size of the files and directories in ``~/work`` on the local file system. It
also tells us that only 1.200 GiB was added to the repository. This means that
some of the data was duplicate and rustic was able to efficiently reduce it.

If you don't pass the ``--verbose`` option, rustic will print less data. You'll
still get a nice live status display. Be aware that the live status shows the
processed files and not the transferred data. Transferred volume might be lower
(due to de-duplication) or higher.

If you run the backup command again, rustic will create another snapshot of
your data, but this time it's even faster and no new data was added to the
repository (since all data is already there). This is de-duplication at work!

.. code-block:: console

    $ rustic -r /srv/rustic-repo --verbose backup ~/work
    open repository
    enter password for repository:
    password is correct
    lock repository
    load index files
    using parent snapshot d875ae93
    start scan
    start backup
    scan finished in 1.881s
    processed 1.720 GiB in 0:03
    Files:           0 new,     0 changed,  5307 unmodified
    Dirs:            0 new,     0 changed,  1867 unmodified
    Added:      0 B
    snapshot 79766175 saved

You can even backup individual files in the same repository (not passing
``--verbose`` means less output):

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup ~/work.txt
    enter password for repository:
    password is correct
    snapshot 249d0210 saved

Now is a good time to run ``rustic check`` to verify that all data
is properly stored in the repository. You should run this command regularly
to make sure the internal structure of the repository is free of errors.

File change detection
*********************

When rustic encounters a file that has already been backed up, whether in the
current backup or a previous one, it makes sure the file's contents are only
stored once in the repository. To do so, it normally has to scan the entire
contents of every file. Because this can be very expensive, rustic also uses a
change detection rule based on file metadata to determine whether a file is
likely unchanged since a previous backup. If it is, the file is not scanned
again.

Change detection is only performed for regular files (not special files,
symlinks or directories) that have the exact same path as they did in a
previous backup of the same location.  If a file or one of its containing
directories was renamed, it is considered a different file and its entire
contents will be scanned again.

Metadata changes (permissions, ownership, etc.) are always included in the
backup, even if file contents are considered unchanged.

On **Unix** (including Linux and Mac), given that a file lives at the same
location as a file in a previous backup, the following file metadata
attributes have to match for its contents to be presumed unchanged:

 * Modification timestamp (mtime).
 * Metadata change timestamp (ctime).
 * File size.
 * Inode number (internal number used to reference a file in a filesystem).

The reason for requiring both mtime and ctime to match is that Unix programs
can freely change mtime (and some do). In such cases, a ctime change may be
the only hint that a file did change.

The following ``rustic backup`` command line flags modify the change detection
rules:

 * ``--force``: turn off change detection and rescan all files.
 * ``--ignore-ctime``: require mtime to match, but allow ctime to differ.
 * ``--ignore-inode``: require mtime to match, but allow inode number
   and ctime to differ.

The option ``--ignore-inode`` exists to support FUSE-based filesystems and
pCloud, which do not assign stable inodes to files.

Note that the device id of the containing mount point is never taken into
account. Device numbers are not stable for removable devices and ZFS snapshots.
If you want to force a re-scan in such a case, you can change the mountpoint.

Dry Runs
********

You can perform a backup in dry run mode to see what would happen without
modifying the repo.

-  ``--dry-run``/``-n`` Report what would be done, without writing to the repository

Excluding Files
***************

You can exclude folders and files by specifying exclude patterns, currently
the exclude options are:

-  ``--git-ignore`` Respect ``.gitignore`` files and exclude paths/files not handled by git.
-  ``--glob`` include/exclue files and dirs based on given glob patterns
-  ``--iglob`` Same as ``--glob`` but ignores the case of paths
-  ``--glob-file`` Specified one or more times to exclude items listed in a given file
-  ``--iglob-file`` Same as ``--glob-file`` but ignores cases like in ``--iglob``
-  ``--exclude-if-present foo`` Specified one or more times to exclude a folder's content if it contains a file called ``foo``.
   For example, to exclude cache dirs, specify ``--exclude-if-present CACHEDIR.TAG``. 
-  ``--exclude-larger-than size`` Specified once to excludes files larger than the given size


Please see ``rustic help backup`` for more specific information about each exclude option.

Let's say we have a file called ``glob.txt`` with the following content:

::

    # exclude go-files
    !*.go
    # exclude foo/x/y/z/bar foo/x/bar foo/bar
    !foo/**/bar

It can be used like this:

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup ~/work --glob="!*.c" --glob-file=glob.txt

This instructs rustic to exclude files matching the following criteria:

 * All files matching ``*.c`` (parameter ``--glob``)
 * All files matching ``*.go`` (second line in ``glob.txt``)
 * All files and sub-directories named ``bar`` which reside somewhere below a directory called ``foo`` (fourth line in ``glob.txt``)


By specifying the option ``--one-file-system`` you can instruct rustic
to only backup files from the file systems the initially specified files
or directories reside on. In other words, it will prevent rustic from crossing
filesystem boundaries and subvolumes when performing a backup.

For example, if you backup ``/`` with this option and you have external
media mounted under ``/media/usb`` then rustic will not back up ``/media/usb``
at all because this is a different filesystem than ``/``. Virtual filesystems
such as ``/proc`` are also considered different and thereby excluded when
using ``--one-file-system``:

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup --one-file-system /

Please note that this does not prevent you from specifying multiple filesystems
on the command line, e.g:

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup --one-file-system / /media/usb

will back up both the ``/`` and ``/media/usb`` filesystems, but will not
include other filesystems like ``/sys`` and ``/proc``.

.. note:: ``--one-file-system`` is currently unsupported on Windows, and will
    cause the backup to immediately fail with an error.

Files larger than a given size can be excluded using the `--exclude-larger-than`
option:

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup ~/work --exclude-larger-than 1M

This excludes files in ``~/work`` which are larger than 1 MiB from the backup.

The default unit for the size value is bytes, so e.g. ``--exclude-larger-than 2048``
would exclude files larger than 2048 bytes (2 KiB). To specify other units,
suffix the size value with one of ``k``/``K`` for KiB (1024 bytes), ``m``/``M`` for MiB (1024^2 bytes),
``g``/``G`` for GiB (1024^3 bytes) and ``t``/``T`` for TiB (1024^4 bytes), e.g. ``1k``, ``10K``, ``20m``,
``20M``,  ``30g``, ``30G``, ``2t`` or ``2T``).


Comparing Snapshots
*******************

Rustic has a `diff` command which shows the difference between two snapshots
or a snapshot and a local path/dir


.. code-block:: console

    $ rustic -r /srv/rustic-repo diff 5845b002 2ab627a6
    password is correct
    comparing snapshot ea657ce5 to 2ab627a6:

     C   /rustic/cmd_diff.go
    +    /rustic/foo
     C   /rustic/rustic



Backing up special items and metadata
*************************************

**Symlinks** are archived as symlinks, ``rustic`` does not follow them.
When you restore, you get the same symlink again, with the same link target
and the same timestamps.

If there is a **bind-mount** below a directory that is to be saved, rustic descends into it.

**Device files** are saved and restored as device files. This means that e.g. ``/dev/sda`` is
archived as a block device file and restored as such. This also means that the content of the
corresponding disk is not read, at least not from the device file.

By default, rustic does not save the access time (atime) for any files or other
items, since it is not possible to reliably disable updating the access time by
rustic itself. This means that for each new backup a lot of metadata is
written, and the next backup needs to write new metadata again. If you really
want to save the access time for files and directories, you can pass the
``--with-atime`` option to the ``backup`` command.

Note that ``rustic`` does not back up some metadata associated with files. Of
particular note are::

  - file creation date on Unix platforms
  - inode flags on Unix platforms
  - xattr information

Reading data from stdin
***********************

Sometimes it can be nice to directly save the output of a program, e.g.
``mysqldump`` so that the SQL can later be restored. Rustic supports
this mode of operation, just supply ``-`` as backup source to the
``backup`` command like this:

.. code-block:: console

    $ set -o pipefail
    $ mysqldump [...] | rustic backup -

This creates a new snapshot of the output of ``mysqldump``. You can then
use e.g. the fuse mounting option (see below) to mount the repository
and read the file.

By default, the file name ``stdin`` is used, a different name can be
specified with ``--stdin-filename``, e.g. like this:

.. code-block:: console

    $ mysqldump [...] | rustic --stdin-filename production.sql -

The option ``pipefail`` is highly recommended so that a non-zero exit code from
one of the programs in the pipe (e.g. ``mysqldump`` here) makes the whole chain
return a non-zero exit code. Refer to the `Use the Unofficial Bash Strict Mode
<http://redsymbol.net/articles/unofficial-bash-strict-mode/>`__ for more
details on this.


Tags for backup
***************

Snapshots can have one or more tags, short strings which add identifying
information. Just specify the tags for a snapshot one by one with ``--tag``:

.. code-block:: console

    $ rustic -r /srv/rustic-repo backup --tag projectX --tag foo --tag bar ~/work
    [...]

The tags can later be used to keep (or forget) snapshots with the ``forget``
command. The command ``tag`` can be used to modify tags on an existing
snapshot.

Scheduling backups
******************

Rustic does not have a built-in way of scheduling backups, as it's a tool
that runs when executed rather than a daemon. There are plenty of different
ways to schedule backup runs on various different platforms, e.g. systemd
and cron on Linux/BSD and Task Scheduler in Windows, depending on one's
needs and requirements. When scheduling rustic to run recurringly, please
make sure to detect already running instances before starting the backup.

Space requirements
******************

Rustic currently assumes that your backup repository has sufficient space
for the backup operation you are about to perform. This is a realistic
assumption for many cloud providers, but may not be true when backing up
to local disks.

Should you run out of space during the middle of a backup, there will be
some additional data in the repository, but the snapshot will never be
created as it would only be written at the very (successful) end of
the backup operation.  Previous snapshots will still be there and will still
work.

Environment Variables
*********************

In addition to command-line options, rustic supports passing various options in
environment variables. The following lists these environment variables:

.. code-block:: console

    RUSTIC_REPOSITORY                   Location of repository (replaces -r)
    RUSTIC_REPO_HOT                     Location of hot repository (replaces -repo-hot)
    RUSTIC_PASSWORD                     The actual password for the repository (replaces --password)
    RUSTIC_PASSWORD_FILE                Location of password file (replaces --password-file)
    RUSTIC_PASSWORD_COMMAND             Command printing the password for the repository to stdout (replaces --password-command)
    RUSTIC_CACHE_DIR                    Location of the cache directory (replaces --cache-dir)
    RUSTIC_NO_CACHE                     Use no cache (replaces --no-cache)

rustic may execute ``rclone`` (for rclone backends) which may respond to further
environment variables and configuration files.

