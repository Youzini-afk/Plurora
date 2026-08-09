# Sharing & Distribution Guide

> [English](./SHARING_DISTRIBUTION.en.md) · [中文](./SHARING_DISTRIBUTION.md)

This guide describes shareable, verifiable, and importable Work and Session distribution under the current Contract V1. `plurora/sharing-lab` is an ordinary Package and Component; shared meaning may belong to an optional Sharing Protocol, not the constitutional substrate.

## Core principles

- A Work Bundle only transports immutable `WorkRevision` / root `AssemblyRevision` / `AssemblyLock` content references and their closure. `work_id` is a logical name, not a substitute for content digest identity.
- A package-set lockfile is distribution metadata. It does not own Work identity and cannot replace `AssemblyLock`.
- The current implementation exchanges local files only. It introduces no marketplace, signing network, dependency-resolution economy, or hosted billing.
- A bundle never stores a raw secret. A `secret_ref` is an unresolved reference only.
- Import only validates input and produces a user-approval-gated plan. It does not install, run, use the network, or derive execution authority from agent output.

## Sharing contract

`plurora/sharing-lab` provides nine capabilities and three surfaces (`forge_panel`, `assistant_action`, and `home_card`):

| Capability | Purpose |
|---|---|
| `describe_sharing_contract` | Describe capabilities, surfaces, output shapes, and red lines |
| `export_work_bundle` | Export `WorkRevision` + root `AssemblyRevision` + `AssemblyLock` references, distribution locks, and disclosures |
| `import_work_bundle` | Validate a Work Bundle and return a `plan_only` import result |
| `create_branch_session_bundle` | Create a branch/session bundle manifest for specific Session state |
| `create_package_set_lockfile` | Pin exact Package versions and content addresses |
| `compatibility_report` | Compare two bundles or Package sets and produce a compatibility report |
| `ai_disclosure_bundle` | Record AI-origin disclosure for Work, Artifact, or Session content |
| `read_only_share_manifest` | Create a local-file read-only Session share manifest |
| `async_fork_share_plan` | Create an approval-gated asynchronous fork sharing plan |

There is no alias for the retired bundle capabilities, no prior-field reader, and no format fallback.

## Work Bundle wire shape

```json
{
  "kind": "work_bundle",
  "bundle_id": "work-bundle:example/playable-creation-board:sha256:e329fd36961fcf2b2e6a4b5e6796f3b761a602b5a0c7bcbecfeff9fe62b38539",
  "format_version": "1",
  "work_id": "example/playable-creation-board",
  "work_revision": {
    "artifact_type_uri": "urn:plurora:work-revision:v1",
    "media_type": "application/json",
    "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    "size_bytes": 768,
    "references": [
      "sha256:3333333333333333333333333333333333333333333333333333333333333333",
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "assembly_revision": {
    "artifact_type_uri": "urn:plurora:assembly-revision:v1",
    "media_type": "application/json",
    "digest": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    "size_bytes": 896,
    "references": [
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "assembly_lock": {
    "artifact_type_uri": "urn:plurora:assembly-lock:v1",
    "media_type": "application/json",
    "digest": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    "size_bytes": 1024,
    "references": [
      "sha256:3333333333333333333333333333333333333333333333333333333333333333",
      "sha256:4444444444444444444444444444444444444444444444444444444444444444"
    ],
    "annotations": {}
  },
  "package_set_lockfile": {
    "lockfile_id": "lockfile:sha256:5c4415f0c1f5b3d7bab60554c5520da7526e845cbe05f421a824479f2d83de90",
    "format_version": "1",
    "packages": [
      {
        "package_id": "plurora/playable-creation-board",
        "version": "0.1.0",
        "content_address": "sha256:1b6d7f605b8d106a43c0f13fb41996552011e0101e10ffae9ea00796edef566e"
      }
    ],
    "content_address": "sha256:5c4415f0c1f5b3d7bab60554c5520da7526e845cbe05f421a824479f2d83de90"
  },
  "ai_disclosure": {
    "disclosure_id": "ai-disclosure:sha256:5758295a886ffad108a6eee14628a7ec02c15b89f31cda336294d4e4c0bf560c",
    "items": [
      {
        "content_ref": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "disclosure_kind": "mixed",
        "description": "Work bundle with AI-generated and human-created content"
      }
    ],
    "content_address": "sha256:5758295a886ffad108a6eee14628a7ec02c15b89f31cda336294d4e4c0bf560c"
  },
  "no_marketplace_fields": true,
  "no_billing_fields": true,
  "no_signing_network_fields": true
}
```

