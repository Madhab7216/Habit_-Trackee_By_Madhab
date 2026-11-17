#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo "Usage: $0 path/to/madhab-habit-tracker_VERSION_amd64.deb" >&2
    exit 2
fi
package_file="$1"
[[ -f "$package_file" ]] || { echo "Package file not found: $package_file" >&2; exit 2; }

package_name="$(dpkg-deb -f "$package_file" Package)"
package_version="$(dpkg-deb -f "$package_file" Version)"
package_arch="$(dpkg-deb -f "$package_file" Architecture)"
depends="$(dpkg-deb -f "$package_file" Depends)"
[[ "$package_name" == "madhab-habit-tracker" ]] || { echo "Unexpected package name: $package_name" >&2; exit 1; }
[[ "$package_arch" == "amd64" ]] || { echo "Unexpected package architecture: $package_arch" >&2; exit 1; }
dpkg --compare-versions "$package_version" eq "$package_version"
[[ "$depends" == *"libgtk-4-1"* ]] || { echo "GTK 4 runtime dependency is missing: $depends" >&2; exit 1; }
[[ "$depends" == *"libc6"* ]] || { echo "glibc runtime dependency is missing: $depends" >&2; exit 1; }

contents="$(dpkg-deb --contents "$package_file")"
for path in \
    "./usr/bin/madhab-habit-tracker" \
    "./usr/bin/madhab-data-tool" \
    "./usr/share/applications/org.madhab.habittracker.desktop" \
    "./usr/share/metainfo/org.madhab.habittracker.metainfo.xml" \
    "./usr/share/icons/hicolor/scalable/apps/org.madhab.habittracker.svg"; do
    if ! grep -Fq "$path" <<<"$contents"; then
        echo "Required package path is missing: $path" >&2
        exit 1
    fi
done
if grep -Eq '^\./(home|root|var/lib|var/cache)/' <<<"$contents"; then
    echo "Package must not contain user data or mutable system state." >&2
    exit 1
fi

extract_dir="$(mktemp -d)"
control_dir="$(mktemp -d)"
trap 'rm -rf "$extract_dir" "$control_dir"' EXIT
dpkg-deb --extract "$package_file" "$extract_dir"
dpkg-deb --control "$package_file" "$control_dir"
for executable in madhab-habit-tracker madhab-data-tool; do
    [[ -x "$extract_dir/usr/bin/$executable" ]] || { echo "Packaged executable is not executable: $executable" >&2; exit 1; }
done
cmp -s packaging/org.madhab.habittracker.desktop "$extract_dir/usr/share/applications/org.madhab.habittracker.desktop"
cmp -s packaging/org.madhab.habittracker.metainfo.xml "$extract_dir/usr/share/metainfo/org.madhab.habittracker.metainfo.xml"
cmp -s packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg "$extract_dir/usr/share/icons/hicolor/scalable/apps/org.madhab.habittracker.svg"
grep -Eq '^Package: madhab-habit-tracker$' "$control_dir/control"
grep -Fqx "Version: $package_version" "$control_dir/control"
grep -Eq '^Architecture: amd64$' "$control_dir/control"
echo "Debian package structure, dependencies, assets and executable modes are valid."
