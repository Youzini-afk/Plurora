# Realization: compiling portable Work onto a Target

> [English](./REALIZATION.en.md) · [中文](./REALIZATION.md)

A Realization is a Host-owned mutable execution record. `WorkRevision`, `AssemblyLock`, and `OperationalIntent` describe what should run; `TargetInventorySnapshot` describes what one Target can currently do; the pure planner compiles them into a content-addressed `RealizationPlan`. A plan grants no execution authority and causes no Target effect.

The current implementation provides `host.realization.plan/apply/get/list/stop/rollback/reconcile`, seven public lifecycle events, CLI commands, and the Installation-frame Realization workbench. Docker/OCI is the first backend, not the model's only long-term execution form.

## Data and ownership

- `OperationalIntent` is the portable Work reference for workloads, allowed execution classes, resources, endpoints, state, placement, and update policy.
- `TargetInventorySnapshot` is the Host-observed Target capability/capacity/trust/topology snapshot; it enters the plan digest as an artifact.
- `RealizationPlan` is the pure compiled artifact pinning Work, AssemblyLock, OperationalIntent, inventory, actions, preconditions, required authority, and risk summary.
- `RealizationRevision` is a Host-journal projection containing status, actual resources, receipts, health, parent, and timestamps. It is not a portable artifact.
- Approvals and effect receipts enter the ObjectStore; public events carry only typed, redactable Realization views.

Runtime does not put Realization into the Constitutional Substrate. The Host control journal is authoritative for mutable facts, while the Runtime public journal receives only the seven public relay events. `host-private/realization.*` stopping/effect checkpoints never enter the public event stream.

## Lifecycle

```text
Planned → Applying → Active / Failed / OutcomeUnknown / RecoveryRequired
Active / Degraded → Stopping → Stopped
historic Planned/Active → Applying replacement → RolledBack(active replacement)
```

`plan` reads and verifies the current Installation, Work/Lock, OperationalIntent, and Target inventory, generates stable plan bytes/digest, then persists a Planned revision. It may write immutable artifacts and Host-journal facts but never calls build, launch, route, or stop effects.

`apply` must match the persisted plan reference exactly and revalidates:

1. Installation revision, Work digest, AssemblyLock digest, and OperationalIntent;
2. Target lease/policy epoch and inventory precondition;
3. approval for the exact plan digest, its expiry, and every `risk_summary` item;
4. the current `realization.apply` grant/delegation/lease and exact Installation, Target, and Realization selectors;
5. current authority before every build, route, launch, and terminal-journal boundary.

The Host then executes backend actions through typed Target operations. Local and remote-Agent Targets share one plan, operation, receipt, and Realization state machine; first-party code has no bypass.

## Durable effect checkpoints

After an external effect succeeds and before public Active/Stopped/RolledBack commits, the Host writes a private effect checkpoint to the control journal. It stores actual resources and structured receipt facts. If receipt artifacts are temporarily unavailable, restart or same-key retry can continue materialization without applying the completed external effect again.

- The same apply key hitting Applying plus an apply checkpoint only completes the public terminal.
- Stopping plus a stop checkpoint commits Stopped after restart without repeating stop.
- If a rollback replacement applied but stopping the old revision temporarily failed, checkpoints retain both replacement and parent obligations; same-key retry resumes at parent stop without reapplying the replacement.
- Incomplete Applying without an effect checkpoint is never implicitly replayed; restart moves it to `recovery_required` for explicit reconcile/stop/rollback.
- If a build completed but launch/route did not complete, partial facts are retained: known receipts and any workload identity that may still exist enter a `recovery_required` or `outcome_unknown` revision for explicit reconciliation. This is never downgraded to an ordinary no-effect failure.
- Private events are not among the 76 platform events and are unavailable through public journal/SSE.

