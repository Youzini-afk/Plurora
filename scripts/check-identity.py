#!/usr/bin/env python3
"""Enforce Plurora's durable public and repository identity invariants."""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Iterable

ROOT = pathlib.Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Redline:
    name: str
    pattern: re.Pattern[str]


def literal(name: str, *parts: str, flags: int = 0) -> Redline:
    return Redline(name, re.compile(re.escape("".join(parts)), flags))


def regex(name: str, *parts: str, flags: int = 0) -> Redline:
    return Redline(name, re.compile("".join(parts), flags))


# Build retired identities from fragments so this checker can scan its own source.
OLD_LONG_LOWER = "y" + "gg" + "drasil"
OLD_LONG_TITLE = "Y" + "gg" + "drasil"
OLD_SHORT = "y" + "gg"
OLD_TINY = "y" + "g"
OLD_PUBLISHER_ROLE = "off" + "icial"

REDLINES: tuple[Redline, ...] = (
    literal("retired title brand", OLD_LONG_TITLE),
    literal("retired lower brand", OLD_LONG_LOWER),
    literal("retired npm scope", "@", OLD_LONG_LOWER),
    literal("retired environment prefix", "Y", "GG_"),
    literal("retired browser bootstrap", "__", "Y", "GG", "_RUNTIME__"),
    literal("retired home directory", "~/.", OLD_LONG_LOWER),
    literal("retired lock schema", OLD_LONG_LOWER, ".lock"),
    literal("retired URN", "urn:", OLD_LONG_LOWER),
    literal("retired media type", "application/vnd.", OLD_LONG_LOWER),
    regex(
        "retired protocol prefix",
        r"\b",
        OLD_SHORT,
        r"\.(?:contract|shell|change|world|runtime)",
        flags=re.IGNORECASE,
    ),
    regex("retired hyphen prefix", r"\b", OLD_SHORT, r"-", flags=re.IGNORECASE),
    regex("retired underscore prefix", r"\b", OLD_SHORT, r"_", flags=re.IGNORECASE),
    literal("retired Rust SDK", OLD_TINY, "-kernel-sdk"),
    regex(
        "retired CLI token",
        r"(?<![A-Za-z0-9_])",
        OLD_SHORT,
        r"(?![A-Za-z0-9_])",
    ),
    regex(
        "retired short CLI token",
        r"(?<![A-Za-z0-9_])",
        OLD_TINY,
        r"(?![A-Za-z0-9_])",
    ),
    literal("retired contract method prefix", "kernel", ".v1"),
    literal("retired contract event prefix", "kernel", "/v1"),
    literal("retired adapter type", "Legacy", "Adapter"),
    literal("retired TypeScript adapter", "legacy", "Kernel", "V1"),
    literal("retired Rust adapter", "legacy", "_kernel_v1"),
    literal("retired migration command", "contract ", "migrate"),
    literal("retired first-party publisher", OLD_PUBLISHER_ROLE, "/"),
    regex(
        "retired uppercase publisher role",
        r"\b",
        OLD_PUBLISHER_ROLE.upper(),
        r"_[A-Z0-9_]+\b",
    ),
    regex(
        "retired snake-case publisher role",
        r"\b",
        OLD_PUBLISHER_ROLE,
        r"_[A-Za-z0-9_]+\b",
    ),
    regex(
        "retired CamelCase publisher role",
        r"\b",
        OLD_PUBLISHER_ROLE.title(),
        r"[A-Z][A-Za-z0-9_]*\b",
    ),
    literal("retired inventory category", '"', OLD_PUBLISHER_ROLE.upper(), '"'),
    literal("retired target authorization scheme", "Y", "ggTarget"),
    literal("retired pairing credential", OLD_SHORT, "pair."),
    literal("retired access credential", OLD_SHORT, "access."),
    literal("retired enrollment credential", OLD_SHORT, "enroll."),
    literal("retired agent credential", OLD_SHORT, "agent."),
    literal("retired browser bridge", "window.", OLD_SHORT, "Host"),
)

