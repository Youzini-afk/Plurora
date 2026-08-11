# Installation Model

> [English](./INSTALLATION_MODEL.en.md) · [中文](./INSTALLATION_MODEL.md)

An Installation is one Host's local adoption record for an immutable WorkRevision. It is neither content identity nor a running instance: one WorkRevision can produce Installations on multiple Hosts, and an Installation can later produce multiple Runs.

## Object boundaries

| Object | Owner | Purpose |
|---|---|---|
| WorkRevision | portable artifact | User-recognized work and entrypoints, referencing an AssemblyRevision. |
| AssemblyRevision | portable artifact | Component graph, Ports, Bindings, State Slots, and exposed mappings. |
| AssemblyLock | portable artifact | Resolved nodes, bindings, protocol profiles, and content roots. |
| Workspace | Host-local | Mutable source directory used for authoring or builds; its location is not Work identity. |
| Installation | Host-local journal | Active WorkRevision / AssemblyLock, acquisition, state bindings, secret policy, and status. |
| Run | Host Service Run journal | One execution occurrence; `host.run.*` owns independent status, node instances, health, and terminal records. |

A Package remains a replaceable capability and component distribution unit. A Package manifest can normalize into a single-node Assembly, but its Package ID does not automatically become the Work ID, and source visibility grants no execution authority.

## Creation path

```text
work.yaml / assembly.yaml / Package source / Foreign source
  ↓ plurora work check / pack
WorkRevision + AssemblyRevision + AssemblyLock + closure
  ↓ plurora installation create --idempotency-key ...
Installation journal event
  ↓ projection rebuild
installation.json
```

`work pack` writes only the content-addressed ObjectStore. It does not install, run, or mutate a Host profile. Installation create revalidates that the WorkRevision and AssemblyLock artifacts exist and match their digests before appending the journal record.

## Lifecycle

```text
resolving → ready
ready → updating → ready
resolving/updating → blocked | failed
ready/blocked/failed → removing → removed
```

Every mutation increments a monotonic `revision`. Update callers must submit `expected_revision`; a mismatch returns a conflict and cannot overwrite newer state.

Create, update, and remove require an `idempotency_key`. The same key with the same request fingerprint returns the committed result; the same key with a different request fails closed.

## Update and state

Update validates the next WorkRevision / AssemblyLock, snapshots affected state, and requires an explicit state action:

- `preserve`: retain bindings when state contracts remain compatible;
- `replace`: the CLI canonicalizes a public typed snapshot JSON file, uploads it through `object.put`, and sends only its fixed-type descriptor on the update wire;
- `reset`: explicitly abandon incompatible state.

The Host rebuilds the current/candidate diff from verified CAS WorkRevision, root AssemblyRevision, and AssemblyLock objects; request-carried summary metadata is not authoritative. The diff keeps the compatibility top-level changed flags and also reports stable-ID-sorted `added` / `removed` / `changed` before/after values for Work entrypoints, content roots, rights, transparency, and operational intent; Assembly nodes (including inline Ports/config), authoring bindings, exposed Ports, and State Slots; and Lock nodes, bindings, protocol profiles, and content roots.

Each State Slot compares owner, schema digest, scope, portability, backup policy, and migration Port. A compatible slot with existing durable state may be preserved. An incompatible slot permits `replace` only when the candidate Assembly has a validated migration Port; otherwise the caller must separately choose destructive `reset`. Removing a durable slot likewise requires `reset`. Run-scoped slots do not require durable migration, and new slots are allowed. An empty state tree still produces the diff but does not force a meaningless migration or reset.

The caller does not supply an approval, replacement decision receipt, or authority evidence. After explicit `reset` / `replace` intent arrives with current exact `installation.manage` authority, the Host revalidates the grant and expiry before the state effect, then persists its own authority evidence and allow decision receipt. `replace` uses `urn:plurora:installation-state-replacement-decision-receipt:v1` with payload schema `plurora.installation-state-replacement-decision-receipt.v1`; it proves only that the Host decided and applied the submitted snapshot replacement, not that a migration component ran. The update result returns those content-addressed descriptors in `receipts`; an idempotent replay returns the same descriptors after restart.

The current MVP `replace` effect is precise: the Host strictly validates and atomically applies the submitted canonical typed snapshot, then persists decision/evidence for that snapshot replacement. A declared migration Port is the executable contract that permits `replace`, but this phase does not invoke the migration component and does not fabricate an effect receipt claiming that it ran; actual migration component output must first be expressed as the typed snapshot. Public `object.get` access to state decision/evidence requires the complete descriptor to match one issued by the authoritative journal for the current Installation, in addition to structural and CAS identity validation. Putting a structurally valid object through `object.put` does not turn it into a Host receipt.

A `replace` file uses the public `InstallationStateSnapshot` shape:

