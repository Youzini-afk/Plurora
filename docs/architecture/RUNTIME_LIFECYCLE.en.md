# Runtime Lifecycle

> [English](./RUNTIME_LIFECYCLE.en.md) · [中文](./RUNTIME_LIFECYCLE.md)

This document records the Package, Session, Proposal, and Capability Invocation lifecycles coordinated by the current Contract V1 runtime. They are operative compatibility contracts, not a permanent declaration that the constitutional substrate knows or must know exactly these nouns.

These lifecycles do not describe turns, chats, prompts, or other domain operations. Domain meaning belongs to adopted Protocols, Components, or Products; Contract V1 commonly carries it through Package writers and event namespaces.

## Package lifecycle

```text
discovered  manifest visible to the host
loading     manifest validated, sandbox prepared, ABI checked
starting    entry point booted, kernel handshake, capabilities and hooks registered
ready       accepting calls and dispatches
degraded    reachable but reporting reduced ability (heartbeat slow, partial features)
stopping    graceful shutdown signal sent
stopped     resources released
unloaded    no longer active in the host
```

Each transition emits a `kernel/v1/package.*` event. Subscribers react through the public protocol, including observability tools and other packages. The kernel exposes no private hook for package state.

## Session lifecycle

A session is a labeled event stream with an attached package set and a permission scope. The kernel does not assign any other meaning to it.

```text
requested   open() received, principal and labels supplied
opening     context/before_open dispatched (sync, vetoable)
open        context/opened emitted
            event log accepting appends from authorized writers
            capability invocations dispatching against the active package set
forking     fork() received with parent session and forked-from sequence
forked      context/forked emitted; child session inherits parent up to the chosen sequence
closing     context/before_close dispatched (sync, vetoable)
closed      context/closed emitted; log frozen for further appends
```

The runtime does not own a "current turn," "active actor," or any content-level state of a Session. Protocols or Components that need those concepts derive them from events, objects, and their own projections.

## Proposal lifecycle

The kernel mediates generic approval-gated change proposals. The lifecycle is content-free. It only knows the operations it can apply, such as `asset.put` and `projection.rebuild`.

```text
created     proposal recorded under requesting principal; change/proposal.created emitted
approved    approver decision recorded; change/proposal.approved emitted
rejected    approver decision recorded; change/proposal.rejected emitted
applied     approved proposal executed against the kernel; change/proposal.applied emitted
failed     application or validation failed; change/proposal.failed emitted
```

A package or assistant principal cannot apply a proposal directly. It must reach `approved` first. The kernel does not invent domain-specific proposal semantics; richer operations such as multi-step transactions and package-side compensation belong to packages built on top.

## Capability invocation lifecycle

```text
requested        invoke(id, version, input) received
authorizing      capability/before_invoke dispatched (sync, vetoable)
routed           provider selected by id+version+session package set
running          provider executing; streaming chunks may flow
completed        capability/completed emitted with output (or stream end)
failed           capability/failed emitted with structured error
cancelled        cancellation acknowledged by provider; failed/completed event records the outcome
```

The kernel records invocations as kernel events. The contents of `input` and `output` are opaque to the platform. They are validated only against the provider's declared schemas.

## Cancellation and timeouts

Every long-running operation has a deadline, including capability invocation, hook dispatch, and package start. The deadline is derived from manifest sandbox policy plus host policy. Exceeding it triggers cancellation, and the kernel records the outcome.

The kernel does not invent its own cancellation semantics for content. There is no "regenerate" or "stop generating" in the platform. Such operations are package capabilities.

## Replay and bootstrap

When a host restarts:

1. Manifests are rediscovered.
2. Packages move through `loading` and `starting`.
3. Stored sessions are accessible for read-only replay immediately.
4. A session resumes write operations only after its required packages reach `ready`.

Packages that need to rebuild internal state from the event log do so via `events.read` and the replay stream. The kernel offers no other recovery mechanism.

## Errors

The kernel classifies errors only at its own boundary: transport, manifest, schema, permission, capacity, lifecycle, ambiguous-route. Package errors flow through capability invocations as opaque structured failures and are recorded under `capability/failed`.

## What this lifecycle does not describe

- No turn, no message, no prompt cycle.
- No model call orchestration.
- No memory update flow.
- No agent task.
- No world tick.

All of the above belong inside packages. None of them are kernel lifecycles.
