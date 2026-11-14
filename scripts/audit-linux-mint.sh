#!/usr/bin/env bash
# Read-only audit. Does not install packages, use sudo, change settings, or access the network.
set -u
section() { printf '\n== %s ==\n' "$1"; }
show_command() {
  local label="$1" command_name="$2"
  if command -v "$command_name" >/dev/null 2>&1; then
    printf '%-24s %s\n' "$label:" "$("$command_name" --version 2>&1 | head -n 1)"
  else
    printf '%-24s %s\n' "$label:" 'not installed'
  fi
}
section 'Operating system'
if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  printf 'OS name:                 %s\n' "${PRETTY_NAME:-unknown}"
  printf 'OS ID:                   %s\n' "${ID:-unknown}"
  printf 'OS family:               %s\n' "${ID_LIKE:-unknown}"
else
  printf 'OS release:              /etc/os-release not available\n'
fi
printf 'Kernel:                  %s\n' "$(uname -sr 2>/dev/null || printf 'unknown')"
printf 'Architecture:            %s\n' "$(uname -m 2>/dev/null || printf 'unknown')"
printf 'Session type:            %s\n' "${XDG_SESSION_TYPE:-unknown}"
printf 'Desktop session:         %s\n' "${XDG_CURRENT_DESKTOP:-unknown}"

section 'Hardware and storage'
if [[ -r /proc/meminfo ]]; then
  awk '/MemTotal:/ { printf "Total RAM:                %.1f GiB\n", $2 / 1048576 }
       /MemAvailable:/ { printf "Available RAM:            %.1f GiB\n", $2 / 1048576 }' /proc/meminfo
fi
if command -v df >/dev/null 2>&1; then
  printf '\nFilesystem space for home and root:\n'
  df -h --output=source,size,used,avail,pcent,target "$HOME" / 2>/dev/null | sort -u || true
fi

section 'Compiler and build tools'
show_command 'Rust compiler' 'rustc'
show_command 'Cargo' 'cargo'
show_command 'Rustup' 'rustup'
show_command 'C compiler' 'cc'
show_command 'GCC' 'gcc'
show_command 'pkg-config' 'pkg-config'
show_command 'ShellCheck' 'shellcheck'

section 'Native libraries'
if command -v pkg-config >/dev/null 2>&1; then
  for module in gtk4 gio-2.0 glib-2.0 sqlite3; do
    if pkg-config --exists "$module"; then
      printf '%-24s %s\n' "$module:" "$(pkg-config --modversion "$module")"
    else
      printf '%-24s %s\n' "$module:" 'development metadata not found'
    fi
  done
fi

section 'Package metadata (read-only)'
if command -v dpkg-query >/dev/null 2>&1; then
  for package in build-essential pkg-config libgtk-4-dev libglib2.0-dev libsqlite3-dev gettext; do
    version="$(dpkg-query -W -f='${Version}' "$package" 2>/dev/null || true)"
    if [[ -n "$version" ]]; then
      printf '%-24s %s\n' "$package:" "$version"
    else
      printf '%-24s %s\n' "$package:" 'not installed'
    fi
  done
fi
section 'Audit notes'
printf '%s\n' \
  '- This report is informational and read-only.' \
  '- Missing packages are not installed automatically.' \
  '- Review the report before sharing it; it includes machine resource and software-version information.'
