#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "\${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$ROOT/target/debug/madhab-habit-tracker"
CHECKER="$ROOT/scripts/check-accessibility-tree.py"

for dependency in xvfb-run dbus-run-session python3; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    printf 'Missing accessibility-test dependency: %s\n' "$dependency" >&2
    exit 2
  fi
done

if [[ ! -x "$BINARY" ]]; then
  printf 'Application binary not found. Build it first with cargo build --bin madhab-habit-tracker.\n' >&2
  exit 2
fi

if [[ ! -f "$CHECKER" ]]; then
  printf 'AT-SPI checker not found: %s\n' "$CHECKER" >&2
  exit 2
fi

TEMP_ROOT="$(mktemp -d)"
APP_LOG="$TEMP_ROOT/application.log"
export APP_BINARY="$BINARY"
export APP_LOG
export ACCESSIBILITY_CHECKER="$CHECKER"
export XDG_DATA_HOME="$TEMP_ROOT/xdg-data"
export GDK_BACKEND=x11
export GTK_A11Y=atspi
unset NO_AT_BRIDGE || true

cleanup() {
  rm -rf -- "$TEMP_ROOT"
}
trap cleanup EXIT

xvfb-run -a dbus-run-session -- bash -euo pipefail -c '
  "$APP_BINARY" >"$APP_LOG" 2>&1 &
  app_pid=$!

  cleanup_app() {
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  }
  trap cleanup_app EXIT

  python3 "$ACCESSIBILITY_CHECKER" --timeout 25
';
