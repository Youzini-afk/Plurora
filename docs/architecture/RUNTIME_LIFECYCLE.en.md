# Runtime Lifecycle

> [English](./RUNTIME_LIFECYCLE.en.md) · [中文](./RUNTIME_LIFECYCLE.md)

This document records the Package, Context, Proposal, and Capability Invocation lifecycles coordinated by the current public-contract runtime. These are operative contracts, not a claim that every Plurora product must organize its domain around the same nouns.

## Package lifecycle

```text
discovered  Manifest visible to the Host
loading     Manifest validated and execution boundary prepared
starting    entry starting and declarations registered
ready       accepting supported calls
loaded      Package registration committed
degraded    reachable but reduced or failed
stopping    shutdown initiated
stopped     execution resources released
unloaded    Package removed from the live registry
```

Transitions emit platform-owned `host/package.*` events with writer `plurora/runtime`. Subscribers observe them through the public journal; there is no private first-party lifecycle channel.

## Context lifecycle

A Context is a labeled event stream with an active Package set and authority scope. The runtime does not assign content meaning to it.

```text
requested   context.open received
open        context/opened persisted
forking     context.fork received with parent and sequence
forked      child lineage recorded and context/forked persisted
closing     context.close received
closed      context/closed persisted; further appends rejected
```

The runtime owns identity, ordering, lineage, and authority boundaries. Protocols and Components derive domain state from journal events, objects, and projections.

## Proposal lifecycle

The `change.proposal.*` facade mediates approval-gated generic changes:

```text
created     proposal recorded; change/proposal.created
approved    review decision recorded; change/proposal.approved
rejected    review decision recorded; change/proposal.rejected
applied     approved operations committed; change/proposal.applied
failed      validation or application failed; change/proposal.failed
```

Apply rechecks authority and terminal state. Proposal payload meaning remains outside the constitutional substrate.

## Capability invocation lifecycle

```text
requested        capability.invoke or capability.stream received
authorizing      caller handle, scope, permissions, and input checked
intercepting     capability/before_invoke dispatched
routed           provider selected explicitly or unambiguously
running          provider executes; stream frames may be emitted
completed        capability/completed or stream-ended terminal recorded
failed           capability/failed or stream-error terminal recorded
cancelled        cancellation terminal recorded
timed out        timeout terminal recorded
```

Invocation input and output are provider-owned JSON validated against declared schemas. The runtime records authority and execution evidence without interpreting domain content.

## Deadlines and cancellation

Long-running execution is bounded by Manifest sandbox policy and Host policy. Cancellation and timeout prevent further stream chunks and produce distinct terminal state. Domain-specific actions such as “regenerate” remain Package capabilities rather than platform lifecycle primitives.

## Restart and replay

On Host restart:

1. durable stores and profiles are opened;
2. Manifests are rediscovered and autoloaded Packages re-enter their lifecycle;
3. journal data is immediately readable according to authority;
4. Components rebuild their own projections through public replay;
5. interrupted Host operations use their domain-specific durable recovery rules.

The runtime does not reconstruct hidden content state outside public events, objects, receipts, and registered stores.

## Deliberate omissions

This lifecycle does not define turns, messages, prompts, model orchestration, memory updates, agent tasks, or world ticks. Protocols, Components, and Products may define those lifecycles independently.
