#!/usr/bin/env python3
"""Check repository Markdown links and bilingual document pairs.

The checker is intentionally mechanical. It does not judge architecture,
wording, or product direction; those remain review responsibilities.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
SKIP_PARTS = {".git", "node_modules", "target", "dist"}

INLINE_LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
REFERENCE_LINK_RE = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)", re.MULTILINE)
HTML_LINK_RE = re.compile(r"(?:href|src)=[\"']([^\"']+)[\"']", re.IGNORECASE)
INLINE_CODE_RE = re.compile(r"`[^`]*`")
EXTERNAL_SCHEMES = (
    "http://",
    "https://",
    "mailto:",
    "tel:",
    "data:",
    "javascript:",
)


def markdown_files() -> list[Path]:
    return sorted(
        path
        for path in ROOT.rglob("*.md")
        if not any(part in SKIP_PARTS for part in path.parts)
    )


def strip_code(text: str) -> str:
    """Remove fenced and inline code before looking for Markdown links."""
    output: list[str] = []
    fence: str | None = None

    for line in text.splitlines():
        stripped = line.lstrip()
        if fence is None and (stripped.startswith("```") or stripped.startswith("~~~")):
            fence = stripped[:3]
            output.append("")
            continue
        if fence is not None:
            if stripped.startswith(fence):
                fence = None
            output.append("")
            continue
        output.append(INLINE_CODE_RE.sub("", line))

    return "\n".join(output)


def normalize_link_target(raw: str) -> str:
    value = raw.strip()
    if value.startswith("<") and ">" in value:
        value = value[1 : value.index(">")]
    elif " " in value:
        # Markdown allows an optional title after the target. Repository-local
        # paths use percent encoding rather than unescaped spaces.
        value = value.split(None, 1)[0]
    return unquote(value)


def check_links(files: list[Path]) -> tuple[list[str], int]:
    errors: list[str] = []
    checked = 0

    for path in files:
        text = strip_code(path.read_text(encoding="utf-8"))
        targets = [match.group(1) for match in INLINE_LINK_RE.finditer(text)]
        targets.extend(match.group(1) for match in REFERENCE_LINK_RE.finditer(text))
        targets.extend(match.group(1) for match in HTML_LINK_RE.finditer(text))

        for raw in targets:
            target = normalize_link_target(raw)
            lower = target.lower()
            if (
                not target
                or target.startswith("#")
                or target.startswith("/")
                or lower.startswith(EXTERNAL_SCHEMES)
            ):
                continue

            local_target = target.split("#", 1)[0].split("?", 1)[0]
            if not local_target:
                continue

            checked += 1
            candidate = (path.parent / local_target).resolve()
            try:
                candidate.relative_to(ROOT)
            except ValueError:
                errors.append(
                    f"{path.relative_to(ROOT)}: link escapes repository: {target}"
                )
                continue

            if not candidate.exists():
                errors.append(f"{path.relative_to(ROOT)}: missing link target: {target}")

    return errors, checked


def chinese_counterpart(english_path: Path) -> Path:
    return Path(str(english_path)[: -len(".en.md")] + ".md")


def check_bilingual_pairs(files: list[Path]) -> tuple[list[str], int]:
    errors: list[str] = []
    pairs = 0

    for english_path in files:
        if not english_path.name.endswith(".en.md"):
            continue

        chinese_path = chinese_counterpart(english_path)
        if not chinese_path.exists():
            errors.append(
                f"{english_path.relative_to(ROOT)}: missing Chinese counterpart "
                f"{chinese_path.relative_to(ROOT)}"
            )
            continue

        pairs += 1
        english_head = "\n".join(
            english_path.read_text(encoding="utf-8").splitlines()[:12]
        )
        chinese_head = "\n".join(
            chinese_path.read_text(encoding="utf-8").splitlines()[:12]
        )

        # Some ecosystem-standard README pairs intentionally use a prose
        # language note. When either file opts into the repository's standard
        # switch, require the complete switch in both files.
        uses_standard_switch = any(
            marker in english_head or marker in chinese_head
            for marker in ("[English]", "[中文]")
        )
        if not uses_standard_switch:
            continue

        for path, head in (
            (english_path, english_head),
            (chinese_path, chinese_head),
        ):
            if "[English]" not in head or "[中文]" not in head:
                errors.append(
                    f"{path.relative_to(ROOT)}: incomplete bilingual switch near top"
                )
            if english_path.name not in head or chinese_path.name not in head:
                errors.append(
                    f"{path.relative_to(ROOT)}: bilingual switch does not link both files"
                )

    return errors, pairs


def main() -> int:
    files = markdown_files()
    link_errors, checked_links = check_links(files)
    bilingual_errors, checked_pairs = check_bilingual_pairs(files)
    errors = link_errors + bilingual_errors

    if errors:
        print(f"documentation check failed: {len(errors)} issue(s)", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print(
        "documentation check passed: "
        f"{len(files)} Markdown files, "
        f"{checked_links} local links, "
        f"{checked_pairs} bilingual pairs"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