METHOD_OWNERS = {
    "context": "substrate",
    "journal": "substrate",
    "capability": "substrate",
    "authority": "substrate",
    "object": "substrate",
    "identity": "substrate",
    "host": "host",
    "protocol": "protocol",
    "change": "protocol",
    "projection": "protocol",
    "shell": "shell",
}

EXPECTED_METHOD_PREFIX_COUNTS = {
    "authority": 7,
    "capability": 5,
    "change": 6,
    "context": 6,
    "host": 52,
    "identity": 1,
    "journal": 3,
    "object": 3,
    "projection": 4,
    "protocol": 3,
    "shell": 2,
}

EXPECTED_TOP_LEVEL_SCHEMAS = {
    "active-binding-record.schema.json",
    "artifact-descriptor.schema.json",
    "assembly-lock.schema.json",
    "assembly-revision.schema.json",
    "capability-descriptor.schema.json",
    "capability-invocation-request.schema.json",
    "capability-invocation-result.schema.json",
    "change-set.schema.json",
    "commit.schema.json",
    "component-descriptor.schema.json",
    "contract-selection.schema.json",
    "effect-receipt.schema.json",
    "event-envelope.schema.json",
    "exposure-record.schema.json",
    "installation-record.schema.json",
    "installation-state-authority-evidence.schema.json",
    "installation-state-decision-receipt.schema.json",
    "installation-state-snapshot.schema.json",
    "intent.schema.json",
    "manifest.schema.json",
    "operational-intent.schema.json",
    "package-envelope-descriptor.schema.json",
    "permission-set.schema.json",
    "policy-decision.schema.json",
    "port-descriptor.schema.json",
    "protocol-context.schema.json",
    "protocol-descriptor.schema.json",
    "protocol-response.schema.json",
    "realization-plan.schema.json",
    "realization-revision.schema.json",
    "rights-declaration.schema.json",
    "run-record.schema.json",
    "state-slot-descriptor.schema.json",
    "target-inventory.schema.json",
    "transparency-declaration.schema.json",
    "work-revision.schema.json",
    "world-bundle.schema.json",
    "world-head.schema.json",
    "world-journal-range.schema.json",
}

EXPECTED_POSITIVE_MARKERS = {
    "crates/plurora-cli/src/cli.rs": ('#[command(name = "plurora")]',),
    "crates/plurora-core/src/paths.rs": ('"PLURORA_DATA_DIR"', 'join(".plurora")'),
    "clients/web/src/client-core/host-endpoint.ts": ("__PLURORA_RUNTIME__", "PluroraRuntimeBootstrap"),
    "clients/web/public/surface-frame-bootstrap.js": ("window.pluroraHost",),
    "clients/desktop/src-tauri/tauri.conf.json": ('"productName": "Plurora"', '"identifier": "io.github.youzini-afk.plurora"'),
    "docs/spec/v1/ERROR_CODES.en.md": ("protocol/error/unsupported_contract", "runtime/error/schema_invalid"),
}

FORBIDDEN_PATHS = {
    "docs/roadmap/PLURORA_RENAME.md",
    "docs/roadmap/PLURORA_RENAME.en.md",
    "scripts/plurora-rename-map.json",
    "scripts/check-plurora-identity.py",
    "docs/spec/KERNEL_V1_CONTRACT.md",
    "docs/spec/KERNEL_V1_CONTRACT.en.md",
    "docs/protocol/PROTOCOL_V0.md",
    "docs/protocol/PROTOCOL_V0.en.md",
    "docs/architecture/PLATFORM_KERNEL.md",
    "docs/architecture/PLATFORM_KERNEL.en.md",
    "crates/plurora-cli/src/commands/composition.rs",
    "docs/spec/v1/schemas/composition-lock.schema.json",
    "packages/plurora/composition-lab/manifest.yaml",
    "examples/bundles/playable-creation-board-composition-bundle/bundle.json",
}

