# Event Model

> [English](./EVENT_MODEL.en.md) · [中文](./EVENT_MODEL.md)

The public-contract journal preserves facts that need durable order, audit, and causal relationships. It is append-only, scoped by Context, durable, and ordered. Large or portable content belongs in ObjectStore and ArtifactDescriptor references rather than being copied into every event.

The runtime does not interpret domain payloads. Adopted Protocols define shared meaning, while Components and Products own concrete state.

## Envelope

Every persisted event uses the same `EventEnvelope`:

```text
id                  unique event id
session_id          target context
sequence            monotonic within the context
timestamp           runtime-assigned
writer_package_id   runtime or Package writer identity
kind                owner-based event kind
schema_version      payload schema version
payload             opaque JSON
metadata            opaque JSON for causation, correlation, traces, and hints
```

The runtime assigns `id`, `sequence`, `timestamp`, and the effective writer identity. A Package principal cannot self-assert a different writer.

## Platform-owned event kinds

The 69 platform-owned kinds are an explicit registry, not a magic string prefix. They use semantic owner namespaces such as:

```text
context/opened
host/package.loading
host/run.started
host/exposure.created
host/binding.selected
capability/stream.started
authority/grant.created
object/put
projection/updated
change/proposal.applied
runtime/error
```

Only writer `plurora/runtime` may append a registered platform-owned kind. The canonical 69-entry list and payload schemas are in [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.en.md). Exposure and Binding public relays return only Host-authorized journal projections; private intent, grant basis, and runtime handles never enter events.

This explicit registry matters because platform events span Substrate, Host, Protocol, and runtime concerns. A single reserved prefix would hide ownership rather than clarify it.

## Package-owned event kinds

A Package event kind must begin with its exact Package ID followed by `/`:

```text
someorg/conversation/turn.started
someorg/world-sim/tick.completed
someorg/memory-pack/proposal.created
```

A Package cannot append another Package's kind or impersonate a registered platform-owned kind. Cross-Package coordination uses public capability invocation, adopted Protocols, or extension points rather than writer impersonation.

## Validation and authority

Appending requires `events.append` in the writer Package Manifest unless the writer is `plurora/runtime`. When a Package declares a payload schema for one of its event kinds, the runtime validates the payload against that schema subset before persistence.

Reading journal data requires the applicable public-method authority and may be scoped by Context, sequence range, writer, or kind prefix.

## Persistence rules

- Append-only: persisted events are not edited.
- Monotonic ordering: sequence is monotonic within one Context; no cross-Context total order is promised.
- Durable: append succeeds only after the EventStore commits the envelope.
- Replayable: consumers can read from a sequence cursor and reconstruct their own projections.
- Opaque by default: the runtime owns envelope integrity, not domain interpretation.

## Replay and projections

Replay serves newly connected clients, rebuilding Components, audit tools, and projection materializers. The runtime returns the stored envelope unchanged. Protocol or Component code interprets payloads and performs migrations.

Each event kind carries `schema_version`. The semantic owner is responsible for version evolution; the EventStore preserves what was written.

## Causation and correlation

`metadata` may contain `causation_id`, `correlation_id`, trace identifiers, or other owner-defined fields. The runtime treats these fields as opaque and never infers domain meaning from them.

## Deliberate omissions

The platform event registry does not define chat history, turns, prompts, model calls, memory, worlds, or agent tasks as universal ontology. A Protocol or Package may define such facts under its own ownership without promoting them into the constitutional substrate.
