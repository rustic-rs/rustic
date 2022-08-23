##########################
Preparing a new repository
##########################

The place where your backups will be saved is called a "repository".
This chapter explains how to create ("init") such a repository. The repository
can be stored locally, or on some remote server or service. We'll first cover
using a local repository; the remaining sections of this chapter cover all the
other options. You can skip to the next chapter once you've read the relevant
section here.

For automated backups, rustic accepts the repository location in the
environment variable ``RUSTIC_REPOSITORY``.

For the password, several options exist:

 * Setting the environment variable ``RUSTIC_PASSWORD``

 * Specifying the path to a file with the password via the option
   ``--password-file`` or the environment variable ``RUSTIC_PASSWORD_FILE``

 * Configuring a program to be called when the password is needed via the
   option ``--password-command`` or the environment variable
   ``RUSTIC_PASSWORD_COMMAND``
   
The ``init`` command has an option called ``--set-version`` which can
be used to explicitely set the version for the new repository.

The below table shows which rustic version is required to use a certain
repository version and shows new features introduced by the repository format.

+--------------------+------------------------+---------------------+
| Repository version | Minimum rustic version | Major new features  |
+====================+========================+=====================+
| ``1``              | any version            |                     |
+--------------------+------------------------+---------------------+
| ``2``              | >= 0.2.0               | Compression support |
+--------------------+------------------------+---------------------+

Moreover, there are different options which can be set when initializing
a repository:

Options to specify the target pack size:

 * ``--set-treepack-size``, ``--set-datapack-size`` specify the default target pack
   size for tree and data pack files. Arguments can given using TODO
   For example, valid sizes are "4048kiB", "2MB", "30MiB", etc. 
   If not specified, the default is 4 MiB for tree packs and 32 MiB for data packs.

 * ``--set-treepack-growfactor``, ``--set-datapack-growfactor`` specify how much the 
   target pack size should be increased per square root of the total pack size in bytes
   of the given type. This equals to 32kiB per square root of the total pack size in GiB.



Note that larger pack sizes have advantages, especially for large repository or remote 
repositories. They lead to less packs in the repository and transfer larger datasets 
to the repository which can increase the throughput.
But there are also disadvantages. Rustic keeps the whole pack in memory before writing it 
to the backend. As writes are parallelized, multiple packs are kept. So larger pack sizes
increase the memory usage of the ``backup`` command. Moreover larger pack sizes lead to
increased repack rates during ``prune`` or ``forget -- prune``.

Local
*****

In order to create a repository at ``/srv/rustic-repo``, run the following
command and enter the same password twice:

.. code-block:: console

    $ rustic init -r /srv/rustic-repo
    enter password for new repository:
    created rustic repository 085b3c76b9 at /srv/rustic-repo

.. warning::

   Remembering your password is important! If you lose it, you won't be
   able to access data stored in the repository.

.. warning::

   On Linux, storing the backup repository on a CIFS (SMB) share is not
   recommended due to compatibility issues. Either use another backend
   or set the environment variable `GODEBUG` to `asyncpreemptoff=1`.
   Refer to GitHub issue `#2659 <https://github.com/rustic/rustic/issues/2659>`_ for further explanations.

REST Server
***********

In order to backup data to the remote server via HTTP or HTTPS protocol,
you must first set up a remote `REST
server <https://github.com/rustic/rest-server>`__ instance. Once the
server is configured, accessing it is achieved by changing the URL
scheme like this:

.. code-block:: console

    $ rustic -r rest:http://host:8000/ init

Depending on your REST server setup, you can use HTTPS protocol,
password protection, multiple repositories or any combination of
those features. The TCP/IP port is also configurable. Here
are some more examples:

.. code-block:: console

    $ rustic -r rest:https://host:8000/ init
    $ rustic -r rest:https://user:pass@host:8000/ init
    $ rustic -r rest:https://user:pass@host:8000/my_backup_repo/ init

If you use TLS, rustic will use the system's CA certificates to verify the
server certificate. When the verification fails, rustic refuses to proceed and
exits with an error. If you have your own self-signed certificate, or a custom
CA certificate should be used for verification, you can pass rustic the
certificate filename via the ``--cacert`` option. It will then verify that the
server's certificate is contained in the file passed to this option, or signed
by a CA certificate in the file. In this case, the system CA certificates are
not considered at all.

REST server uses exactly the same directory structure as local backend,
so you should be able to access it both locally and via HTTP, even
simultaneously.