RETIRED_COMPOSITION_REFERENCES = (
    "Composition" + "Descriptor",
    "Composition" + "Lock",
    "composition" + "_lock",
    "init-" + "composition",
    "composition" + " check",
    "composition" + "-lab",
    "composition" + "_lab",
    "Composition" + " Lab",
    "export_" + "composition" + "_bundle",
    "import_" + "composition" + "_bundle",
    "composition" + "_bundle",
    "composition" + "_manifest",
    "composition" + "_id",
    "composition" + "_launch_plan",
    "composition" + "_permission_preview",
    "composition" + "_surface_graph",
    "composition" + "_compat_report",
    "asset:" + "composition" + ":",
    "bundle:" + "composition" + ":",
)
COMPOSITION_MIGRATION_BRIEFS = {
    "docs/roadmap/WORK_ASSEMBLY_REALIZATION.md",
    "docs/roadmap/WORK_ASSEMBLY_REALIZATION.en.md",
}

RETIRED_PROJECT_REFERENCES = (
    literal("retired Project method namespace", "host", ".", "project"),
    regex("retired Project event namespace", r"host/", "project", r"(?:[./]|$)"),
    literal("retired Project descriptor", "Project", "Descriptor"),
    literal("retired Project registry", "Project", "Registry"),
)
PROJECT_MIGRATION_BRIEFS = COMPOSITION_MIGRATION_BRIEFS


def repository_paths() -> list[str]:
    raw = subprocess.check_output(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=ROOT,
    )
    return [part.decode("utf-8") for part in raw.split(b"\0") if part]


def tracked_texts(paths: Iterable[str]) -> Iterable[tuple[str, str]]:
    for relative in paths:
        path = ROOT / relative
        if not path.is_file():
            continue
        try:
            yield relative, path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def check_redlines(paths: list[str], texts: list[tuple[str, str]]) -> list[str]:
    errors: list[str] = []
    for redline in REDLINES:
        matches: list[str] = []
        for relative in paths:
            match = redline.pattern.search(relative)
            if match:
                matches.append(f"path:{relative}")
        for relative, text in texts:
            for match in redline.pattern.finditer(text):
                matches.append(f"{relative}:{line_number(text, match.start())}")
                if len(matches) >= 12:
                    break
            if len(matches) >= 12:
                break
        if matches:
            errors.append(f"{redline.name}: {', '.join(matches)}")
    return errors


