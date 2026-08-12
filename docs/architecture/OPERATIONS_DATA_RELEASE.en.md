# Operations, Data, and Release Safety

> [English](./OPERATIONS_DATA_RELEASE.en.md) · [中文](./OPERATIONS_DATA_RELEASE.md)

Status: **The operational-safety baseline is implemented; remaining hardening stays governed by this contract**. This document defines the data, health, diagnostics, upgrade, and release baseline required while a Host carries real Works and remote Targets.

## Current implementation status

Implemented:

- An Install Lab store schema mismatch no longer deletes data. The old store is atomically moved to a versioned, uniquely suffixed preservation directory before a fresh store receives the current marker.
- `plurora host backup` creates an offline directory snapshot for a SQLite Host profile whose relative database path is inside the data directory. It first acquires the durable Host control-plane lease, excludes the explicit `cache`, uses SQLite's online backup API, and writes a SHA-256 manifest for secrets, keys, objects, installations, profiles, and journals copied under the same lease boundary.
- `plurora host restore` only targets a nonexistent data directory. It rejects traversal, symlinks, duplicate entries, checksum/schema mismatches, verifies SQLite integrity in staging, and only then atomically publishes the restored directory.
- `/livez`, `/health`, and `/healthz` are compatibility liveness endpoints. `/readyz` returns structured status without resource identifiers: event-store or Host control-plane lease failure is `503/unready`; an unhealthy durable Realization is `200/degraded`.
- `host.diagnostics` includes the Host version and aggregate runtime counts without adding Work, Installation, route, or lease identifiers.
- A tag release explicitly reuses the complete CI workflow, strictly validates tag/commit/Cargo/npm/Tauri version identity, and then runs platform builds. Each platform publishes SHA-256 checksums and an SPDX SBOM and records GitHub OIDC/Sigstore provenance and SBOM attestations. Only the build job receives release permissions.

Still pending: a general migration ledger, PostgreSQL backup references, separate `backup inspect/verify` commands, authenticated `/host/v1/status` and diagnostics export, active object/secret probes, one continuous HTTP health-policy parser, clean-runner installer startup smoke, reviewed-SHA pinning for Actions/toolchains, and platform signing/notarization. Releases remain drafts and must not be represented as signed when signing is not configured.

## Classify data before migrating it

Every data class declares its source of truth, rebuildability, consistency boundary, schema version, retention, and restore order.

| Class | Default property | Backup rule |
|---|---|---|
| Event and Host control journals | Authoritative, append-only | Required; preserve sequence/CAS semantics |
| Object store | May be referenced by journals/descriptors | Same backup set as referencing journals |
| Secret store and key | Sensitive and not reconstructible | Paired encrypted backup with strict permissions |
| Installation record/state/managed workspace | User-bearing data | Required unless Installation policy excludes it |
| Profiles/lockfiles/keys | Runtime and supply-chain configuration | Required with permissions/version |
| Realization intents/revisions/receipts | Recovery and rollback truth | Required and journal-consistent |
| Download/build cache | Reconstructible cache | Excludable only when explicitly classified |
| Package/content store | Conditionally reconstructible | Reset only when every object has a proven source |

“Delete on schema mismatch” is valid only for an explicit cache. A missing marker does not prove cache status.

## Schema migration

Each persistent backend has a monotonic schema version and migration ledger:

```text
MigrationRecord
  component / from_version / to_version / migration_id
  started_at / completed_at
  preflight_digest / backup_ref?
  result / diagnostic_ref?
```

Startup performs read-only discovery, integrity preflight, backup/space/lock checks, re-entrant migration, target validation, and atomic version commit. The Host is not ready while migration is incomplete. Destructive migration requires an explicit operator flag or a verified backup.

## Backup contract

```text
BackupManifest
  format_version
  host_id / created_at / created_by
  application_version / schema_versions{}
  consistency_mode
  included_components[] / excluded_components[]
  files[{path, size, digest, mode?}]
  encrypted_secret_payload_ref?
  journal_heads{}
```

