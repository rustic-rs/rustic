############
Installation
############

Official Binaries
*****************

Stable Releases
===============

You can download the latest stable release versions of rustic from the `rustic
release page <https://github.com/rustic/rustic/releases/latest>`__. These builds
are considered stable and releases are made regularly in a controlled manner.

There's both pre-compiled binaries for different platforms as well as the source
code available for download. Just download and run the one matching your system.

Once downloaded, the official binaries can be updated in place using the 
``rustic self-update`` command (needs rustic 0.3.1 or later):

.. code-block:: console

    $ rustic  self-update
    Checking target-arch... x86_64-unknown-linux-musl
    Checking current version... v0.3.0-dev
    Checking latest released version... v0.3.1
    New release found! v0.3.0-dev --> v0.3.1
    New release is *NOT* compatible
    
    rustic release status:
      * Current exe: "/usr/local/bin/rustic"
      * New exe release: "rustic-v0.3.1-x86_64-unknown-linux-musl.tar.gz"
      * New exe download url: "https://api.github.com/repos/rustic/rustic/releases/assets/75146490"
    
    The new release will be downloaded/extracted and the existing binary will be replaced.
    Do you want to continue? [Y/n] Y
    Downloading...
    [00:00:00] [========================================] 4.29MiB/4.29MiB (0s) Done
    Extracting archive... Done
    Replacing binary file... Done
    Update status: `0.3.1`!

.. note:: Please be aware that the user executing the ``rustic self-update``
   command must have the permission to replace the rustic binary.

Unstable Builds
===============

Another option is to use the latest builds for the master branch, available on
the `rustic beta download site
<https://github.com/rustic-rs/rustic-beta>`__. These too are pre-compiled
and ready to run, and a new version is built every time a push is made to the
maina branch.

From Source
***********

rustic is written in Rust and you need a current Rust version.

In order to build rustic from source, execute the following steps:

.. code-block:: console

    $ git clone https://github.com/rustic-rs/rustic
    [...]
    $ cd rustic
    $ cargo build --release

You can easily cross-compile rustic for all supported platforms, make sure
that the cross-compile toolchain is installed for your target.
Then run the build for your chosen target like this

.. code-block:: console

    $ cargo build --target aarch64-unknown-linux-gnu --release