def load_json(path: pathlib.Path) -> dict[str, object]:
    with path.open(encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected object in {path.relative_to(ROOT)}")
    return value


def method_id_from_schema(path: pathlib.Path, schema: dict[str, object]) -> str:
    properties = schema.get("properties")
    metadata = schema.get("x-plurora-contract")
    if not isinstance(properties, dict) or not isinstance(metadata, dict):
        raise ValueError(f"missing method properties or metadata in {path.name}")
    method = properties.get("method")
    if not isinstance(method, dict) or not isinstance(method.get("const"), str):
        raise ValueError(f"missing method const in {path.name}")
    method_id = method["const"]
    if metadata.get("id") != method_id:
        raise ValueError(f"metadata id mismatch in {path.name}")
    if schema.get("title") != method_id:
        raise ValueError(f"title mismatch in {path.name}")
    if path.name != f"{method_id}.schema.json":
        raise ValueError(f"filename mismatch for {method_id}")
    expected_urn = f"urn:plurora:schema:method:{method_id}:v1"
    if schema.get("$id") != expected_urn:
        raise ValueError(f"schema URN mismatch for {method_id}")
    prefix = method_id.split(".", 1)[0]
    owner = METHOD_OWNERS.get(prefix)
    if owner is None:
        raise ValueError(f"unknown method owner prefix: {method_id}")
    if metadata.get("owner_layer") != owner:
        raise ValueError(f"owner layer mismatch for {method_id}")
    forbidden_metadata = {"aliases", "alias", "replacement", "deprecated_in", "support_until", "legacy"}
    present = forbidden_metadata.intersection(metadata)
    if present:
        raise ValueError(f"compatibility metadata {sorted(present)} present for {method_id}")
    return method_id


def event_kind_from_schema(path: pathlib.Path, schema: dict[str, object]) -> str:
    properties = schema.get("properties")
    if not isinstance(properties, dict):
        raise ValueError(f"missing event properties in {path.name}")
    kind = properties.get("kind")
    if not isinstance(kind, dict) or not isinstance(kind.get("const"), str):
        raise ValueError(f"missing event kind const in {path.name}")
    event_kind = kind["const"]
    if schema.get("title") != event_kind:
        raise ValueError(f"event title mismatch in {path.name}")
    if path.name != f"{event_kind.replace('/', '__')}.schema.json":
        raise ValueError(f"event filename mismatch for {event_kind}")
    if schema.get("$id") != f"urn:plurora:schema:event:{event_kind}:v1":
        raise ValueError(f"event schema URN mismatch for {event_kind}")
    return event_kind


def check_generated_contract() -> list[str]:
    errors: list[str] = []
    method_dir = ROOT / "docs/spec/v1/schemas/methods"
    event_dir = ROOT / "docs/spec/v1/schemas/events"
    top_dir = ROOT / "docs/spec/v1/schemas"

    protocol_source = (ROOT / "crates/plurora-runtime/src/protocol.rs").read_text(encoding="utf-8")
    source_method_ids = {
        value
        for value in re.findall(r'Self::[A-Za-z0-9_]+\s*=>\s*"([a-z][a-z0-9_.]+)"', protocol_source)
        if value.split(".", 1)[0] in METHOD_OWNERS
    }
    event_source = (ROOT / "crates/plurora-core/src/event.rs").read_text(encoding="utf-8")
    event_constants = dict(
        re.findall(
            r'pub const ([A-Z][A-Z0-9_]+):\s*&str\s*=\s*"([a-z][a-z0-9_./-]+)";',
            event_source,
        )
    )
    registry_match = re.search(
        r"pub const PLATFORM_EVENT_KINDS:\s*&\[&str\]\s*=\s*&\[(.*?)\];",
        event_source,
        flags=re.DOTALL,
    )
    if registry_match is None:
        source_event_kinds: set[str] = set()
        errors.append("core PLATFORM_EVENT_KINDS registry is missing")
    else:
        registry_names = re.findall(
            r"^\s*([A-Z][A-Z0-9_]+),\s*$",
            registry_match.group(1),
            flags=re.MULTILINE,
        )
        unknown_names = sorted(set(registry_names) - event_constants.keys())
        if unknown_names:
            errors.append(f"core event registry contains unknown constants: {unknown_names}")
        source_event_kinds = {
            event_constants[name] for name in registry_names if name in event_constants
        }
    if len(source_method_ids) != 92:
        errors.append(f"expected 92 runtime method identities, found {len(source_method_ids)}")
    if len(source_event_kinds) != 69:
        errors.append(f"expected 69 core event identities, found {len(source_event_kinds)}")

    method_paths = sorted(method_dir.glob("*.schema.json"))
    event_paths = sorted(event_dir.glob("*.schema.json"))
    top_paths = sorted(top_dir.glob("*.schema.json"))
    if len(method_paths) != len(source_method_ids):
        errors.append(
            f"expected {len(source_method_ids)} method schemas from the runtime registry, "
            f"found {len(method_paths)}"
        )
    if len(event_paths) != len(source_event_kinds):
        errors.append(
            f"expected {len(source_event_kinds)} event schemas from the core registry, "
            f"found {len(event_paths)}"
        )
    actual_top_level_schemas = {path.name for path in top_paths}
    missing_top_level = EXPECTED_TOP_LEVEL_SCHEMAS - actual_top_level_schemas
    extra_top_level = actual_top_level_schemas - EXPECTED_TOP_LEVEL_SCHEMAS
    if missing_top_level or extra_top_level:
        errors.append(
            "top-level schema set differs: "
            f"missing={sorted(missing_top_level)}, extra={sorted(extra_top_level)}"
        )
    expected_total = len(source_method_ids) + len(source_event_kinds) + len(EXPECTED_TOP_LEVEL_SCHEMAS)
    if len(method_paths) + len(event_paths) + len(top_paths) != expected_total:
        errors.append(f"expected {expected_total} total public-contract schemas")

    method_ids: set[str] = set()
    method_statuses: dict[str, str] = {}
    prefix_counts: dict[str, int] = {}
    for path in method_paths:
        try:
            schema = load_json(path)
            method_id = method_id_from_schema(path, schema)
            metadata = schema["x-plurora-contract"]
            if not isinstance(metadata, dict) or not isinstance(metadata.get("implementation_status"), str):
                raise ValueError(f"missing implementation status in {path.name}")
            implementation_status = metadata["implementation_status"]
        except (ValueError, json.JSONDecodeError) as exc:
            errors.append(str(exc))
            continue
        if method_id in method_ids:
            errors.append(f"duplicate method id: {method_id}")
        method_ids.add(method_id)
        method_statuses[method_id] = implementation_status
        prefix = method_id.split(".", 1)[0]
        prefix_counts[prefix] = prefix_counts.get(prefix, 0) + 1
    if prefix_counts != EXPECTED_METHOD_PREFIX_COUNTS:
        errors.append(f"method prefix counts differ: {prefix_counts}")

    method_row_pattern = re.compile(
        r"^\| `([^`]+)` \| (implemented|partial|planned) \|",
        flags=re.MULTILINE,
    )
    for relative in ("docs/spec/PUBLIC_CONTRACT.en.md", "docs/spec/PUBLIC_CONTRACT.md"):
        contract_text = (ROOT / relative).read_text(encoding="utf-8")
        all_rows = dict(method_row_pattern.findall(contract_text))
        documented = {
            method_id: status
            for method_id, status in all_rows.items()
            if method_id.split(".", 1)[0] in METHOD_OWNERS
        }
        unknown = set(documented) - method_ids
        missing = method_ids - set(documented)
        if unknown or missing:
            errors.append(
                f"{relative} method matrix differs from schemas: "
                f"doc_only={sorted(unknown)}, schema_only={sorted(missing)}"
            )
        status_mismatches = {
            method_id: (method_statuses[method_id], documented[method_id])
            for method_id in method_ids.intersection(documented)
            if method_statuses[method_id] != documented[method_id]
        }
        if status_mismatches:
            errors.append(f"{relative} method status mismatches: {status_mismatches}")

    if source_method_ids != method_ids:
        errors.append(
            "runtime method registry differs from schemas: "
            f"source_only={sorted(source_method_ids - method_ids)}, "
            f"schema_only={sorted(method_ids - source_method_ids)}"
        )

    event_kinds: set[str] = set()
    for path in event_paths:
        try:
            event_kind = event_kind_from_schema(path, load_json(path))
        except (ValueError, json.JSONDecodeError) as exc:
            errors.append(str(exc))
            continue
        if event_kind in event_kinds:
            errors.append(f"duplicate event kind: {event_kind}")
        event_kinds.add(event_kind)

    if source_event_kinds != event_kinds:
        errors.append(
            "runtime event registry differs from schemas: "
            f"source_only={sorted(source_event_kinds - event_kinds)}, "
            f"schema_only={sorted(event_kinds - source_event_kinds)}"
        )

    registry_text = (ROOT / "docs/spec/v1/EVENT_KIND_REGISTRY.en.md").read_text(encoding="utf-8")
    documented_event_kinds = set(re.findall(r"^\| `([^`]+)` \|", registry_text, flags=re.MULTILINE))
    if documented_event_kinds != event_kinds:
        errors.append(
            "event registry document differs from schemas: "
            f"doc_only={sorted(documented_event_kinds - event_kinds)}, "
            f"schema_only={sorted(event_kinds - documented_event_kinds)}"
        )
    return errors


def manifest_identity(path: pathlib.Path) -> tuple[str | None, list[str]]:
    package_id: str | None = None
    provides: list[str] = []
    in_provides = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("id:") and package_id is None:
            package_id = line.split(":", 1)[1].strip().strip('"\'')
        if line == "provides:":
            in_provides = True
            continue
        if in_provides and line and not line.startswith((" ", "\t")):
            in_provides = False
        if in_provides:
            match = re.match(r"\s+- id:\s*([^\s#]+)", line)
            if match:
                provides.append(match.group(1).strip('"\''))
    return package_id, provides


def check_first_party_manifests() -> list[str]:
    errors: list[str] = []
    root = ROOT / "packages/plurora"
    manifests = sorted(root.glob("*/manifest.yaml"))
    if len(manifests) != 34:
        errors.append(f"expected 34 first-party manifests, found {len(manifests)}")
    for manifest in manifests:
        package_id, provides = manifest_identity(manifest)
        expected = f"plurora/{manifest.parent.name}"
        if package_id != expected:
            errors.append(f"{manifest.relative_to(ROOT)} id is {package_id!r}, expected {expected!r}")
            continue
        for capability_id in provides:
            if not capability_id.startswith(f"{package_id}/"):
                errors.append(
                    f"{manifest.relative_to(ROOT)} provides capability outside its namespace: {capability_id}"
                )
    return errors


def check_positive_markers() -> list[str]:
    errors: list[str] = []
    for relative, markers in EXPECTED_POSITIVE_MARKERS.items():
        text = (ROOT / relative).read_text(encoding="utf-8")
        for marker in markers:
            if marker not in text:
                errors.append(f"missing positive identity marker {marker!r} in {relative}")
    return errors


def check_forbidden_paths(paths: Iterable[str]) -> list[str]:
    paths = {path for path in paths if (ROOT / path).exists()}
    present = sorted(FORBIDDEN_PATHS.intersection(paths))
    present.extend(sorted(path for path in paths if path.endswith("/composition.yaml")))
    return [f"retired or temporary identity path is present: {path}" for path in present]


def check_retired_composition_references(texts: Iterable[tuple[str, str]]) -> list[str]:
    errors: list[str] = []
    for relative, text in texts:
        if relative in COMPOSITION_MIGRATION_BRIEFS or relative == "scripts/check-identity.py":
            continue
        for reference in RETIRED_COMPOSITION_REFERENCES:
            offset = text.find(reference)
            if offset >= 0:
                errors.append(
                    f"retired Composition identity {reference!r}: "
                    f"{relative}:{line_number(text, offset)}"
                )
    return errors


def check_retired_project_references(texts: Iterable[tuple[str, str]]) -> list[str]:
    """Reject retired machine identities from implementation and generated contracts."""
    errors: list[str] = []
    source_roots = ("crates/", "clients/", "packages/", "profiles/", "examples/", "sdk/")
    schema_root = "docs/spec/v1/schemas/"
    for relative, text in texts:
        if relative in PROJECT_MIGRATION_BRIEFS:
            continue
        if not (relative.startswith(source_roots) or relative.startswith(schema_root)):
            continue
        for redline in RETIRED_PROJECT_REFERENCES:
            match = redline.pattern.search(text)
            if match:
                errors.append(
                    f"{redline.name}: {relative}:{line_number(text, match.start())}"
                )
    return errors


def main() -> int:
    paths = repository_paths()
    texts = list(tracked_texts(paths))
    errors = []
    errors.extend(check_forbidden_paths(paths))
    errors.extend(check_redlines(paths, texts))
    errors.extend(check_retired_composition_references(texts))
    errors.extend(check_retired_project_references(texts))
    errors.extend(check_generated_contract())
    errors.extend(check_first_party_manifests())
    errors.extend(check_positive_markers())

    if errors:
        print(f"Plurora identity check failed: {len(errors)} issue(s)", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1

    print(
        "Plurora identity check passed: "
        f"zero retired identities; 92 methods, 69 events, {len(EXPECTED_TOP_LEVEL_SCHEMAS)} top-level schemas; "
        "34 first-party Package manifests."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
