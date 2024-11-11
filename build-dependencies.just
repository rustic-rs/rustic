### DEFAULT ###

# Install dependencies for the default feature on x86_64-unknown-linux-musl
install-default-x86_64-unknown-linux-musl:
    sudo apt-get update
    sudo apt-get install -y musl-tools

# Install dependencies for the default feature on aarch64-unknown-linux-musl
install-default-aarch64-unknown-linux-musl:
    sudo apt-get update
    sudo apt-get install -y musl-tools

### MOUNT ###

# Install dependencies for the mount feature on x86_64-unknown-linux-gnu
install-mount-x86_64-unknown-linux-gnu:
    sudo apt-get update
    sudo apt-get install -y fuse3 libfuse3-dev pkg-config

# Install dependencies for the mount feature on aarch64-unknown-linux-gnu
install-mount-aarch64-unknown-linux-gnu:
    sudo apt-get update
    sudo apt-get install -y fuse3 libfuse3-dev pkg-config

# Install dependencies for the mount feature on i686-unknown-linux-gnu
install-mount-i686-unknown-linux-gnu:
    sudo apt-get update
    sudo apt-get install -y fuse3 libfuse3-dev pkg-config

# Install dependencies for the mount feature on x86_64-apple-darwin
install-mount-x86_64-apple-darwin:
    brew install macfuse

# Install dependencies for the mount feature on aarch64-apple-darwin
install-mount-aarch64-apple-darwin:
    brew install macfuse

# Install dependencies for the mount feature on x86_64-pc-windows-msvc
install-mount-x86_64-pc-windows-msvc:
    winget install winfsp