Other Services via rclone
*************************

The program `rclone`_ can be used to access many other different services and
store data there. First, you need to install and `configure`_ rclone.  The
general backend specification format is ``rclone:<remote>:<path>``, the
``<remote>:<path>`` component will be directly passed to rclone. When you
configure a remote named ``foo``, you can then call rustic as follows to
initiate a new repository in the path ``bar`` in the repo:

.. code-block:: console

$ rustic -r rclone:foo:bar init

rustic takes care of starting and stopping rclone.

As a more concrete example, suppose you have configured a remote named
``b2prod`` for Backblaze B2 with rclone, with a bucket called ``yggdrasil``.
You can then use rclone to list files in the bucket like this:

.. code-block:: console

    $ rclone ls b2prod:yggdrasil

In order to create a new repository in the root directory of the bucket, call
rustic like this:

.. code-block:: console

    $ rustic -r rclone:b2prod:yggdrasil init

If you want to use the path ``foo/bar/baz`` in the bucket instead, pass this to
rustic:

.. code-block:: console

    $ rustic -r rclone:b2prod:yggdrasil/foo/bar/baz init

Listing the files of an empty repository directly with rclone should return a
listing similar to the following:

.. code-block:: console

    $ rclone ls b2prod:yggdrasil/foo/bar/baz
        155 bar/baz/config
        448 bar/baz/keys/4bf9c78049de689d73a56ed0546f83b8416795295cda12ec7fb9465af3900b44

Rclone can be `configured with environment variables`_, so for instance
configuring a bandwidth limit for rclone can be achieved by setting the
``RCLONE_BWLIMIT`` environment variable:

.. code-block:: console

    $ export RCLONE_BWLIMIT=1M

For debugging rclone, you can set the environment variable ``RCLONE_VERBOSE=2``.

Cold storage
************

Rustic supports to store the repository in a so-called cold storage. These are 
storages which are design for long-time storage and offer usually cheap storage
for the price of retarded or expensive access. Examples are Amazon S3 Glacier or
OVH Cloud Archive.

To use a cold storage and not access any data in the storage for every-day operations,
rustic needs an extra repository to store hot data. This repository can be specified
by the ``--hot-repo`` option or the ``RUSTIC_REPO_HOT`` environmental variable, e.g.:

.. code-block:: console

   $ rustic -r rclone:foo:bar --repo-hot rclone:foo:bar init

In this example in the repository ``rclone:foo:bar``` all data is saved. In the repository
``rclone:foo:bar-hot`` only hot data is saved, i.e. this is not a complete repository.

.. warning::

   You have to specify both the cold repository (using ``-r``) and the hot repository
   (using ``--repo-hot``) in the ``init`` command and all other commands which access
   and work with the repository.

Configuration file
************

Rustic supports configuration files in the TOML format which should be located in the rustic config dir. 
On unix this is typically ``$HOME/.config/rustic``, see https://docs.rs/directories/latest/directories/struct.ProjectDirs.html
for more details about the config location. If no rustic config dir is available, rustic searches the current working dir
for configuration files.

By default, rustic uses the file ``rustic.toml``. This can be overwritten by the ``-P <PROFILE>`` option which tells rustic to
search for a ``<PROFILE>.toml`` configuration file. For example, if you have a ``local.toml`` configuration for backing up to a
local dir and a ``remote.toml`` configuration for a remote storage, you can use ``rustic -P local <COMMAND>`` and ``rustic -P remote <COMMAND>``,
respectively to switch between you two backup configurations.

Note that options in the config file can always be overwritten by ENV

In the configuration file, you can specify all global and repository-specific options as well as options/sources for the ``backup`` command
and ``forget`` options. Using a config file like

.. code-block:: 
  # rustic config file to backup /home and /etc to a local repository
  [repository]
  repository = "/backup/rustic"
  password-file =  "/root/key-rustic"
  no-cache = true # no cache needed for local repository
  
  [forget]
  keep-daily = 14
  keep-weekly = 5
  
  [backup]
  exclude-if-present = [".nobackup", "CACHEDIR.TAG"]
  glob-file = ["/root/rustic-local.glob"]
  
  [[backup.sources]]
  source = "/home"
  git-ignore = true
  
  [[backup.sources]]
  source = "/etc"

allows you to use ``rustic backup`` and ``rustic forget --prune`` in your regularly backup/cleanup scripts.

See also https://github.com/rustic-rs/rustic/tree/main/examples for more config file examples.