This cannot turn an external system and the Host journal into one atomic transaction. If an external effect occurred while the control journal itself also became unwritable, the Host can only fail closed and require reconciliation. Target operations use stable idempotency keys, and recovery never guesses inputs from a live workspace.

## Stop, rollback, and reconcile

`stop` acts only on resources recorded in `actual_resources`. It durably records stopping intent, calls typed Target stop, stores a receipt checkpoint, and finally commits Stopped. Closing a browser tab, leaving the Installation frame, or reading status never stops anything.

`rollback` reads only a persisted historic `RealizationPlan` and a new exact approval. It never reads a live workspace, repackages a source directory, or guesses parameters from another mutable projection. The Host generates a replacement RealizationId, checkpoints the replacement effect, stops the parent, then commits `host/realization.rolled_back`.

`reconcile` is explicit effect-free Target observation. It updates observed resources, health, and receipts while distinguishing stable reasons such as `outcome_unknown`, `recovery_required`, and `target_unsatisfied`; it never implicitly applies again.

## Backends and Targets

The current implementation supports:

- exactly one workload per plan in the first executor, while the portable model retains its multi-workload/action shape. A multi-workload request returns `unsupported_backend` before any Target effect until a later executor supplies atomic per-action checkpoints and compensation;
- prebuilt immutable OCI images, required to use `name@sha256:<digest>`;
- Dockerfile build through a declarative verifier, producing a content-addressed image before using the same launch path;
- typed build/apply/observe/stop operations for local and remote Target-Agent Targets;
- Host-owned loopback port leases, routes, and actual-resource receipts.

The planner accepts only execution classes allowed by OperationalIntent and explicitly advertised by TargetInventory. Unknown/offline Targets, missing capability, stale authority epochs, non-content-addressed images, and stale Work/Lock preconditions return structured gaps or errors. There is no publisher priority or automatic fallback Target.

## Public methods and authority

| Method | Action | Exact resources |
|---|---|---|
| `host.realization.plan` | `realization.plan` | Installation + Target |
| `host.realization.get/list` | `observe` | visible Installation + Realization (list may filter by Target) |
| `host.realization.apply/stop/reconcile` | `realization.apply` | Installation + Target + Realization |
| `host.realization.rollback` | `realization.apply` | Installation + Target + current Realization + historic Realization |

Omitting a resource ID never creates a wildcard. Root credentials remain Host-maintenance authority; plans, UI, Agent output, and approval artifacts are not execution authority. Raw Target/exec/port/proxy effects remain HostAdmin/HostDev adapters and cannot substitute for `host.realization.apply`.

## CLI and Web

The CLI provides:

```text
plurora realization list
plurora realization info
plurora realization plan
plurora realization apply
plurora realization stop
plurora realization rollback
plurora realization reconcile
```

Every effect command requires explicit Installation, Target, Realization revision, and idempotency key. Apply/rollback require `--approve` for the exact plan plus acceptance of every risk. Output contains no credential, absolute path, raw stderr, or secret.

The Installation frame shows the Realization workbench only when Work declares OperationalIntent. Users select Target/backend, plan first, inspect actions/preconditions/authority/risks, then acknowledge every risk before apply. The UI does not reconstruct plan bodies it does not hold; an older Planned revision requires explicit Target reselection or CLI use. Closing the frame does not stop a Realization.

## Boundaries with Run, Binding, and other capability

- `host.run.start` still never implicitly builds or applies a Realization; missing managed Realization remains a structured gap.
- Exposure/Binding grants only the selected Port's minimal runtime handle; it does not create or migrate managed resources.
- Realization does not share cross-Installation state; StateSlot/backup/migration remains under explicit owner and policy.
- Foreign Work, Rights/Transparency, closed-source entries, and opaque-state backup use the same public authority/effect boundary.
- ChangeSets and Plans generated by the development loop or companion agent still cannot bypass this guide's authority/effect boundary.

Machine-readable contracts are in [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.en.md), [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.en.md), and `docs/spec/v1/schemas/`.
