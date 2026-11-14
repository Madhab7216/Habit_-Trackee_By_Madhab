#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$ROOT/target/debug/madhab-habit-tracker"

for dependency in xvfb-run xdotool dbus-run-session sqlite3; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    printf 'Missing smoke-test dependency: %s\n' "$dependency" >&2
    exit 2
  fi
done

if [[ ! -x "$BINARY" ]]; then
  printf 'Application binary not found: %s\nBuild it first with cargo build --bin madhab-habit-tracker.\n' "$BINARY" >&2
  exit 2
fi

TEMP_ROOT="$(mktemp -d)"
APP_LOG="$TEMP_ROOT/application.log"
export APP_BINARY="$BINARY"
export APP_LOG
export XDG_DATA_HOME="$TEMP_ROOT/xdg-data"
export GDK_BACKEND=x11
export NO_AT_BRIDGE=1

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

  window_id=""
  for attempt in $(seq 1 100); do
    if ! kill -0 "$app_pid" 2>/dev/null; then
      echo "The GTK application exited before presenting its main window." >&2
      cat "$APP_LOG" >&2
      exit 1
    fi

    window_id="$(xdotool search --onlyvisible --name "^Habit Trackers by Madhab$" 2>/dev/null | head -n 1 || true)"
    if [[ -n "$window_id" ]]; then
      break
    fi
    sleep 0.1
  done

  if [[ -z "$window_id" ]]; then
    echo "The expected GTK main window did not appear within 10 seconds." >&2
    cat "$APP_LOG" >&2
    exit 1
  fi

  actual_title="$(xdotool getwindowname "$window_id")"
  if [[ "$actual_title" != "Habit Trackers by Madhab" ]]; then
    printf "Unexpected GTK window title: %s\n" "$actual_title" >&2
    exit 1
  fi

  database="$XDG_DATA_HOME/madhab-habit-tracker/habit-tracker.sqlite3"
  for attempt in $(seq 1 50); do
    [[ -s "$database" ]] && break
    sleep 0.1
  done

  if [[ ! -s "$database" ]]; then
    echo "The application did not create its database under the isolated XDG_DATA_HOME." >&2
    cat "$APP_LOG" >&2
    exit 1
  fi

  integrity="$(sqlite3 "$database" "PRAGMA integrity_check;")"
  if [[ "$integrity" != "ok" ]]; then
    printf "SQLite integrity_check failed: %s\n" "$integrity" >&2
    exit 1
  fi

  printf "GTK launch passed: %s\n" "$actual_title"
  echo "Isolated SQLite database created and integrity_check returned ok."
';
