#!/usr/bin/env python3
"""Validate the one-time Plurora identity reset inventory.

Transition mode permits only decreases from the frozen baseline in
``scripts/plurora-rename-map.json``. Final mode requires zero old identity
outside this checker and the machine-readable map themselves.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
from collections.abc import Iterable

ROOT = pathlib.Path(__file__).resolve().parents[1]
MAP_PATH = ROOT / "scripts/plurora-rename-map.json"
EXCLUDED = {
    "scripts/plurora-rename-map.json",
    "scripts/check-plurora-identity.py",
}


def tracked_texts() -> Iterable[tuple[str, str]]:
    paths = subprocess.check_output(
        ["git", "ls-files", "-co", "--exclude-standard"],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
    ).splitlines()
    for relative in paths:
        if relative in EXCLUDED:
            continue
        path = ROOT / relative
        if not path.is_file():
            continue
        try:
            yield relative, path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue


def count_matches(item: dict[str, object], texts: list[tuple[str, str]]) -> int:
    literal = item.get("literal")
    if isinstance(literal, str):
        return sum(text.count(literal) for _, text in texts)
    regex = item.get("regex")
    if not isinstance(regex, str):
        raise ValueError(f"redline lacks literal or regex: {item!r}")
    pattern = re.compile(regex)
    return sum(len(pattern.findall(text)) for _, text in texts)


def validate_map(data: dict[str, object]) -> list[str]:
    errors: list[str] = []
    methods = data.get("methods")
    events = data.get("events")
    if not isinstance(methods, list) or len(methods) != 80:
        errors.append(f"expected 80 method mappings, found {len(methods or [])}")
    if not isinstance(events, list) or len(events) != 59:
        errors.append(f"expected 59 event mappings, found {len(events or [])}")

    for label, rows in (("method", methods), ("event", events)):
        if not isinstance(rows, list):
            continue
        old_ids = [row.get("old") for row in rows if isinstance(row, dict)]
        new_ids = [row.get("new") for row in rows if isinstance(row, dict)]
        if len(set(old_ids)) != len(old_ids):
            errors.append(f"duplicate old {label} IDs")
        if len(set(new_ids)) != len(new_ids):
            errors.append(f"duplicate new {label} IDs")

    if isinstance(methods, list):
        for row in methods:
            new_id = row.get("new") if isinstance(row, dict) else None
            if isinstance(new_id, str) and new_id.startswith(("kernel.", "plurora.")):
                errors.append(f"method ID is not owner-based: {new_id}")
    if isinstance(events, list):
        for row in events:
            new_id = row.get("new") if isinstance(row, dict) else None
            if isinstance(new_id, str) and new_id.startswith(("kernel/", "plurora/")):
                errors.append(f"event ID is not owner-based: {new_id}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--final",
        action="store_true",
        help="require every old-identity redline to have zero matches",
    )
    args = parser.parse_args()

    data = json.loads(MAP_PATH.read_text(encoding="utf-8"))
    errors = validate_map(data)
    texts = list(tracked_texts())
    report: list[tuple[str, int, int]] = []

    redlines = data.get("redlines", [])
    if not isinstance(redlines, list):
        errors.append("redlines must be a list")
        redlines = []

    for item in redlines:
        if not isinstance(item, dict):
            errors.append(f"invalid redline: {item!r}")
            continue
        name = str(item.get("name", "unnamed"))
        actual = count_matches(item, texts)
        limit = 0 if args.final else int(item.get("baseline_max", 0))
        report.append((name, actual, limit))
        if actual > limit:
            errors.append(f"{name}: {actual} exceeds allowed maximum {limit}")

    print("Plurora identity inventory:")
    for name, actual, limit in report:
        print(f"  {name:22} {actual:6} / {limit}")

    if errors:
        print(f"identity check failed: {len(errors)} issue(s)", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    mode = "final zero-tolerance" if args.final else "transition"
    print(f"Plurora identity check passed in {mode} mode")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
