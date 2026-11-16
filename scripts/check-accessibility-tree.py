#!/usr/bin/env python3
"""Smoke-test the running GTK window through the Linux AT-SPI accessibility tree."""

from __future__ import annotations

import argparse
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

    print("AT-SPI accessibility smoke test passed.")
    print("Accessible window: " + WINDOW_TITLE)
    print("Named buttons: " + ", ".join(sorted(EXPECTED_BUTTONS)))
    print("Named form controls: " + ", ".join(sorted(expected_fields)))
    print(f"Accessible objects inspected: {len(items)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
