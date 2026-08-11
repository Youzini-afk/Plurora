# Powerbox, Exposure, and Cross-Installation Binding

> [English](./POWERBOX_BINDING.en.md) · [中文](./POWERBOX_BINDING.md)

This guide describes the Phase 5 cross-Installation Powerbox. It connects a provider's public Exposure to a consumer import Port without turning candidates, preferences, or UI into authority. Managed Realization, remote deployment compilation, and `host.realization.*` remain Phase 6; they must not be described as implemented managed deployment here.

## Authority and isolation

- The Host journal is durable authority for Exposure, Binding, Installation, and Realization. The public relay returns only filtered records; private intent and handles stay inside the Host.
- Exposure moves through `active → closing → revoked/expired`; a Binding decision moves through `selected/active → closing → terminal`. Close precedes the terminal commit, and repeated close/revoke/expiry converges through idempotent journal results.
- On Host restart, owner takeover, retry, or an uncertain effect result, the Host never guesses success. It records `outcome_unknown`/`recovery_required` (or an equivalent terminal diagnostic) and recovers through a new record.
- Exposure authority is limited to the provider Installation, Run, and exact export Port. Binding authority is limited to the consumer Installation, import Port, and Exposure, with an optional exact consumer Run. Omitting a selector ID never expands it.
- A composite Port resource ID is `<installation-id>/<port-id>`. It is not a path and never a capability handle. Grant basis is stored only as a hash; credentials are never stored.
- Before every external effect (Exposure create/revoke, Binding select/revoke, injection, or stop), refresh grant, ancestor delegation, lease, owner, and resource matching.

## Public methods and events

All Phase 5 methods are Host-owned and `implemented`, with typed request/result schemas:

```text
host.exposure.list
host.exposure.create
host.exposure.revoke
host.binding.list
host.binding.candidates
host.binding.select
host.binding.revoke
```

The corresponding events are:

```text
host/exposure.created
host/exposure.revoked
host/exposure.expired
host/binding.selected
host/binding.revoked
host/binding.expired
```

See [`../spec/v1/EVENT_KIND_REGISTRY.en.md`](../spec/v1/EVENT_KIND_REGISTRY.en.md) for payloads and [`../spec/v1/schemas/methods/`](../spec/v1/schemas/methods/) for request/results. Only `plurora/runtime` writes these events; Packages cannot impersonate them.

## Exposure and Binding data

`host.exposure.create` requires the provider Installation, Run, export Port, exact audience, expiry, expected revisions, and an `idempotency_key`. Audience is an explicit resource selector; publisher identity or a default device identity cannot substitute for it.

`host.binding.candidates` is effect-free. It requires the consumer Installation, import Port, `phase`, and an expected revision; optional `preferences` are ordering hints only. The result discloses:

- the exact Exposure, audience, and effective expiry;
- `PortDescriptor` on both consumer and provider sides;
- `PortContract` protocol, interface, version, and Profiles; interaction model, effect class, transport, multiplicity, availability, and latest binding phase;
- provider Work/Installation source and identity;
- provider Component version, entry kind, artifact/behavior digests, trust class, claim status, enforced boundaries, and protocol implementations;
- `candidate_digest`, stale/unknown gaps, and next steps.

Candidates sort deterministically without publisher priority. Zero candidates and multiple candidates both require an explicit choice. The current provider-candidate return cap is 256; overflow returns structured `work_too_complex` rather than silent truncation. Unknown fields or interactions are disclosed and never guessed or downgraded.

`host.binding.select` must return the selected `candidate_digest`, Exposure revision, and provider/consumer revisions; the Host then creates an opaque Binding ID. `host.binding.revoke` targets only the exact consumer/Port/Exposure/Binding and expected revision.

## Powerbox disclosure and choice

The official Web chooser shows the explicit binding phase (authoring, installation, launch, or runtime), then the exact Exposure, audience, expiry, and provider origin. Both PortContracts are shown with protocol, interface, version, Profiles, interaction, effects, transport, multiplicity, and availability.

Provider disclosure separates Work/Installation source, Component trust class, claims and enforced boundaries, artifact/behavior digests, protocol evidence, and stale state. First-party and third-party providers follow the same rules, and stable ordering never uses publisher priority.

A preference hint can affect presentation order only; it is not authority. It cannot write an active Binding, implicitly deploy, rebind, or expand scope. Unknown values remain visible and require a user or explicit policy decision.

## Runtime pins and injection

- The Host injects a least-authority runtime handle only into the selected Component. Handles, credentials, grant basis, and private intent never appear in UI, public events, or receipts.
- Each Binding pin records consumer/provider Installation revisions, Work/AssemblyLock references, root Port, full `node_path` and leaf Port, Component artifact/behavior/trust, and capability version.
- Launch phase carries no consumer Run pin. Runtime phase must pin `RunId`, `run_revision`, and `context_id` together. Multiple Ports on one Component may share one activation; a different Component or `node_path` is isolated.
- Unary and stream paths preserve authority, deadlines, cancellation, backpressure, and receipt semantics. Streams deliver frames only after their establishment barrier.
- Provider stop, Exposure revoke/expiry, authority revoke, version drift, or pin-digest mismatch cancels the Binding. The Host does not rebind automatically; the consumer follows `availability` (`required`, `degraded_without`, or `optional`).
- State, secrets, and activation context never cross Installations. Subprocess invocation tokens and background work require explicit declaration, separate grants, and revocation; they cannot be inferred from UI or ordinary events.

## CLI, Web, and Surface boundaries

The CLI uses public commands:

```bash
plurora exposure list|create|revoke
plurora binding candidates|list|select|revoke
```

Home/Library provides discovery and entry points; the Installation frame provides the chooser and exact Run/Installation context. PWA/mobile uses the same Host API, with caches isolated by Host and Installation. Closing a tab, iframe, or PWA connection does not stop a Run and does not revoke an Exposure or Binding.

Surfaces have only an explicitly allowlisted public bridge. There is no private Powerbox bridge, root credential, or hidden first-party route. A surface cannot select a provider directly, read a handle, deploy, or create a rebind.

## Verification and boundary

Six Phase 5 conformance cases cover method owner/status/typed DTOs, action-before-effect, 69-event payloads, no private authority, effect-free candidates/no auto-select, and exact launch/runtime Run pins:

```text
powerbox.public_method_identity_owner_typed_dto
powerbox.public_method_actions_no_effect_on_denial
powerbox.public_event_identity_payload
powerbox.public_wire_no_private_authority
powerbox.candidates_effect_free_no_auto_select
powerbox.launch_runtime_exact_run_pin
```

Generated-output and schema/SDK cleanliness use the Phase generation-chain hash `9c3923c6ffd365a3b7a0a5e64a177a9d89708e70e215e454bfd9b1a5098d6e94`. The current public contract is 92 methods, 69 events, 39 top-level schemas, and 200 schemas total; Phase 6 Realization remains planned.

Related documents: [`RUN_LIBRARY.en.md`](RUN_LIBRARY.en.md), [`INSTALLATION_MODEL.en.md`](INSTALLATION_MODEL.en.md), [`../architecture/HOST_RESOURCE_AUTHORITY.en.md`](../architecture/HOST_RESOURCE_AUTHORITY.en.md), and [`../spec/PUBLIC_CONTRACT.en.md`](../spec/PUBLIC_CONTRACT.en.md).
