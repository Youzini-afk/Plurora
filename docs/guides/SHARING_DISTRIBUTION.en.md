# Sharing & Distribution Guide

> [English](./SHARING_DISTRIBUTION.en.md) · [中文](./SHARING_DISTRIBUTION.md)

This document describes shareable, reproducible, and importable composition and Session distribution under the current Contract V1. `official/sharing-lab` is the current ordinary Package and Component implementation; long-term shared meaning may enter an optional Sharing Protocol rather than the constitutional substrate.

## Core Principles

- Share first, marketplace later: the current implementation is local/file-level sharing. It covers composition bundles, branch/session bundle manifests, package-set lockfiles, compatibility/migration reports, AI disclosure metadata, read-only shared session manifests, and async fork sharing plans.
- No marketplace: no package signing network, dependency resolver economy, or hosted billing. Distribution is local file exchange, not a commercial marketplace.
- No `kernel.v1.sharing.*`: shared Sharing meaning belongs to an optional Protocol, while concrete import/export behavior is implemented by ordinary Components and Products rather than the constitutional substrate.
- No raw secrets: only `secret_ref` references are allowed in bundles; raw secrets are never stored.
- No public network required: the current sharing fixtures use local files and need no remote service or public network access.

## Sharing Contract

`official/sharing-lab` provides 9 capabilities and 3 surfaces (forge_panel, assistant_action, home_card). The core contract:

| Capability | Purpose |
|---|---|
| `describe_sharing_contract` | Describe the sharing contract: capabilities, surfaces, output shapes, red-line constraints |
| `export_composition_bundle` | Export a composition as a self-contained bundle: manifest + lockfile + disclosure |
| `import_composition_bundle` | Import a bundle, validating shape, compatibility, and no-raw-secrets constraints |
| `create_branch_session_bundle` | Create a branch/session bundle manifest for sharing a specific session state |
| `create_package_set_lockfile` | Create a package-set lockfile pinning exact package versions and content addresses |
| `compatibility_report` | Produce a compatibility/migration report between two bundle versions or package sets |
| `ai_disclosure_bundle` | Produce AI disclosure metadata bundle for composition or session content |
| `read_only_share_manifest` | Create a read-only shared session manifest (current local/file-level implementation) |
| `async_fork_share_plan` | Create an async fork sharing plan (current local implementation for deferred/async session fork sharing) |

## Bundle Shapes

### Composition Bundle

```json
{
  "bundle_id": "bundle:<composition_id>:<content_address>",
  "format_version": "1",
  "composition_id": "...",
  "composition_manifest": { ... },
  "package_set_lockfile": {
    "lockfile_id": "lockfile:<content_address>",
    "format_version": "1",
    "packages": [
      { "package_id": "...", "version": "...", "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000" }
    ],
    "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
  },
  "ai_disclosure": {
    "disclosure_id": "disclosure:<bundle_id>",
    "items": [
      { "content_ref": "...", "disclosure_kind": "ai_generated|ai_assisted|human_created|mixed", "description": "..." }
    ],
    "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
  },
  "no_marketplace_fields": true,
  "no_billing_fields": true,
  "no_signing_network_fields": true
}
```

### Branch/Session Bundle

```json
{
  "bundle_id": "branch-bundle:<session_id>:<branch_ref>:<content_address>",
  "format_version": "1",
  "session_id": "...",
  "branch_ref": "branch:main",
  "sequence": 42,
  "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "ai_disclosure": { ... }
}
```

### Package-Set Lockfile

```json
{
  "lockfile_id": "lockfile:<content_address>",
  "format_version": "1",
  "packages": [
    { "package_id": "...", "version": "...", "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000" }
  ],
  "content_address": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
}
```

### Compatibility Report

```json
{
  "report_id": "compat-report:<source>:<content_address>",
  "source_ref": "bundle:v1",
  "target_ref": "bundle:v2",
  "status": "compatible|minor_incompatibility|major_incompatibility|migration_required",
  "incompatibilities": [
    { "package_id": "...", "kind": "missing_in_target|version_mismatch|added_in_target", "severity": "minor|major" }
  ],
  "migration_steps": [ { "action": "...", "package_id": "..." } ]
}
```

## AI Disclosure

Every bundle can carry AI disclosure metadata marking content provenance:

| `disclosure_kind` | Meaning |
|---|---|
| `ai_generated` | Content fully AI-generated |
| `ai_assisted` | Human-authored with AI assistance |
| `human_created` | Human original content |
| `ai_reviewed` | Human-authored with AI review |
| `mixed` | Mixed provenance |
| `undisclosed` | Provenance not disclosed |

## Read-Only Sharing & Async Fork

Read-only sharing (`read_only_share_manifest`): Creates a read-only snapshot proof of a session that can be viewed but not modified by recipients. `share_scope: local_file`, `no_remote_service: true`.

Async fork sharing (`async_fork_share_plan`): Creates an async fork plan allowing recipients to later fork their own session. Status is `draft`, `plan_only: true`, requires user approval.

## Red Lines

The following are explicitly forbidden in the sharing contract:

- Marketplace fields (`marketplace_id`, `marketplace_category`)
- Billing fields (`billing_token`, `payment_method`, `subscription`)
- Signing network fields (`signing_network`, `license_key`)
- Raw secrets (`api_key`, `token`, `password` raw values; only `secret_ref` references allowed)
- Kernel sharing namespaces (`kernel.v1.sharing.*`, `kernel.v1.marketplace.*`, `kernel.v1.billing.*`)
- Public network or remote service dependency

## Examples

See `examples/bundles/playable-creation-board-composition-bundle/` for a complete example containing:
- `bundle.json` — composition bundle + lockfile + compatibility report + AI disclosure
- `branch-session-bundle.json` — branch/session bundle manifest
- `read-only-share-manifest.json` — read-only shared session manifest
- `async-fork-share-plan.json` — async fork sharing plan

## Verification

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
```

Validation covers contract shape, export/import, lockfile, compatibility report, AI disclosure, read-only sharing, async fork, and red-line constraints.
