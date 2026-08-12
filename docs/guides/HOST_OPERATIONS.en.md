# Host operations runbook

> [English](./HOST_OPERATIONS.en.md) · [中文](./HOST_OPERATIONS.md)

This runbook covers the current operational baseline for a local SQLite Host. PostgreSQL backup, zero-downtime online backup, and in-place overwrite restore are not yet provided.

## Health checks

- `GET /livez`: process and HTTP-reactor liveness with body `ok`.
- `GET /health` and `GET /healthz`: compatibility liveness aliases.
- `GET /readyz`: public, redacted structured readiness. An event-store or Host control-plane lease failure returns HTTP 503. If the Host can accept control requests but a durable Realization is unhealthy, it returns HTTP 200 with `status: "degraded"`.

Do not use liveness to decide whether mutations are safe. Orchestrators should use the `/readyz` HTTP status and `ready` field.

## Create a backup

Prerequisites:

1. Stop the desktop or CLI Host so an external installer cannot mutate the data directory concurrently.
2. The profile is inside the data directory, uses `event_store.kind: sqlite`, and has an event-store path relative to the profile.
3. The output directory does not exist and is outside the data directory.

```bash
plurora host backup \
  --data-dir /srv/plurora \
  --profile /srv/plurora/profiles/host.yaml \
  --output /srv/backups/plurora-2026-07-23
```

The command acquires the durable Host control-plane lease. It fails without copying if another Host still owns that lease. The snapshot excludes top-level `cache/`, refuses symlinks in the data directory, and records every file below `data/` with size and SHA-256 in `manifest.json`. SQLite is captured with the online backup API rather than by copying a live database file.

## Restore and accept

Restore only targets a new, nonexistent directory:

```bash
plurora host restore \
  --backup /srv/backups/plurora-2026-07-23 \
  --data-dir /srv/plurora-restored
```

Restore validates the manifest, paths, file types, sizes, SHA-256 digests, profile-to-SQLite reference, and SQLite integrity in a sibling staging directory. It atomically renames staging only after every check succeeds. Start the Host with the restored data directory and its profile, confirm `/readyz`, then verify critical Works, secret references, and Realization history. Keep the old data directory until acceptance is complete.

## Verify a release

A `v*` tag cannot bypass CI. The release workflow first reuses the complete Contract/Rust/Web/Desktop gate and checks that the tag, exact commit, and every Cargo/npm/Tauri version agree. Platform builds begin only after the gate. The draft release contains installers plus per-platform `SHA256SUMS` and SPDX SBOM files, with provenance and SBOM attestations bound to installer digests.

After downloading:

```bash
sha256sum -c Plurora-<target>-SHA256SUMS.txt
gh attestation verify <installer> -R Youzini-afk/Plurora
```

Platform signing/notarization is not configured yet, so the draft must not be represented as a signed release.
