# Run and Library Guide

> [English](./RUN_LIBRARY.en.md) · [中文](./RUN_LIBRARY.md)

This guide describes the Phase 4 Run lifecycle and the official Library entry point. A Run is one durable execution fact on a Host. An Installation is an adoption record, while Work and Assembly are portable definitions. Opening a Library item or Installation detail never implicitly creates a Run, builds source, or deploys machine resources.

## Objects and ownership

| Object | Owner | Meaning |
|---|---|---|
| WorkRevision / AssemblyRevision / AssemblyLock | Immutable artifacts in ObjectStore | Define the work, composition, and exact provider/binding resolution. |
| Installation | Host Service journal | Owns the active Work/Lock, user choices, state bindings, and secret policy. |
| InstallationWorkSummary | Host-verified projection of the exact WorkRevision | Exposes WorkId, title, complete entrypoints, and Rights / Transparency / OperationalIntent references to Library; it is not a second authority. |
| Run | Host Service Run journal | Owns one start, activation context, node instances, health, and terminal status. |
| Library | Official Shell / client | Derives affordances from Work, Installation, Run, Rights, and current authority; it does not replace Host authorization. |

The MVP default policy permits at most one active Run per Installation. The Host generates RunId after preflight succeeds; clients cannot choose it in advance. The Run journal is durable authority. In-memory activation handles are only runtime projections, so a Host restart never guesses that an execution succeeded.

## Lifecycle

```text
starting → running ↔ degraded → stopping → stopped
    └────→ failed / interrupted
```

Powerbox manages dynamic Exposure and Binding state in the durable journal:

```text
Exposure: active → closing → revoked / expired
Binding:  selected / active → closing → terminal
```

Close precedes the terminal commit. Host restart, owner takeover, retry, and `outcome_unknown` never rewrite terminal history; recovery creates a new record.

The `starting` event commits before activation. A successful activation commits `running`. An explicit stop passes through `stopping` and then commits `stopped`. Activation failure records `failed`. When the Host definitively observes permanent loss of a JSON-RPC stdio Package transport protected by Run leases, every affected active Run durably converges to `failed` with health reason `package_activation_lost`; duplicate or late notifications from an old process cannot create another terminal event. On Host restart, any still-active Run is appended as `interrupted` with health reason `host_restart`; the Host neither replays effects nor assumes that the process remains available. Terminal history is not rewritten; recovery creates a new Run.

A Run `context_id`, when present, belongs only to that Run execution context. Closing a browser tab, Surface iframe, or PWA connection does not stop the Run. A caller with authority must invoke `host.run.stop` explicitly. Stop releases only that Run's activation/context; it does not globally unload a Package or affect Runs owned by another Installation.

## Public methods and authority

| Method | Required scope | Key input/behavior |
|---|---|---|
| `host.run.list` | `observe` | Optional `installation_id` and `status` filters; returns only visible Runs. |
| `host.run.get` | `observe` | Requires exact `installation_id` and `run_id`; verifies ownership before returning the Run view. |
| `host.run.status` | `observe` + exact Installation | Without `entrypoint_id`, returns the current Installation revision and active-Run overview. With an entrypoint, performs zero-effect preflight and returns gaps. The result is neither a reservation nor authority; start revalidates it. |
| `host.run.start` | `run` + exact Installation | Takes `expected_installation_revision`, entrypoint, and `idempotency_key`. The Host revalidates Installation and authority before generating RunId. |
| `host.run.stop` | `run` + exact Installation + requested Run child | Carries exact I/R, `expected_revision`, and `idempotency_key`. Because the Host creates RunId, the public rule derives only to a Run child whose relationship the registry proves; wrong ownership still fails closed. |

The same idempotency key and fingerprint replay the durable result; a different fingerprint returns `idempotency_conflict`. The journal stores only the key's SHA-256, never the caller's raw key. A second start while an Installation has an active Run returns `active_run_exists`.

## Preflight and structured gaps

