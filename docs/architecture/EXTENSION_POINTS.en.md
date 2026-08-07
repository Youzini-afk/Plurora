# Extension Points

> [English](./EXTENSION_POINTS.en.md) · [中文](./EXTENSION_POINTS.md)

An extension point is a named hook in the current Contract V1 runtime. The core runtime or a Package writer may declare it, and ordinary Components may subscribe. The runtime performs authorized routing without interpreting domain meaning.

This document records the current compatibility contract. Long-term shared extension meaning belongs to an explicit Protocol owner; a Package distributes implementations and declarations rather than permanently owning all extension ontology.

## Hook contract

Every extension point has:

- `id`: namespaced, immutable.
- `payload_schema`: the JSON shape of the call.
- `timing`: `sync` or `async`. Synchronous handlers block the operation. Asynchronous handlers do not.
- `modifiable`: whether subscribers may return a changed payload that the next subscriber sees.
- `short_circuit`: whether a subscriber may veto the operation.
- `ordering`: how the dispatcher orders subscribers. Declared precedence is used first; ties use a stable order.

The core owner publishes schemas for core extension points. A non-core Protocol or Component owner publishes its schema and distributes it through the current Package Manifest.

## Subscription

A subscriber is declared in a manifest:

```yaml
contributes:
  hooks:
    - extension_point: kernel/v1/event.before_append
      handler: my_handler
      timing: sync
      precedence: 100
```

The kernel verifies that the subscriber's manifest declares the permissions implied by the hook. For example, `event.before_append` requires event read; modifying the payload requires event append.

A subscriber that returns an error stops the operation only when `short_circuit: true`. Otherwise the error is logged and dispatch continues.

## Cancellation and timeout

Synchronous handlers run within the operation's deadline. Asynchronous handlers receive a deadline derived from the package sandbox policy. Exceeding the deadline cancels the handler and counts as a failed call.

## Implementation status

The current `kernel/v1/*` extension-point set remains compatibility-stable. The implementation covers event append and capability invocation: stable ordering, Component handlers, payload metadata mutation, veto, and unload cleanup. Session and Package lifecycle hooks are reserved in the contract. Today they are delivered through `kernel/v1/session.*` and `kernel/v1/package.*` events; synchronous and asynchronous handling may be completed later. New shared extension meaning belongs in a Protocol namespace with an explicit owner; ordinary Component Packages may provide implementations without continuing to grow monolithic `kernel.v1`.

## Kernel-emitted points

The current runtime emits only this small compatibility set. New non-core extension points are defined by explicit Protocol or Component owners and distributed through ordinary Packages.

### Session lifecycle

- `kernel/v1/session.before_open` — sync, modifiable false, short_circuit true.
  Permission to open is enforced here. Subscribers may veto.
- `kernel/v1/session.after_open` — async.
- `kernel/v1/session.before_close` — sync, modifiable false, short_circuit true.
- `kernel/v1/session.after_close` — async.

Payload: session id, requested labels, package set, requesting principal.

### Event log

- `kernel/v1/event.before_append` — sync, modifiable true, short_circuit true.
  Permission and schema enforcement happen here. Subscribers may amend metadata or veto.
- `kernel/v1/event.after_append` — async.
  Subscribers receive the persisted envelope.

Payload: event envelope. The kernel does not interpret the payload field. It only checks declared schemas when the writer's manifest references a payload schema for that event kind.

### Capability invocation

- `kernel/v1/capability.before_invoke` — sync, modifiable true, short_circuit true.
  Permission, route resolution, and quota enforcement happen here.
- `kernel/v1/capability.after_invoke` — async.
  Subscribers receive input, output (or error), latency, and provider id.
- `kernel/v1/capability.error` — async.
  Subscribers receive the structured failure.

Payload: invocation envelope.

### Package lifecycle

- `kernel/v1/package.loaded` — async.
- `kernel/v1/package.unloaded` — async.
- `kernel/v1/package.degraded` — async.
- `kernel/v1/package.heartbeat_lost` — async.

### Hook registry

- `kernel/v1/hook.registered` — async.
- `kernel/v1/hook.unregistered` — async.

These let observability packages discover the live extension topology.

## Package-emitted points

A package may publish its own extension points by listing them under `contributes.extension_points`. The package becomes the owner of the schema.

The kernel routes calls but does not validate semantics. If the owning package is unloaded, the kernel refuses to dispatch the point and emits `kernel/v1/hook.unregistered` for any orphaned subscribers.

Example (illustrative; not part of the kernel):

```yaml
contributes:
  extension_points:
    - id: someorg/conversation/before_step
      payload_schema: ...
      timing: sync
      modifiable: true
      short_circuit: true
```

A different package can subscribe:

```yaml
contributes:
  hooks:
    - extension_point: someorg/conversation/before_step
      handler: ...
```

The kernel does not know what `conversation/before_step` means. The owning package does.

## Discovery

A client may query the kernel for live extension points and their subscribers. Schemas are exposed. Creator tools, observability dashboards, and other packages use this to see what is currently extensible in a running host.

## Versioning

Each extension point has a `version`. Subscribers declare the version they target. The kernel refuses to dispatch to a subscriber whose declared version is incompatible with the live point.

Breaking changes to a point require a new id. The owning package may emit both versions during transition.

## Stability

The kernel-emitted point set is small by design. Adding a kernel point needs the same justification as adding a kernel responsibility: it truly cannot live in a package.
