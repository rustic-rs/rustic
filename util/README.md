# Utility scripts

## Install a released rustic binary

[`install.sh`](install.sh) installs the latest stable rustic release on Linux
or macOS. It is a direct-binary installer, not an apt, yum, Homebrew, Scoop, or
other package-manager integration.

The script resolves the latest release, selects a published archive for the
local CPU and operating system, verifies its published SHA-256 checksum, and
installs the contained `rustic` executable. It fails instead of guessing when
the platform, target, checksum, required tools, or destination is unsupported.

Download and review it before running:

```sh
curl --fail --location --output /tmp/rustic-install.sh \
  https://raw.githubusercontent.com/rustic-rs/rustic/main/util/install.sh
sh /tmp/rustic-install.sh
```

The default destination is `/usr/local/bin`, so it may require appropriate
privileges. To install into an existing writable directory instead:

```sh
RUSTIC_INSTALL_DIR="$HOME/.local/bin" sh /tmp/rustic-install.sh
```

Set `RUSTIC_VERSION` to install a particular release tag, for example
`RUSTIC_VERSION=v0.11.3`. Linux defaults to the GNU target; an advanced user
can select the corresponding published musl archive with `RUSTIC_TARGET`, for
example `RUSTIC_TARGET=x86_64-unknown-linux-musl`.
