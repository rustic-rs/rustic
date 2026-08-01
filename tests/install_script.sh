#!/bin/sh
# Smoke-test util/install.sh with a mocked GitHub release and local archive.

set -eu

ROOT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
INSTALLER="${ROOT_DIR}/util/install.sh"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/rustic-install-test.XXXXXX")"

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup 0 1 2 15

fail() {
    printf '%s\n' "test failure: $*" >&2
    exit 1
}

checksum() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

MOCK_BIN="${TEMP_DIR}/mock-bin"
PAYLOAD_DIR="${TEMP_DIR}/payload"
INSTALL_DIR="${TEMP_DIR}/install"
ARCHIVE="${TEMP_DIR}/rustic-v0.11.3-x86_64-unknown-linux-gnu.tar.gz"
CHECKSUM_FILE="${ARCHIVE}.sha256"
BAD_CHECKSUM_FILE="${TEMP_DIR}/bad.sha256"
mkdir -p "$MOCK_BIN" "$PAYLOAD_DIR" "$INSTALL_DIR"

printf '%s\n' '#!/bin/sh' 'printf "%s\\n" "rustic fixture"' >"${PAYLOAD_DIR}/rustic"
chmod +x "${PAYLOAD_DIR}/rustic"
tar -czf "$ARCHIVE" -C "$PAYLOAD_DIR" rustic
printf '%s  %s\n' "$(checksum "$ARCHIVE")" "$(basename "$ARCHIVE")" >"$CHECKSUM_FILE"
printf '%s\n' 'not-a-checksum' >"$BAD_CHECKSUM_FILE"

cat >"${MOCK_BIN}/uname" <<'EOF'
#!/bin/sh
case "$1" in
    -s) printf '%s\n' "${MOCK_UNAME_SYSTEM:-Linux}" ;;
    -m) printf '%s\n' "${MOCK_UNAME_MACHINE:-x86_64}" ;;
    *) exit 2 ;;
esac
EOF
chmod +x "${MOCK_BIN}/uname"

cat >"${MOCK_BIN}/curl" <<'EOF'
#!/bin/sh
set -eu

output=""
write_out=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o | --output)
            output="$2"
            shift 2
            ;;
        -w | --write-out)
            write_out="$2"
            shift 2
            ;;
        -*) shift ;;
        *)
            url="$1"
            shift
            ;;
    esac
done

case "$url" in
    https://github.com/rustic-rs/rustic/releases/latest)
        [ -n "$write_out" ]
        printf 'https://github.com/rustic-rs/rustic/releases/tag/%s' "$MOCK_VERSION"
        ;;
    "$MOCK_ARCHIVE_URL")
        cp "$MOCK_ARCHIVE" "$output"
        ;;
    "$MOCK_CHECKSUM_URL")
        cp "$MOCK_CHECKSUM" "$output"
        ;;
    *)
        printf '%s\n' "unexpected URL: $url" >&2
        exit 2
        ;;
esac
EOF
chmod +x "${MOCK_BIN}/curl"

VERSION="v0.11.3"
ASSET="rustic-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
ARCHIVE_URL="https://github.com/rustic-rs/rustic/releases/download/${VERSION}/${ASSET}"

run_installer() {
    env \
        PATH="${MOCK_BIN}:$PATH" \
        MOCK_VERSION="$VERSION" \
        MOCK_ARCHIVE="$ARCHIVE" \
        MOCK_CHECKSUM="$1" \
        MOCK_ARCHIVE_URL="$ARCHIVE_URL" \
        MOCK_CHECKSUM_URL="${ARCHIVE_URL}.sha256" \
        RUSTIC_INSTALL_DIR="$2" \
        "$INSTALLER"
}

run_installer "$CHECKSUM_FILE" "$INSTALL_DIR"
test -x "${INSTALL_DIR}/rustic" || fail "installer did not create the executable"
test "$("${INSTALL_DIR}/rustic")" = "rustic fixture" || fail "installed binary is unexpected"

BAD_INSTALL_DIR="${TEMP_DIR}/bad-install"
mkdir -p "$BAD_INSTALL_DIR"
if run_installer "$BAD_CHECKSUM_FILE" "$BAD_INSTALL_DIR" >/dev/null 2>&1; then
    fail "installer accepted an invalid checksum"
fi
test ! -e "${BAD_INSTALL_DIR}/rustic" || fail "installer wrote after checksum failure"

if env \
    PATH="${MOCK_BIN}:$PATH" \
    MOCK_UNAME_SYSTEM="FreeBSD" \
    RUSTIC_VERSION="$VERSION" \
    RUSTIC_INSTALL_DIR="$BAD_INSTALL_DIR" \
    "$INSTALLER" >/dev/null 2>&1; then
    fail "installer accepted an unsupported platform"
fi

printf '%s\n' 'install script tests passed'
