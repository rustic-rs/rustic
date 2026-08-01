#!/bin/sh
# Install a released rustic binary on Linux or macOS.
#
# This script intentionally does not create a system package. It downloads a
# published release archive, verifies its published SHA-256 checksum, and
# installs only the `rustic` executable.

set -eu

REPOSITORY="rustic-rs/rustic"
LATEST_RELEASE_URL="https://github.com/${REPOSITORY}/releases/latest"
DOWNLOAD_BASE="https://github.com/${REPOSITORY}/releases/download"
INSTALL_DIR="${RUSTIC_INSTALL_DIR:-/usr/local/bin}"
VERSION="${RUSTIC_VERSION:-}"
TEMP_DIR=""

fail() {
    printf '%s\n' "error: $*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

cleanup() {
    if [ -n "${TEMP_DIR:-}" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}

validate_version() {
    case "$1" in
        v[0-9]*) ;;
        *) fail "invalid release version: $1" ;;
    esac

    case "$1" in
        *[!A-Za-z0-9._-]*) fail "invalid release version: $1" ;;
    esac
}

detect_target() {
    system="$(uname -s)"
    machine="$(uname -m)"

    case "$system:$machine" in
        Linux:x86_64 | Linux:amd64)
            target="${RUSTIC_TARGET:-x86_64-unknown-linux-gnu}"
            case "$target" in
                x86_64-unknown-linux-gnu | x86_64-unknown-linux-musl) ;;
                *) fail "unsupported target for Linux x86_64: $target" ;;
            esac
            ;;
        Linux:aarch64 | Linux:arm64)
            target="${RUSTIC_TARGET:-aarch64-unknown-linux-gnu}"
            case "$target" in
                aarch64-unknown-linux-gnu | aarch64-unknown-linux-musl) ;;
                *) fail "unsupported target for Linux aarch64: $target" ;;
            esac
            ;;
        Linux:i386 | Linux:i486 | Linux:i586 | Linux:i686)
            target="${RUSTIC_TARGET:-i686-unknown-linux-gnu}"
            [ "$target" = "i686-unknown-linux-gnu" ] \
                || fail "unsupported target for Linux i686: $target"
            ;;
        Linux:armv7l | Linux:armv7*)
            target="${RUSTIC_TARGET:-armv7-unknown-linux-gnueabihf}"
            [ "$target" = "armv7-unknown-linux-gnueabihf" ] \
                || fail "unsupported target for Linux armv7: $target"
            ;;
        Darwin:x86_64)
            target="${RUSTIC_TARGET:-x86_64-apple-darwin}"
            [ "$target" = "x86_64-apple-darwin" ] \
                || fail "unsupported target for macOS x86_64: $target"
            ;;
        Darwin:arm64 | Darwin:aarch64)
            target="${RUSTIC_TARGET:-aarch64-apple-darwin}"
            [ "$target" = "aarch64-apple-darwin" ] \
                || fail "unsupported target for macOS arm64: $target"
            ;;
        *) fail "unsupported platform: $system $machine" ;;
    esac

    printf '%s\n' "$target"
}

latest_version() {
    release_url="$(
        curl --fail --location --silent --show-error --output /dev/null \
            --write-out '%{url_effective}' "$LATEST_RELEASE_URL"
    )" || fail "could not resolve the latest rustic release"

    version="${release_url##*/}"
    validate_version "$version"
    printf '%s\n' "$version"
}

download() {
    output="$1"
    url="$2"
    curl --fail --location --silent --show-error --output "$output" "$url" \
        || fail "could not download $url"
}

checksum_from_file() {
    checksum_file="$1"
    checksum="$(
        awk '{
            for (i = 1; i <= NF; i++) {
                value = $i
                gsub(/[^0-9A-Fa-f]/, "", value)
                if (length(value) == 64) {
                    print tolower(value)
                    exit
                }
            }
        }' "$checksum_file"
    )"
    [ -n "$checksum" ] || fail "could not read a SHA-256 checksum from $checksum_file"
    printf '%s\n' "$checksum"
}

file_checksum() {
    archive="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        output="$(sha256sum "$archive")" || fail "could not checksum $archive"
    elif command -v shasum >/dev/null 2>&1; then
        output="$(shasum -a 256 "$archive")" || fail "could not checksum $archive"
    else
        fail "required command not found: sha256sum or shasum"
    fi
    printf '%s\n' "$output" | awk '{print tolower($1)}'
}

main() {
    require_command curl
    require_command tar
    require_command install
    require_command mktemp
    require_command awk

    case "$INSTALL_DIR" in
        /*) ;;
        *) fail "RUSTIC_INSTALL_DIR must be an absolute path: $INSTALL_DIR" ;;
    esac
    [ -d "$INSTALL_DIR" ] || fail "installation directory does not exist: $INSTALL_DIR"
    [ -w "$INSTALL_DIR" ] \
        || fail "installation directory is not writable: $INSTALL_DIR"

    if [ -n "$VERSION" ]; then
        validate_version "$VERSION"
    else
        VERSION="$(latest_version)"
    fi

    TARGET="$(detect_target)"
    ASSET="rustic-${VERSION}-${TARGET}.tar.gz"
    ARCHIVE_URL="${DOWNLOAD_BASE}/${VERSION}/${ASSET}"
    CHECKSUM_URL="${ARCHIVE_URL}.sha256"

    TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/rustic-install.XXXXXX")" \
        || fail "could not create a temporary directory"
    trap cleanup 0 1 2 15

    ARCHIVE="${TEMP_DIR}/${ASSET}"
    CHECKSUM_FILE="${ARCHIVE}.sha256"
    download "$ARCHIVE" "$ARCHIVE_URL"
    download "$CHECKSUM_FILE" "$CHECKSUM_URL"

    EXPECTED_CHECKSUM="$(checksum_from_file "$CHECKSUM_FILE")"
    ACTUAL_CHECKSUM="$(file_checksum "$ARCHIVE")"
    [ "$EXPECTED_CHECKSUM" = "$ACTUAL_CHECKSUM" ] \
        || fail "checksum verification failed for $ASSET"

    tar -xzf "$ARCHIVE" -C "$TEMP_DIR" \
        || fail "could not extract $ASSET"
    BINARY="${TEMP_DIR}/rustic"
    [ -f "$BINARY" ] || fail "release archive did not contain a rustic executable"

    install -m 0755 "$BINARY" "${INSTALL_DIR}/rustic" \
        || fail "could not install rustic to ${INSTALL_DIR}/rustic"
    printf 'installed rustic %s to %s/rustic\n' "$VERSION" "$INSTALL_DIR"
}

main "$@"
