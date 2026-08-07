# Event Model

> [English](./EVENT_MODEL.en.md) · [中文](./EVENT_MODEL.md)

The current Contract V1 event log preserves facts that need durable order, audit, and causal relationships. It is organized by Session scope, append-only, durable, and ordered; large objects and portable content use ObjectStore and ArtifactDescriptor instead of copying every byte into the journal.

The runtime does not interpret event payloads. Adopted Protocols define shared meaning, while Components or Products own concrete domain state; Contract V1 uses the Package-writer namespace to identify event ownership.

## Envelope

Every persisted event uses the same envelope:

```text
EventEnvelope
- id                  unique event id
- session_id          target session
- sequence            monotonic per session
- timestamp           kernel-assigned
- writer_package_id   the package that produced the event (or "kernel")
- kind                namespaced string, e.g. "context/opened" or "org/name/event/foo"
- schema_version      payload schema version, owned by the writer
- payload             opaque JSON, validated only against the writer's declared schema
- metadata            opaque JSON; causation_id, correlation_id, trace ids, etc.
```

The kernel:

- assigns `id`, `sequence`, `timestamp`, and `writer_package_id`,
- enforces that `kind` is namespaced under the writer's id (or `kernel/v1/...` for kernel events),
- validates `payload` against the writer's declared schema when one is declared,
- treats `metadata` as opaque.

## Kinds

Event kinds fall into two groups.

### Kernel-emitted kinds

The kernel itself produces a small fixed set. These kinds describe kernel operations, not content.

Session:

```text
context/opened
context/closed
context/forked
```

Package lifecycle:

```text
host/package.loading
host/package.starting
host/package.ready
host/package.stopping
host/package.stopped
host/package.loaded
host/package.unloaded
host/package.degraded
host/package.log
```

Capability invocation (planned audit shape):

```text
capability/invoked
capability/completed
capability/failed
```

Permission audit:

```text
authority/grant.created
authority/grant.revoked
authority/denied
```

Generic substrate:

```text
object/put
projection/updated
```

Proposal lifecycle:

```text
change/proposal.created
change/proposal.approved
change/proposal.rejected
change/proposal.applied
change/proposal.failed
```

Transport / runtime errors (planned):

```text
runtime/error
```

These are the only event kinds the kernel knows about by name. Their payloads describe kernel operations, not content.

### Non-core event kinds

Under Contract V1, a Package writer declares non-core event kinds in its Manifest and uses its Package ID namespace. Long-term meaning may be owned by a Protocol, Component, or Product; Package is the current distribution and writer-identity boundary. These examples are illustrative and not part of the substrate:

```text
someorg/conversation/turn.started
someorg/conversation/prompt.rendered
someorg/conversation/model.streamed
someorg/world-sim/tick.completed
someorg/memory-pack/proposal.created
```

The runtime persists and orders these opaque events without interpreting their domain meaning.

## Permissions

Appending an event requires `events.append` in the writer's manifest. Reading an event stream requires `events.read`, and may be scoped to specific sessions.

A writer cannot append events under another owner's namespace. Cross-component or cross-protocol coordination uses public invocation, protocols, or extension points rather than impersonation in the log.

## Persistence rules

- Append-only. The log is never edited.
- Per-session ordering is monotonic. The kernel makes no cross-session ordering claim.
- Durable. After `journal/after_append` fires, the event is committed.
- Replayable. The kernel can stream events from `sequence` 0 forward.

## Replay

The kernel can replay events to:

- a newly subscribing client,
- a newly loaded package that requested catch-up,
- a snapshot tool.

The kernel replays envelopes verbatim. Meaning, projection, and state reconstruction belong to packages.

## Versioning

Each event kind carries a `schema_version`. The owning writer is responsible for migrations. The kernel does not migrate payloads; it persists what was written at the time.

A package can publish a new `schema_version` for its kind without changing the platform.

## Causation and correlation

The envelope's `metadata` may carry `causation_id` (the event that caused this one) and `correlation_id` (a logical trace). The kernel treats them as opaque. Packages decide what they mean.

## What this model deliberately omits

- No chat history concept.
- No turn or message concept.
- No prompt frame, context plan, or model call concept.
- No memory or world-state concept.
- No agent task or proposal concept.

Packages that need these concepts may define them as their own event kinds. None of them are kernel events.

## Stability

The kernel-emitted kind set is small by design. Adding a new kernel kind needs the same justification as adding a new kernel responsibility: it truly cannot live in a package.