```json
{
  "schema": "plurora.installation-state-snapshot.v1",
  "entries": [
    { "path": "save/profile.bin", "bytes": [1, 2, 3, 255] }
  ]
}
```

`entries` are strictly sorted by unique `path`. Paths contain only non-empty relative segments separated by `/`; absolute paths, platform prefixes, `.`, `..`, backslashes, and NUL are rejected. The CLI puts neither its local file path nor raw state bytes in the update DTO or errors. It works against a remote Host with a different data directory because the snapshot travels through public `object.put` first.

The active Work/Lock pointer changes only after validation, state preparation, and journal compare-and-set succeed. Failure preserves the prior pointer and bounded rollback evidence. Unknown external effects are never guessed to have succeeded.

## Removal

Removal requires an explicit choice:

- `keep`: terminate Installation authority while retaining Host-owned state;
- `delete`: delete only Installation state that passes containment and symlink/reparse checks.

For device authority, the Host passes refresh capability after the initial exact `installation.manage` authorization only through a sidecar that cannot appear on the wire. After remove waits for the Installation apply lock and synchronizes the journal, it rechecks the exact Installation subject, grant, expiry, and delegation around snapshot, delete/replace effect, and terminal commit. `keep` also refreshes before its durable terminal commit. Refresh follows owner-lease validation and sits immediately before the journal append or filesystem effect. Expiry or revocation while waiting fails closed without deleting state or writing a false successful terminal. A replay with a persisted claim returns the authoritative terminal result directly and performs no projection repair or filesystem effect. A new-key no-op for an already Removed Installation persists its claim only after another boundary refresh.

A linked-local Workspace points to user-owned source, so neither choice deletes or rewrites it. A managed Workspace can be handled only after its canonical path is proven to remain under the Host-owned Workspace root.

## Data layout

```text
~/.plurora/
├── objects/                         # content-addressed artifacts
├── installations/<installation-id>/
│   ├── installation.json            # rebuildable journal projection, not authority
│   ├── assembly.lock.json            # local projection of the active closure
│   ├── secrets.dat                   # age-encrypted Installation secret store
│   ├── state/                        # Host-owned state
│   └── diagnostics/
├── workspaces/<workspace-id>/
│   ├── workspace.json
│   └── source/
└── runtime/
    └── installations.sqlite3         # default durable journal backend
```

Startup rebuilds Installation projections from the EventStore journal. A missing or corrupt `installation.json` cannot change authority and does not trigger compatibility reads from retired directories or descriptors.

## Secrets and authority

Installation-local secrets use:

```text
secret_ref:installation:OPENAI_API_KEY
```

Scope comes from a Host-verified Installation context, never an arbitrary client path. `secret_policy.allow_platform_fallback` controls whether a missing local value can fall back to `secret_ref:store:*`. Raw secrets never enter Work, Assembly, journal events, logs, or diagnostics.

Device grants combine explicit action scopes with resource selectors. Installation mutation requires `installation.manage`; list and get require `observe`. A wildcard must be written explicitly on the wire as `id: null`; omitting `id` is not a wildcard.

## CLI and public protocol

```bash
plurora installation list
plurora installation info <installation-id>
plurora installation create <work-source> --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action preserve --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action reset --idempotency-key <key>
plurora installation update <installation-id> <work-source> --expected-revision <n> \
  --state-action replace --replacement-snapshot <snapshot.json> \
  --idempotency-key <key>
plurora installation remove <installation-id> --state keep \
  --idempotency-key <key>
```

Exact wire IDs:

```text
host.installation.list
host.installation.get
host.installation.create
host.installation.update
host.installation.remove
```

Lifecycle events:

```text
host/installation.created
host/installation.updated
host/installation.removed
```

There are no compatibility aliases. Web Home and third-party clients use the same public methods.

## Boundary with Run

Installation `ready` means that the local adoption record and artifact closure are valid. It does not mean a process is running, a port is allocated, or an endpoint is exposed. Phase 4 `host.run.*` methods use a separate Run journal with starting → running → degraded → stopping → stopped (or failed / interrupted) states; opening Library or Installation detail never creates a Run. Run start activates only an installed, verified, uniquely matching local implementation from the AssemblyLock. Missing, ambiguous, unsupported-backend, or machine-resource requirements return structured gaps and next steps rather than implicit build/deploy. A Run context is independent from a browser tab, so closing a tab does not stop it; stop releases only that Run activation and does not unload global Packages. Phase 5 Exposure/Binding uses the Host journal, exact provider/consumer Ports, and an optional Run pin; close, revoke, expiry, and version drift never rewrite the Installation lock. Managed Realization plan/apply remains Phase 6. See [`RUN_LIBRARY.en.md`](RUN_LIBRARY.en.md) and [`POWERBOX_BINDING.en.md`](POWERBOX_BINDING.en.md).