- SQLite uses its online backup API or an exclusive checkpoint, not a blind copy.
- PostgreSQL records an external backup reference and journal heads.
- Objects and journals use a consistent cut and restore-time reachability scan.
- Secret data and key are backed up together, optionally wrapped by an operator key.
- Temporary build/download cache is excluded by default.
- Restore extracts to a new directory, verifies all digests/schema, then switches atomically.
- The old data directory remains a rollback source until acceptance.

Expose `backup create/inspect/verify/restore`. Restore defaults to a stopped Host and empty destination; overwrite is explicit.

## Health semantics

| Endpoint | Authentication | Meaning |
|---|---|---|
| `/livez` | Public minimal response | Process and HTTP reactor respond |
| `/readyz` | Public status only | Hydration/migration complete and required stores available |
| `/host/v1/status` | Host identity | Structured component status and degraded reasons |
| `/host/v1/diagnostics/export` | Explicit diagnostic authority | Redacted diagnostic bundle |

Compatibility `/health` and `/healthz` map to a documented meaning instead of always succeeding.

Readiness checks runtime hydration, event-store basic read/write/CAS, temporary object verification, secret-store status without secret reads, Realization-controller fatal state, profile, and contract registry. Optional failures are degraded; required failures are not-ready. Details require authentication.

## Realization health policy

A revision declares protocol, path, expected status range, interval, timeout, success/failure thresholds, and initial delay. HTTP defaults to 2xx only. Startup and continuous health share one parser. Probes observe and audit; only the Realization Controller applies restart policy. Logs/bodies are bounded and redacted.

## Observability

Minimum structured signals cover request correlation and authority, canonical method and policy decision, Realization operation/step/target/generation/epoch, queue and operation latency, retries/cancels/rollbacks, target heartbeat/tunnel errors, route transitions, journal CAS errors, object verification, backup, and migration.

Metrics exclude project names, secrets, tokens, full query strings, and source. High-cardinality resource IDs live only in controlled traces/logs. Diagnostic bundles contain redacted version/config shape, component status, bounded logs, journal heads, Realization summaries, and integrity results; creation and download are audited.

## Supported Host topology

1. Desktop managed Host: random loopback port, one-time bootstrap, persistent local profile.
2. Local/LAN operator Host: explicit bind, non-empty root credential, firewall restriction.
3. Internet Host: TLS reverse proxy or trusted overlay; raw HTTP visible only to proxy with explicit trusted-proxy policy.

Project public routes and Host control APIs remain separate exposure planes. App-domain configuration never publishes a project or lets proxy headers forge identity.

## Release gate

```text
source commit
  -> contract/schema clean check
  -> locked Rust/Web tests and conformance
  -> desktop sidecar smoke
  -> platform builds
  -> installer smoke
  -> checksums + SBOM + provenance/attestation
  -> signing/notarization where available
  -> draft release
```

- Release explicitly depends on the complete gate.
- Cargo uses the lockfile, Node uses `npm ci`, and toolchains are pinned.
- Actions are pinned to reviewed commit SHAs.
- Permissions are per-job; only publishing gets `contents: write`.
- tag, Cargo, npm, and Tauri versions agree.
- Assets carry checksum, SBOM, and source-commit provenance.
- Installers smoke-test on clean runners.
- Missing signing is labelled unsigned rather than implied trusted.

## Upgrade and rollback

Upgrade performs schema preflight, compatibility checks, and backup policy. Binary rollback and data rollback are distinct; an irreversible migration blocks binary-only downgrade. Release notes state readable schema bounds and topology constraints. Desktop coordinates sidecar and shell versions. CI exercises old data → new migration → backup → restore for each release candidate.

## Completion gate

- authoritative data has no silent destructive reset; cache reset is classified and audited;
- backup/restore preserves secret/object/journal references under fault injection;
- live, ready, and degraded distinguish listener, store, and controller failure;
- a release comes from one gated commit with verifiable provenance;
- supported topologies have smoke tests and runbooks;
- large migration, restore, installer, and cross-platform matrices run only in GitHub CI.
