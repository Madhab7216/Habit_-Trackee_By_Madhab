#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE_NAME="madhab-habit-tracker"
BUILD_BINARY="${BUILD_BINARY:-1}"
DEB_ARCH="${DEB_ARCH:-$(dpkg --print-architecture)}"

if [[ "$DEB_ARCH" != "amd64" ]]; then
    echo "Unsupported Debian architecture: $DEB_ARCH (this preview targets Linux Mint x86_64/amd64)." >&2
    exit 2
fi
command -v dpkg-deb >/dev/null 2>&1 || { echo "Install dpkg-dev first." >&2; exit 2; }
command -v cargo >/dev/null 2>&1 || { echo "Install the pinned Rust toolchain first." >&2; exit 2; }

if [[ ! -f "$ROOT/Cargo.lock" ]]; then
    echo "Cargo.lock is not committed yet; generating a build-local lockfile." >&2
    cargo generate-lockfile --manifest-path "$ROOT/Cargo.toml"
fi
if [[ "$BUILD_BINARY" == "1" ]]; then
    cargo build --release --locked --manifest-path "$ROOT/Cargo.toml" --bin madhab-habit-tracker --bin madhab-data-tool
elif [[ "$BUILD_BINARY" != "0" ]]; then
    echo "BUILD_BINARY must be 1 or 0." >&2
    exit 2
fi

MAIN_BINARY="$ROOT/target/release/madhab-habit-tracker"
DATA_BINARY="$ROOT/target/release/madhab-data-tool"
for binary in "$MAIN_BINARY" "$DATA_BINARY"; do
    [[ -x "$binary" ]] || { echo "Expected executable is missing: $binary" >&2; exit 1; }
done

default_version="$(python3 - "$ROOT/Cargo.toml" <<'PY'
import sys
import tomllib
with open(sys.argv[1], "rb") as handle:
    print(tomllib.load(handle)["package"]["version"])
PY
)"
PACKAGE_VERSION="${PACKAGE_VERSION:-$default_version}"
if [[ ! "$PACKAGE_VERSION" =~ ^[0-9][A-Za-z0-9.+:~-]*$ ]] ||
   ! dpkg --compare-versions "$PACKAGE_VERSION" eq "$PACKAGE_VERSION"; then
    echo "Invalid Debian package version: $PACKAGE_VERSION" >&2
    exit 2
fi

for required in \
    "$ROOT/packaging/org.madhab.habittracker.desktop" \
    "$ROOT/packaging/org.madhab.habittracker.metainfo.xml" \
    "$ROOT/packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg" \
    "$ROOT/docs/DESKTOP_INTEGRATION.md" "$ROOT/README.md"; do
    [[ -f "$required" ]] || { echo "Required package input is missing: $required" >&2; exit 1; }
done

mkdir -p "$ROOT/target"
stage="$(mktemp -d "$ROOT/target/deb-stage.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/DEBIAN"
install -D -m 0755 "$MAIN_BINARY" "$stage/usr/bin/madhab-habit-tracker"
install -D -m 0755 "$DATA_BINARY" "$stage/usr/bin/madhab-data-tool"
install -D -m 0644 "$ROOT/packaging/org.madhab.habittracker.desktop" "$stage/usr/share/applications/org.madhab.habittracker.desktop"
install -D -m 0644 "$ROOT/packaging/org.madhab.habittracker.metainfo.xml" "$stage/usr/share/metainfo/org.madhab.habittracker.metainfo.xml"
install -D -m 0644 "$ROOT/packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg" "$stage/usr/share/icons/hicolor/scalable/apps/org.madhab.habittracker.svg"
install -D -m 0644 "$ROOT/README.md" "$stage/usr/share/doc/$PACKAGE_NAME/README.md"
install -D -m 0644 "$ROOT/docs/DESKTOP_INTEGRATION.md" "$stage/usr/share/doc/$PACKAGE_NAME/DESKTOP_INTEGRATION.md"

installed_size="$(du -sk "$stage/usr" | awk '{print $1}')"
sed \
    -e "s|@VERSION@|$PACKAGE_VERSION|g" \
    -e "s|@ARCH@|$DEB_ARCH|g" \
    -e "s|@INSTALLED_SIZE@|$installed_size|g" \
    "$ROOT/packaging/debian/control.in" > "$stage/DEBIAN/control"

output_dir="${DEB_OUTPUT_DIR:-$ROOT/target/packages}"
mkdir -p "$output_dir"
output="$output_dir/${PACKAGE_NAME}_${PACKAGE_VERSION}_${DEB_ARCH}.deb"
rm -f "$output"
if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
    find "$stage" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +
fi
dpkg-deb --root-owner-group --build "$stage" "$output" >/dev/null
printf 'Built Debian package: %s\n' "$output"
