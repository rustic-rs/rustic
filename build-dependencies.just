# Install dependencies for the mount feature on x86_64-unknown-linux-gnu
install-mount-x86_64-unknown-linux-gnu:
    sudo apt-get update
    sudo apt-get install -y fuse3 libfuse3-dev pkg-config

# Install dependencies for the mount feature on x86_64-apple-darwin
install-mount-x86_64-apple-darwin:
    brew install macfuse

# Install dependencies for the mount feature on x86_64-pc-windows-msvc
install-mount-x86_64-pc-windows-msvc:
    winget install winfsp
