#!/usr/bin/env python3
"""Smoke-test the running GTK window through the Linux AT-SPI accessibility tree."""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from collections import deque

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


WINDOW_TITLE = "Habit Trackers by Madhab"
EXPECTED_BUTTONS = {
    "Add habit",
    "View archived",
    "Goals & milestones",
}
MAX_NODES = 5000
MAX_DEPTH = 20


def read_text(node: object, method: str) -> str:
    try:
        value = getattr(node, method)()
        return value.strip() if isinstance(value, str) else ""
    except Exception:
        return ""


def walk_tree(root: object):
    queue = deque([(root, 0)])
    visited = 0

    while queue:
        node, depth = queue.popleft()
        visited += 1
        if visited > MAX_NODES:
            raise RuntimeError(f"Accessibility tree exceeded {MAX_NODES} nodes")
        yield node

        if depth >= MAX_DEPTH:
            continue
        try:
            child_count = node.get_child_count()
        except Exception:
            continue
        if child_count < 0:
            continue

        for index in range(child_count):
            try:
                child = node.get_child_at_index(index)
            except Exception:
                child = None
            if child is not None:
                queue.append((child, depth + 1))


def find_window(desktop: object):
    for node in walk_tree(desktop):
        role = read_text(node, "get_role_name").lower()
        name = read_text(node, "get_name")
        if role in {"frame", "window"} and name == WINDOW_TITLE:
            return node
    return None


def snapshot(window: object) -> list[tuple[str, str]]:
    return [
        (
            read_text(node, "get_role_name").lower(),
            read_text(node, "get_name"),
        )
        for node in walk_tree(window)
    ]


def focused_control(window: object, name: str, roles: set[str]):
    for node in walk_tree(window):
        if read_text(node, "get_name") != name:
            continue
        if read_text(node, "get_role_name").lower() not in roles:
            continue
        try:
            states = node.get_state_set()
            if states is not None and states.contains(Atspi.StateType.FOCUSED):
                return node
        except Exception:
            continue
    return None


def send_mnemonic(key: str) -> None:
    search = subprocess.run(
        [
            "xdotool",
            "search",
            "--onlyvisible",
            "--name",
            WINDOW_TITLE,
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    )
    window_ids = [line.strip() for line in search.stdout.splitlines() if line.strip()]
    if not window_ids:
        raise RuntimeError(
            f"Could not find the X11 window {WINDOW_TITLE!r}; "
            f"xdotool search returned {search.stdout!r}"
        )

    # Explicitly focus the app because Xvfb does not start a window manager.
    # Use XTEST key input, not xdotool's --window/XSendEvent path, which GTK
    # does not treat as a real keyboard event.
    window_id = window_ids[0]
    focused = subprocess.run(
        ["xdotool", "windowfocus", "--sync", window_id],
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    )
    subprocess.run(
        ["xdotool", "key", "--clearmodifiers", "--delay", "100", f"alt+{key}"],
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    )


def focused_list_item(window: object):
    # GtkDropDown opens its option list on mnemonic activation. GTK may report
    # the focused option as an unnamed AT-SPI "list item" rather than the
    # combo-box itself. The smoke app uses a fresh empty database, so the only
    # list item in the Today window during this check belongs to the dropdown.
    for node in walk_tree(window):
        if read_text(node, "get_role_name").lower() != "list item":
            continue
        try:
            states = node.get_state_set()
            if states is not None and states.contains(Atspi.StateType.FOCUSED):
                return node
        except Exception:
            continue
    return None


def verify_mnemonic_focus(
    window: object, key: str, field_name: str, roles: set[str], timeout: float = 3.0
) -> bool:
    try:
        send_mnemonic(key)
    except (OSError, subprocess.SubprocessError, RuntimeError) as error:
        print(f"Could not send Alt+{key.upper()}: {error}", file=sys.stderr)
        return False

    time.sleep(0.15)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if focused_control(window, field_name, roles) is not None:
            return True
        if field_name == "Schedule" and focused_list_item(window) is not None:
            return True
        time.sleep(0.1)

    current_focus = []
    for node in walk_tree(window):
        try:
            states = node.get_state_set()
            if states is not None and states.contains(Atspi.StateType.FOCUSED):
                current_focus.append(
                    (
                        read_text(node, "get_role_name").lower(),
                        read_text(node, "get_name"),
                    )
                )
        except Exception:
            continue
    print(
        f"Focus after Alt+{key.upper()}: {current_focus or 'no focused AT-SPI node'}; "
        f"expected {field_name!r} with roles {sorted(roles)!r}",
        file=sys.stderr,
    )
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--timeout", type=float, default=20.0)
    args = parser.parse_args()

    if args.timeout <= 0 or args.timeout > 120:
        parser.error("--timeout must be greater than 0 and at most 120 seconds")

    init_result = Atspi.init()
    if init_result not in (0, 1):
        print(f"AT-SPI initialization failed with code {init_result}", file=sys.stderr)
        return 2

    deadline = time.monotonic() + args.timeout
    window = None
    while time.monotonic() < deadline:
        try:
            desktop = Atspi.get_desktop(0)
            window = find_window(desktop) if desktop is not None else None
        except Exception:
            window = None
        if window is not None:
            break
        time.sleep(0.25)

    if window is None:
        print(
            f"Could not find the accessible GTK window {WINDOW_TITLE!r}. "
            "Check GTK_A11Y=atspi and the session accessibility bus.",
            file=sys.stderr,
        )
        return 1

    items = snapshot(window)
    buttons = {name for role, name in items if role == "push button" and name}

    missing = sorted(EXPECTED_BUTTONS - buttons)
    if missing:
        print(
            "Accessible button names missing from the Today window: "
            + ", ".join(repr(name) for name in missing),
            file=sys.stderr,
        )
        return 1

    unnamed_buttons = [
        role for role, name in items if role == "push button" and not name
    ]
    if unnamed_buttons:
        print(
            f"Found {len(unnamed_buttons)} unnamed push button(s) in the accessible tree.",
            file=sys.stderr,
        )
        return 1

    expected_fields = {
        "Habit name": {"text", "text box", "entry"},
        "Schedule": {"combo box", "drop down", "list"},
        "Weekly target": {"spin button", "text"},
    }
    missing_fields = []
    for field_name, allowed_roles in expected_fields.items():
        if not any(
            role in allowed_roles and name == field_name
            for role, name in items
        ):
            missing_fields.append(field_name)

    if missing_fields:
        print(
            "Expected named form controls were not exposed through AT-SPI: "
            + ", ".join(repr(name) for name in missing_fields),
            file=sys.stderr,
        )
        print("Observed controls: " + repr(items), file=sys.stderr)
        return 1

    mnemonic_checks = [
        ("h", "Habit name", {"text", "text box", "entry"}),
        ("s", "Schedule", {"combo box", "drop down", "list"}),
        ("w", "Weekly target", {"spin button", "text"}),
    ]
    for key, field_name, allowed_roles in mnemonic_checks:
        if not verify_mnemonic_focus(window, key, field_name, allowed_roles):
            print(
                f"Alt+{key.upper()} did not focus the {field_name!r} control.",
                file=sys.stderr,
            )
            return 1

    print("AT-SPI accessibility and keyboard mnemonic smoke test passed.")
    print("Accessible window: " + WINDOW_TITLE)
    print("Named buttons: " + ", ".join(sorted(EXPECTED_BUTTONS)))
    print("Named form controls: " + ", ".join(sorted(expected_fields)))
    print("Keyboard focus: Alt+H, Alt+S and Alt+W reached their labelled controls.")
    print(f"Accessible objects inspected: {len(items)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