`host.run.status` and `host.run.start` share the same entrypoint checks. Status is read-only; start repeats the check before an effect. Start activates only a locally installed and verified Package Component pinned by the current Installation's AssemblyLock. Artifact digest, behavior digest, trust class, and entry kind must match exactly, and exactly one ready Package may match. Publisher identity is never a tie-breaker.

In this Phase, Run-bound activation creates a Run context, node-instance records, and exact Installation and Package lifecycle leases over those already-ready local implementations. It does not load, build, or deploy a Package, and it does not start a separate Package process for each Run. The Runtime continues to own shared Package processes and capabilities globally. A Package cannot be unloaded or restarted while any Run lease remains; Run stop releases only that Run's context and leases.

When activation is unsafe, the result keeps `run: null` and returns structured `gaps[]`. Each gap has a stable `reason_code`, optional `node_id` / `port_id`, and an actionable `next_step`:

- `artifact_missing`: the locked Component is not installed/ready, or its artifact, behavior, or trust does not match; install and verify the exact Package.
- `binding_ambiguous`: multiple equally matching local Packages exist; remove the duplicate or make the locked selection explicit instead of choosing by publisher.
- `binding_unavailable`: a required Launch/Runtime import has no fixed usable binding. This Phase stops at an explicit gap; Phase 5 supplies Exposure/Binding selection rather than mutating the Lock as a bypass.
- `unsupported_backend`: WASM, remote, `contract: none`/Foreign Capsule, and unsupported subprocess forms have no Run driver in this Phase; use a supported local implementation.
- `target_unsatisfied`: an entrypoint/Port cannot be satisfied, or the Work carries an OperationalIntent requiring machine resources; follow the gap's next step to prepare the required Realization.

Gaps are diagnostics and next steps, not implicit authority. Run start does not build source, create a public route, execute managed deployment, or call the future Phase 6 `host.realization.apply`. A missing managed Realization remains a gap.

## Powerbox and Library

The official Library reads visible Work, Installation, Run, Rights, and current Host authority to derive affordances such as `Open`, `Install`, `Play/Run`, `Stop`, `Inspect`, `Update`, and `Remove`. Affordance `reason_code`, risk, and `next_step` explain the state to the UI; the actual request is still governed by Host action scopes and exact selectors.

- `Open` / detail reads Work and Installation projections only; it does not create a Run.
- `Play` / `Run` invokes `host.run.start` and displays starting, running, degraded, failed, or structured gaps.
- `Stop` invokes `host.run.stop`; it does not unload shared Packages, delete the Installation, or delete user state.
- An `interrupted` Run after restart is shown as requiring a new explicit start, never as a fabricated successful recovery.
- If the stop effect happened but its terminal journal commit cannot be confirmed, replay converges to `interrupted` + `outcome_unknown` rather than guessing that Stopping succeeded.
- The Powerbox chooser uses `host.exposure.*` / `host.binding.*` to disclose the explicit phase, exact Exposure/audience/expiry, both PortContracts, provider source/trust/claims/boundaries/evidence, and candidate digest/stale state. Zero or multiple candidates require an explicit choice; preferences are ordering hints only.
- Runtime injects the selected Port's least-authority handle. Multiple Ports on one Component may share activation, while a different Component or node path is isolated. Provider stop, revoke, expiry, or version drift cancels the Binding; state and secrets never cross Installations.
- Exposure and cross-Installation Binding are implemented in Phase 5. Managed Realization plan/apply and `host.realization.*` remain Phase 6 planned and are not claimed as complete here.

## Related contracts

- [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.en.md) — 92 methods, 69 events, and authority rules.
- [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.en.md) — Run, Exposure, and Binding lifecycle events.
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.en.md) — Installation journal, state, and Work boundaries.
- [`../architecture/HOST_RESOURCE_AUTHORITY.md`](../architecture/HOST_RESOURCE_AUTHORITY.en.md) — exact selectors and the `run` action.
- [`POWERBOX_BINDING.md`](POWERBOX_BINDING.en.md) — candidate disclosure, runtime pins, revoke, and expiry rules.
