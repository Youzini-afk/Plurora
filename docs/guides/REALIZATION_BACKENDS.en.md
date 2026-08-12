# Runtime adapters for the Realization executor

> [English](./REALIZATION_BACKENDS.en.md) · [中文](./REALIZATION_BACKENDS.md)

This page documents the low-level Target, exec, port, and proxy adapters used by `ServiceRealizationExecutor`. The public managed lifecycle is defined by [`REALIZATION.md`](REALIZATION.en.md) and `host.realization.*`; these adapters are not a second managed lifecycle, revision, or authority model.

## Boundary

- Work expresses portable workload requirements through `OperationalIntent`.
- The pure planner compiles a content-addressed `RealizationPlan` from `TargetInventorySnapshot`.
- `host.realization.apply` invokes the executor only after exact approval, precondition, and authority checks.
- The executor translates plan actions into typed `TargetOperationSpec` values and records `RealizedResource` plus `RealizationEffectReceipt` results.
- `host.exec.*`, `host.port.*`, and `host.proxy.*` are low-level HostAdmin/HostDev adapters. Ordinary devices cannot use these methods to bypass Realization.

Web and CLI use only `host.realization.*`; the low-level adapters expose no parallel managed API to ordinary devices.

## One path for local and Target Agent execution

Local Host and remote Target Agent receive the same typed operations:

- declarative verifier / Dockerfile build;
- managed workload apply;
- workload observe;
- workload stop.

A Target operation carries Installation, Target authority epoch, request digest, idempotency key, status, and terminal receipt. Local driver versus Agent transport changes execution location, not Plan or receipt semantics.

## Docker backend

The first backend currently accepts:

1. an immutable OCI image containing an `@sha256:` digest;
2. a Docker build using a persisted build-context artifact, relative Dockerfile, network policy, workspace/source digest, and build-descriptor hash.

Build must return an immutable image before entering the same launch path. Docker is not required by Work or Assembly; later backends extend the model through new execution classes and typed actions.

## Ports and routes

The executor creates a Host-owned loopback port lease for each endpoint and registers an HTTP route bound to that lease. `RealizedResource.properties` records the Target workload reference, route, port lease, observed host port, public URL, and immutable image. Stop cleans only those recorded resources; it never scans a live workspace or arbitrary containers.

Route access defaults to Host-authenticated. Public exposure requires an explicit public policy in the Realization backend selection. Route and workload IDs pass existing token validation and cannot be absolute paths, URLs, or shell fragments.

## Health and restart

Host startup order is:

1. hydrate Runtime backend event projections;
2. reconcile Target workload/runtime-adapter truth;
3. hydrate Installation, Powerbox, Realization, and Run authority journals;
4. let Realization perform effect-free observation or consume durable effect checkpoints for Applying/Stopping records.

Public readiness reports durable, active, and degraded Realization counts and treats only the Realization projection as managed truth.

An uncertain target result becomes `outcome_unknown`; resources/receipts that cannot be established safely become `recovery_required`. The Host never infers success from a bound port, process presence, or route name.

## Receipts and logs

A Target terminal receipt records operation/action, Target, request digest, timestamps, and structured status. Realization references the receipt as a content-addressed artifact. Public errors expose stable codes and next steps, never credentials, raw stderr, secrets, Docker build output, or host absolute paths.

Low-level exec/proxy/port lifecycle events remain adapter diagnostics. User-visible Realization status is expressed only through the seven `host/realization.*` lifecycle events. Private stopping/effect checkpoints never enter the public journal.

## Non-goals

- no arbitrary shell or arbitrary network proxy;
- no root Target authority for Surfaces, Packages, or Agents;
- no implicit build/apply from Run start;
- no rollback from a live workspace;
- no Docker, route, or Target-operation ontology in the Constitutional Substrate.

See [`REALIZATION.md`](REALIZATION.en.md), [`../architecture/HOST_RESOURCE_AUTHORITY.md`](../architecture/HOST_RESOURCE_AUTHORITY.en.md), and [`../architecture/TARGET_AGENT_PROTOCOL.md`](../architecture/TARGET_AGENT_PROTOCOL.en.md) for authority and recovery details.