`work_revision`, `assembly_revision`, and `assembly_lock` all use the complete `plurora_core::ArtifactDescriptor` wire shape. The handler reuses `plurora_work` type constants and validators:

- `artifact_type_uri` exactly matches the corresponding type;
- `media_type` is canonical JSON, `application/json`;
- `digest` and every `references[]` item are complete SHA-256 values;
- `size_bytes` is a non-zero integer;
- references are neither duplicated nor self-referential;
- annotations satisfy the portable-value rules;
- both `work_revision` and `assembly_lock` exactly reference `assembly_revision.digest`; a shared ordinary content root cannot impersonate the same Assembly;
- package-set lockfile identity is recomputed from Package pins, AI disclosure identity covers the actual items, and `bundle_id` then covers the three root descriptors, Package pins, and disclosure.

Missing fields, out-of-contract fields, wrong types, incomplete digests, invalid size/reference shapes, and unrelated closures return `sharing_lab_rejected`. The handler never synthesizes a Work from Package names, titles, or defaults.

## Import result

`import_work_bundle` accepts the `kind: work_bundle` shape above. A supported format validates to:

```json
{
  "kind": "work_bundle_import",
  "bundle_id": "...",
  "format_version": "1",
  "work_id": "example/playable-creation-board",
  "work_revision": { "artifact_type_uri": "urn:plurora:work-revision:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 768, "references": ["sha256:..."] },
  "assembly_revision": { "artifact_type_uri": "urn:plurora:assembly-revision:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 896, "references": ["sha256:..."] },
  "assembly_lock": { "artifact_type_uri": "urn:plurora:assembly-lock:v1", "media_type": "application/json", "digest": "sha256:...", "size_bytes": 1024, "references": ["sha256:..."] },
  "ai_disclosure": { "disclosure_id": "ai-disclosure:sha256:...", "items": [{ "content_ref": "sha256:...", "disclosure_kind": "mixed", "description": "..." }], "content_address": "sha256:..." },
  "compatibility_status": "compatible",
  "diagnostics": [],
  "requires_user_approval": true,
  "plan_only": true
}
```

Another `format_version` returns `unsupported`; it does not activate an old-format migration or read retired fields. Missing Packages produce a structured `minor_incompatibility`. The result remains only a plan: a later Installation/resolver flow rechecks current authority, policy, and content digests.

## Session sharing, AI disclosure, and asynchronous fork

Branch/session bundles, read-only share manifests, and asynchronous fork plans retain Session-layer identity and do not pretend to be Work artifacts. Read-only sharing uses `share_scope: local_file` and `no_remote_service: true`. An asynchronous fork returns `status: draft`, `requires_user_approval: true`, and `plan_only: true`.

AI disclosure kinds are `ai_generated`, `ai_assisted`, `human_created`, `ai_reviewed`, `mixed`, and `undisclosed`. A disclosure is a claim, not authority, a billing credential, or a legal judgment.

## Red lines

- Reject marketplace, payment, subscription, billing, signing-network, and license-key fields.
- Reject raw API keys, tokens, and passwords; references only.
- Do not create constitutional `platform.sharing.*`, `platform.marketplace.*`, or `platform.billing.*` namespaces.
- Require no public network, remote service, or hidden first-party authority.
- Export/import, Session sharing, and asynchronous fork perform no external effect.

## Examples and verification

The complete fixture is under `examples/bundles/playable-creation-board-work-bundle/`:

- `bundle.json` — WorkRevision, root AssemblyRevision, AssemblyLock references, package-set lockfile, and AI disclosure;
- `branch-session-bundle.json` — branch/session bundle manifest;
- `read-only-share-manifest.json` — read-only Session share manifest;
- `async-fork-share-plan.json` — asynchronous fork sharing plan.

```bash
cargo test -p plurora-runtime sharing_lab
cargo run -p plurora-cli -- conformance --tag sharing --fail-fast
```

Validation covers capability discovery, Work/Assembly descriptor type and portable shape, export/import, format rejection, lockfiles, compatibility reports, AI disclosure, read-only sharing, asynchronous fork, and red lines.
