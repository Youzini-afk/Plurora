# Public Contract v1

> [English](./PUBLIC_CONTRACT.en.md) · [中文](./PUBLIC_CONTRACT.md)

This document specifies Plurora's current v1 public contract. It defines the operative boundary for methods, events, errors, capability handles, Manifest declarations, schemas, negotiation, and conformance. Every participant uses the same owner-based identities and behavioral rules; the contract exposes no parallel alias surface.

v1 does not put content semantics into core mechanisms. Characters, worlds, prompts, models, messages, memory, and similar concepts belong to their protocols, components, or products rather than the constitutional substrate.

## Status language

- `implemented`: present in code and covered by tests or conformance.
- `partial`: the core path exists, but edge cases, transport parity, or production policy remain to be hardened.
- `planned`: reserved in the contract but not implemented; callers must not depend on it.

## Path A vs Path B

The v1 contract supports two first-class participation modes:

- **Path A** (default): a Package sets `entry.contract: "v1"` and accepts contract enforcement. Its Manifest declares capabilities, permissions, and effects; the runtime enforces them; invocation uses runtime-minted handles; lifecycle and audit events are recorded.
- **Path B**: a Package sets `entry.contract: "none"` and opts out of v1 capability enforcement. The Host may still run the process and records lifecycle, but does not inject v1 authority bindings.

Path A is for Packages that need platform authority, network, secrets, audit, and SDK support. Path B is for self-contained applications and tools that need hosting but no platform authority.

## Public method matrix (80)

Complete request/response schemas live under `docs/spec/v1/schemas/methods/`. Method names are stable public API. v1 only allows additive changes.

### `context.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `context.open` | implemented | Open a content-free session and write `context/opened`. |
| `context.close` | implemented | Close a session and write `context/closed`. |
| `context.fork` | partial | Create branch lineage from a parent session/sequence without interpreting content. |
| `context.branch.list` | partial | List branch records related to a session. |
| `context.get` | partial | Query one session; behavior and error contracts continue to harden. |
| `context.list` | planned | Reserved host-management list. |

### `journal.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `journal.append` | implemented | Enforce event ownership and `events.append` for Package writers. |
| `journal.list` | partial | List events with sequence, limit, kind, writer filters, and permission gates; backend parity continues to harden. |
| `journal.subscribe` | planned | The SSE replay/tail route exists; public method dispatch and package-principal subscribe permission are not implemented. |

### `host.package.*` (7)

| Method | Status | Contract |
|---|---:|---|
| `host.package.load` | partial | Validate manifest, host policy, Path A/Path B, entry constraints, register declarations, and emit lifecycle; some entry forms remain placeholders. |
| `host.package.unload` | partial | Stop execution, remove declarations, revoke runtime handles, and emit stop/unload events; full symmetry across entry forms continues to harden. |
| `host.package.list` | implemented | List in-memory package records. |
| `host.package.status` | implemented | Return one package record. |
| `host.package.restart` | partial | Supports subprocess restart; other entries are rejected by policy. |
| `host.package.logs` | partial | Captures subprocess stderr; stdout remains JSON-RPC frames. |
| `host.package.describe` | planned | Reserved descriptor query derivable from status manifest. |

### `capability.*` (5)

| Method | Status | Contract |
|---|---:|---|
| `capability.discover` | implemented | List registered capability descriptors. |
| `capability.describe` | planned | Reserved single-descriptor query. |
| `capability.invoke` | partial | Enforce caller context and capability handles, validate schemas, attach EffectReceipt to completed/failed terminals, and support recorded replay plus branch re-execution; parity across entries and transports continues to harden. |
| `capability.stream` | partial | Start a streaming invocation with ordered start/chunk/progress/terminal frames; ended/error/cancelled/timeout produce distinct terminal receipts. |
| `capability.cancel` | partial | Cancel a known active invocation, prevent further chunks, and record a cancelled terminal state. |

### `authority.handle.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `authority.handle.attenuate` | partial | Derive a child handle from a parent; constraint-subset validation needs hardening. |
| `authority.handle.revoke` | partial | Immediately revoke a handle; complete descendant propagation needs hardening. |
| `authority.handle.list` | partial | List live handles held by a package; delegation and lease refresh are not complete. |

### `authority.grant.*` / `authority.decision.*` (4)

| Method | Status | Contract |
|---|---:|---|
| `authority.grant.create` | partial | Host-dev grants scoped permission to human/assistant principals and writes audit. |
| `authority.grant.revoke` | partial | Revoke scoped permission and write audit. |
| `authority.grant.list` | partial | List current grants. |
| `authority.decision.list` | partial | Query grant/revoke audit. |

### `change.proposal.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `change.proposal.create` | partial | Create approval-gated generic changes. |
| `change.proposal.get` | partial | Fetch a proposal. |
| `change.proposal.list` | partial | List proposals. |
| `change.proposal.approve` | partial | Require proposal-scoped review authority, mark approved, and emit an event. |
| `change.proposal.reject` | partial | Require proposal-scoped review authority, mark rejected, and emit a denied receipt/event. |
| `change.proposal.apply` | partial | Recheck apply plus required authority, map the current proposal facade into Intent/ChangeSet/PolicyDecision/Commit evidence, preflight object/projection operations, and CAS-record committed/failed/partial receipts. |

### `object.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `object.put` | partial | Store opaque asset metadata, block raw secrets, write `object/put`. |
| `object.get` | partial | Read an asset record. |
| `object.list` | partial | List asset records. |

### `projection.*` (4)

| Method | Status | Contract |
|---|---:|---|
| `projection.register` | partial | Register a generic projection descriptor. |
| `projection.rebuild` | partial | Rebuild from event filters and write `projection/updated`. |
| `projection.get` | partial | Read projection state. |
| `projection.list` | partial | List projections. |

### `host.outbound.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `host.outbound.audit` | partial | Query outbound audit and terminal receipt descriptors; cross-executor view parity continues to harden. |
| `host.outbound.execute` | partial | Manifest-gated unary HTTPS outbound with `secret_ref` support; denied/error/success all produce receipts. |
| `host.outbound.stream` | partial | Manifest-gated SSE/NDJSON/raw streaming outbound with terminal completion receipts. |
| `host.outbound.websocket.open` | partial | Open a Manifest-gated WSS connection through Host policy, secret resolution, audit, and the streaming event boundary. |
| `host.outbound.websocket.send` | partial | Send a bounded frame on a Host-owned connection after caller and connection checks. |
| `host.outbound.websocket.close` | partial | Close a Host-owned connection and emit the terminal completion evidence. |

Git installation is not a transport primitive; it belongs in the ordinary first-party capability Package `plurora/git-tools-lab` using `host.outbound.execute` plus declared filesystem authority.

### `host.target.*` / `host.exec.*` / `host.port.*` / `host.proxy.*` (17)

| Method | Status | Contract |
|---|---:|---|
| `host.target.list` | partial | HostAdmin/HostDev only; list execution targets. |
| `host.target.status` | partial | HostAdmin/HostDev only; inspect one target. |
| `host.target.register` | partial | HostAdmin/HostDev only; register a controlled target. |
| `host.target.unregister` | partial | HostAdmin/HostDev only; unregister a target. |
| `host.exec.start` | partial | HostAdmin/HostDev only; start controlled execution through the host `LocalExecExecutor`; deny-all by default, with receipts on denied/failed terminal paths. |
| `host.exec.stop` | partial | HostAdmin/HostDev only; stop a known execution and produce a cancelled/failed/denied receipt. |
| `host.exec.status` | partial | HostAdmin/HostDev only; return the runtime-observed status. Live executors are actively monitored, and status/stop/restart races reuse the unique persisted terminal receipt. |
| `host.exec.logs` | partial | HostAdmin/HostDev only; read redacted log tail. |
| `host.exec.list` | partial | HostAdmin/HostDev only; list execution records. |
| `host.port.lease` | partial | HostAdmin/HostDev only; lease a loopback port. |
| `host.port.release` | partial | HostAdmin/HostDev only; release a port lease. |
| `host.port.status` | partial | HostAdmin/HostDev only; inspect a port lease. |
| `host.port.list` | partial | HostAdmin/HostDev only; list port leases. |
| `host.proxy.register` | partial | HostAdmin/HostDev only; register an HTTP/WebSocket route; upstream must reference an active port lease and matching `port_name`. |
| `host.proxy.unregister` | partial | HostAdmin/HostDev only; unregister a route. |
| `host.proxy.status` | partial | HostAdmin/HostDev only; inspect a route. |
| `host.proxy.list` | partial | HostAdmin/HostDev only; list routes. |


### `host.project.*` (5)

| Method | Status | Contract |
|---|---:|---|
| `host.project.list` | implemented | HostAdmin/HostDev only; list installed projects and state. |
| `host.project.get` | implemented | HostAdmin/HostDev only; return one project's full descriptor and registry record; includes `running_session_id` when running. |
| `host.project.start` | implemented | HostAdmin/HostDev only; transition an Installed/Stopped project to Running, open a project session, return `session_id` and `already_running`, and emit lifecycle events. |
| `host.project.stop` | implemented | HostAdmin/HostDev only; stop a Running project and emit lifecycle events. |
| `host.project.status` | implemented | HostAdmin/HostDev only; return project state and last error; includes `running_session_id` when running. |

### `host.*` / `identity.current` (4)

| Method | Status | Contract |
|---|---:|---|
| `host.info` | implemented | Return protocol version, methods, statuses, and transport labels. |
| `host.ping` | partial | Reserved lightweight health check. |
| `host.diagnostics` | partial | Return local package/capability/hook diagnostics. |
| `identity.current` | planned | Reserved identity-provider integration. |

### `host.package.audit` (1)

| Method | Status | Contract |
|---|---:|---|
| `host.package.audit` | partial | Report declared vs used authority for `plurora audit --package <id>`; actual-use tracking continues to expand. |

### Surface / extension point / hook (6)

| Method | Status | Contract |
|---|---:|---|
| `shell.contribution.list` | partial | List typed package-declared surface contributions. |
| `shell.contribution.describe` | partial | Describe one contribution. |
| `host.surface.bundle.resolve` | partial | HostAdmin/HostDev only; resolve a mountable bundle URL from a surface contribution, project dev path, or installed project; cross-source parity continues to harden. |
| `protocol.extension.list` | implemented | List extension points. |
| `protocol.extension.describe` | planned | Describe one extension point. |
| `protocol.hook.list` | partial | List hook subscriptions. |

## Event kind matrix (59)

The full registry is [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md). Event payload schemas live under `docs/spec/v1/schemas/events/`.

| Group | Count | Examples |
|---|---:|---|
| context | 3 | `context/opened`, `context/closed`, `context/forked` |
| Package lifecycle | 9 | `host/package.loading`, `.starting`, `.ready`, `.loaded`, `.stopping`, `.stopped`, `.unloaded`, `.degraded`, `.log` |
| Project lifecycle | 4 | `host/project.installed`, `.started`, `.stopped`, `.uninstalled` |
| Capability lifecycle | 3 | `capability/invoked`, `capability/completed`, `capability/failed` |
| Stream lifecycle | 7 | `capability/stream.started`, `.chunk`, `.progress`, `.ended`, `.error`, `.cancelled`, `.timeout` |
| Authority | 3 | `authority/grant.created`, `authority/grant.revoked`, `authority/denied` |
| Change proposals | 5 | `change/proposal.created`, `.approved`, `.rejected`, `.applied`, `.failed` |
| Objects / projections | 2 | `object/put`, `projection/updated` |
| Outbound / WebSocket | 8 | `host/outbound.request`, `.denied`, completion events, WebSocket frames |
| Exec | 6 | `host/exec.request`, `.started`, `.completed`, `.failed`, `.stopped`, `.denied` |
| Port | 3 | `host/port.leased`, `.released`, `.denied` |
| Proxy | 3 | `host/proxy.registered`, `.unregistered`, `.denied` |
| Deployment | 2 | `host/deployment.reconciled`, `host/deployment.health` |
| error | 1 | `runtime/error` |

Package-owned event kinds must start with the exact writer Package ID followed by `/`. Registered platform-owned kinds may be written only by `plurora/runtime`; Packages cannot impersonate them or another Package namespace.

## Capability handle model

Manifest strings are the **authority ceiling**. Runtime handles are the **actual authority**. A Package cannot gain authority by forging strings; it must use handles minted by the runtime during load, handshake, or init.

- `authority.handle.attenuate(parent, constraints)` → child handle.
- `authority.handle.revoke(handle)` → immediately invalid.
- `authority.handle.list(package_id)` → all live handles currently held.

Handle fields:

- `id`: unforgeable runtime-minted identifier.
- `cap_type`: authority type, such as capability invoke, events read, or outbound.
- `cap_version`: handle semantic version.
- `scope`: package, session, capability, provider, host, or related scope.
- `constraints`: methods, hosts, schemas, counts, byte limits, deadlines, and similar limits.
- `lease`: expiry or lease policy.
- `provenance`: who minted it, why, and which manifest declaration it came from.
- `parent`: optional parent handle for attenuation trees and revocation propagation.

See [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.en.md).

## Binding injection model

Each entry form receives bindings at startup:

| Entry | Injection | v1 status |
|---|---|---:|
| `subprocess` | `package.handshake` receives/returns a `bindings` dictionary; SDK exposes `pluroraClient` and handles. | implemented |
| `rust_inproc` | `ComponentEnv` is passed to `InprocPackage::init` with runtime bindings. | implemented |
| `wasm` | WIT resource imports. | planned |
| `remote` | SPIFFE + Biscuit token exchange. | planned |

Bindings must contain only the caller's granted authority. Path B packages do not receive v1 capability bindings.

## Effect audit

`plurora audit --package <id>` and `host.package.audit` report declared vs used authority. Audit input comes from:

1. manifest permissions, capabilities, secret_refs, and network hosts;
2. runtime-minted and attenuated capability handles;
3. `capability.invoked|completed|failed` and outbound audit events;
4. permission grants/revokes and package lifecycle;
5. Path B's `contract_mode: "none"` marker.

Audit reports find unused declarations, undeclared use, authority expansion, expired-handle use, use after revoke, undeclared `secret_ref`s, and undeclared network targets. See the audit section in [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.en.md).

## Conformance kit

Third-party packages can run:

```bash
plurora conformance package --contract v1 --path <package>
```

The kit has 8 acceptance checks: manifest parse, contract mode, entry support, bindings/handshake, capability declarations, permission declarations, audit visibility, and fixture invocation. It outputs PASS/FAIL/SKIP/WARNING and a compliance percentage. Path A packages must pass applicable checks; Path B packages skip capability/permission checks but must remain self-contained and lifecycle-observable.

See [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.en.md).

## SDK generation

`docs/spec/v1/schemas/` is the single source of truth. SDKs are available through three channels:

- npm: `@plurora/contract-sdk` (`sdk/typescript/contract-sdk/`).
- workspace path: `file:../plurora/sdk/typescript/contract-sdk`.
- generate yourself: read `docs/spec/v1/schemas/` with any codegen tool.

See [`../../sdk/README.md`](../../sdk/README.md).

## Versioning strategy

See [`v1/VERSIONING.md`](v1/VERSIONING.en.md).

v1 only allows additive changes: optional fields, new methods, new events, new error codes, and new schemas. Removing fields, changing requiredness, changing semantics, or renaming methods/events is breaking and must go into a v2 namespace.

## Schemas and error codes

- Method schemas: `docs/spec/v1/schemas/methods/` (80).
- Event schemas: `docs/spec/v1/schemas/events/` (59).
- Top-level schemas: `docs/spec/v1/schemas/*.schema.json` (22), including additive Protocol Commons, component/package-envelope, World Bundle, and protocol-response descriptors.
- Error codes: [`v1/ERROR_CODES.md`](v1/ERROR_CODES.en.md).
- Event registry: [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md).

All 161 schemas must pass `cargo run -p plurora-cli --bin validate-schemas`.

## Content-free invariant

The constitutional substrate and public-contract runtime must not require content-shaped concepts such as `Turn`, `Message`, `PromptFrame`, `ModelCall`, `Agent`, `World`, `Scene`, `Director`, or `Memory`. Those concepts belong to the relevant Protocol, Component, Product, or Client; Contract V1 may carry them as opaque Package-owned events, objects, projections, and capabilities.

## Object contracts

### `SessionRecord`

`SessionRecord` is a content-free execution context. It may hold identity, labels, active package set, principal scope, status, timestamps, and metadata. It must not hold messages, turns, prompts, characters, worlds, memory, or model calls.

A session id only identifies current runtime ordering and permission scope. Protocols or Components may express domain state through the current Package writer's event payloads, objects, or projections, while the runtime treats them as opaque data and public descriptors.

### `EventEnvelope`

`EventEnvelope` is append-only fact storage. Each envelope contains at least session id, sequence, writer package id, kind, schema version, timestamp, payload, and metadata. Sequence is monotonic per session.

The runtime validates ownership, authority, and schema shape only. Event meaning belongs to its semantic owner.

### `PackageManifest`

A manifest declares package identity, entry, contract mode, provided capabilities, consumed capabilities, surface contributions, hooks, extension points, asset/schema declarations, permissions, and sandbox policy.

## Package dependencies (manifest.requires)

`requires` is the first-class package dependency declaration field in the manifest. It expresses which other packages must be resolved and installed for this package. It is distinct from `consumes`: `consumes` declares capability requirements, while `requires` declares package dependency data. It is not a protocol method and does not grant runtime authority; installers use it to resolve dependencies and write lockfiles, while runtime authorization still comes from permissions, bindings, and capability handles.

```yaml
requires:
  - id: plurora/model-provider-lab
    source:
      kind: git
      url: https://example.com/plurora/model-provider-lab.git
      ref: v1.2.3
    version: "^1.2"
    minimum_signed_by:
      - "0123456789ABCDEF0123456789ABCDEF01234567"
```

Actual install and resolution are handled by `plurora/install-lab`; the constitutional substrate does not resolve dependencies.
See [`docs/guides/PACKAGE_INSTALLATION.md`](../guides/PACKAGE_INSTALLATION.en.md).

The manifest is audit and handle-minting input, not runtime authority. Runtime authority is expressed through bindings and capability handles.

### `PackageRecord`

`PackageRecord` tracks package id, version, entry kind, contract mode, trust level, state, manifest summary, capability/hook/surface counts, and state timestamps. Records support host diagnostics, package status, lifecycle audit, and the conformance kit.

### `CapabilityDescriptor`

A descriptor describes a provider-owned capability: id, version, input schema, output schema, streaming flag, side effects, description, and metadata. It does not grant call authority; call authority comes from the caller's handle.

### `HookSubscription`

Hook subscriptions come from Manifests. The runtime owns stable ordering, unload cleanup, and the four currently implemented journal/capability interception points. Generic arbitrary hook-handler execution remains incomplete; see [`../architecture/EXTENSION_POINTS.md`](../architecture/EXTENSION_POINTS.en.md).

### `AssetRecord`

An asset record is opaque metadata: id, origin Package, mime, hash, size, and metadata. The runtime does not interpret asset content.

## Permission and denial semantics

Permission checks must fail closed. Missing handle, expired handle, revoked handle, scope mismatch, schema mismatch, host-policy denial, and missing manifest declaration all deny the call.

Denials should produce structured errors and, where applicable, audit events. Errors must not leak raw secrets, full request bodies, user content, or provider credentials.

Host-dev operations must be explicit in protocol context. Anonymous host calls must not become package privilege.

## Namespace rules

Public method IDs are exact owner-based dot names. The first segment identifies Substrate, Host, Protocol, or Shell ownership, for example `context.open`, `host.project.list`, `change.proposal.apply`, and `shell.contribution.list`.

Platform-owned event kinds are the explicit 59-entry registry and may be written only by `plurora/runtime`. Package-owned event kinds must begin with the exact Package ID followed by `/`. Package capability IDs follow the same Package-owned slash namespace convention.

Reserved rules:

- the current registry exposes one wire ID per public method and no aliases;
- a Package cannot claim a public method ID or a registered platform-owned event kind;
- `plurora/*` identifies first-party Packages but grants no runtime authority or routing priority;
- breaking method, event, or schema semantics require a new explicit version boundary rather than an in-place rename.

The executable identity and negotiation rules are defined by [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.en.md).

## Schema rules

v1 schemas are release artifacts. Each method schema describes request and response. Each event schema describes payload. Top-level schemas describe manifest, permission, protocol context, capability descriptor, and shared objects.

Schema change rules:

1. Optional fields may be added.
2. Enum values may be added, but callers must treat unknown values as recoverable extensions.
3. Fields must not be deleted.
4. Optional fields must not become required.
5. Field semantics must not change.
6. Methods, events, and error codes must not be renamed.

## Transport parity

The same protocol envelope can be carried by in-process dispatcher, HTTP `/rpc`, host JSON-RPC stdio, and future transports. Transport must not alter authorization semantics.

HTTP and stdio can frame differently, but request id, method, params, optional contract selection, context, and result/error semantics must match. An unsatisfied explicit selection returns `unsupported_contract` and never silently downgrades. See [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.en.md).

## Package lifecycle

Implemented entries should move through loading, starting, ready, and loaded. Stop moves through stopping, stopped, and unloaded. Execution failure or health loss emits degraded.

Lifecycle events must let operators and the conformance kit distinguish:

- whether the manifest was accepted or rejected;
- whether the entry started;
- whether handshake completed;
- whether contract mode is `v1` or `none`;
- whether unload revoked handles;
- whether subprocess stderr was captured as logs.

## Subprocess contract

Subprocess stdout is JSON-RPC protocol frames and must not contain ordinary logs. Logs go to stderr. The host may capture stderr as package log events.

Handshake must declare package id, protocol version, contract mode, available capability endpoints, and binding compatibility. Path A handshake failure should prevent ready. Path B may use a narrower handshake, but must let the host determine that it is self-contained.

## Rust in-process contract

Rust in-process packages load only through the host catalog. Manifest-declared in-process entries must map to host-provided trait implementations. Missing catalog entries fail closed.

In-process Packages do not gain first-party privilege. They participate in v1 through `ComponentEnv`, bindings, handles, schemas, and audit.

## WASM and remote reservations

WASM and remote are first-class manifest entry forms, with execution to be completed. v1 reserves their contract shape:

- WASM uses WIT resources to express handles.
- Remote uses mTLS/SPIFFE identity and Biscuit tokens for attenuated authority.
- Both must follow the same schema, event, audit, and namespace rules.

## Outbound execution boundary

Outbound requests gain platform-managed network authority only through v1 outbound primitives. Manifests must declare host, method, purpose, and required `secret_ref`s. Host policy may narrow further.

Audit records contain destination, method, package id, capability id, purpose, redaction state, `secret_ref` references, status, duration, and counts. Raw bodies, headers, prompts, responses, and raw secrets must not be written to events.

## Secret reference contract

Packages pass references such as `secret_ref:<vault>:<key>`. The host resolver resolves them at runtime. Resolved values only enter executors or provider adapters; they are not written back to events, logs, proposals, or audit.

```yaml
secret_ref:env:OPENAI_API_KEY    # resolved via host env var (allowlisted)
secret_ref:store:OPENAI_API_KEY  # resolved via local encrypted store
secret_ref:project:OPENAI_API_KEY # resolved via project store, then policy fallback
```

Project-backed references resolve from the active project store first, then fall back to the platform store when `secret_policy.fallback_to_platform` allows it and the key is not listed in `require_per_project`.

Store-backed references are resolved via the `StoreSecretResolver` against an age-encrypted file at `~/.plurora/secrets.dat`. See [`docs/guides/SECRET_MANAGEMENT.md`](../guides/SECRET_MANAGEMENT.en.md).

Undeclared secret refs, resolution failure, resolver denial, and raw secrets in protected payloads must fail closed.

## Proposal contract

A proposal is an approval-gated change, not a content model. The runtime manages the create, approve, reject, apply, and failed states. Approval/apply boundaries enforce explicit authority, terminal transitions are compare-and-set guarded, and operation payload remains opaque JSON subject to raw-secret scanning and schema checks.

Current apply supports generic asset/projection operations. Broader transactions, compensation, and revert are later work.

## Surface contract

A surface contribution is a Package-declared UI/UX entry descriptor. The runtime stores and lists descriptors; it does not render UI or interpret content semantics. The Host shell decides how to mount iframe, bundle, or native surfaces.

First-party and third-party surfaces use the same descriptors, permission declarations, and review path.

## Conformance requirements

A v1 implementation must at least prove:

1. 80 method schemas export.
2. 59 event schemas validate.
3. 22 top-level schemas validate.
4. Method registry and dispatcher are consistent.
5. Capability handle mint/attenuate/revoke/list behavior is testable.
6. Invoke instrumentation emits lifecycle events.
7. Binding injection covers subprocess and rust_inproc.
8. Path B self-contained mode is observable.
9. Package audit reports explain declared vs used authority.

## Operator visibility

Host operators should be able to see through public methods or CLI:

- loaded packages and contract modes;
- capabilities, surfaces, and hooks for each package;
- live handles and revoke state;
- denied permission, outbound, secret, and schema errors;
- Path B package lifecycle and logs;
- conformance percentage and failure reasons.

## Canonical references

Long-term references point to `PUBLIC_CONTRACT.md`. The Contract Registry, error codes, versioning rules, event registry, and schemas under `docs/spec/v1/` are its machine-readable companions.

## Appendix A: method owner-prefix counts

| Prefix | Count |
|---|---:|
| `authority.*` | 7 |
| `capability.*` | 5 |
| `change.*` | 6 |
| `context.*` | 6 |
| `host.*` | 40 |
| `identity.*` | 1 |
| `journal.*` | 3 |
| `object.*` | 3 |
| `projection.*` | 4 |
| `protocol.*` | 3 |
| `shell.*` | 2 |

## Appendix B: release checks

Before releasing a v1 Host, run:

```bash
cargo test -p plurora-core
cargo test -p plurora-runtime
cargo test -p plurora-cli
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli --bin export-schemas
cargo run -p plurora-cli --bin validate-schemas
cargo run -p plurora-cli --bin generate-sdks
```

Also run package conformance against representative Path A and Path B examples.

## Appendix C: non-goals

v1 does not promise:

- chat, agent, model, world, memory, or director semantics in the constitutional substrate;
- arbitrary subprocess OS-level network interception;
- production-grade secret vault integration;
- completed WASM / remote execution;
- marketplace, package-signing network, or dependency-resolution economy;
- UI framework or Studio private APIs.

Those capabilities may be provided by ordinary packages, host policy, or future rounds, but must not break this contract's invariants.

## Other references

- [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md)
- [`v1/ERROR_CODES.md`](v1/ERROR_CODES.en.md)
- [`v1/VERSIONING.md`](v1/VERSIONING.en.md)
- [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.en.md)
- [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.en.md)
- [`../guides/PATH_B_SELF_CONTAINED.md`](../guides/PATH_B_SELF_CONTAINED.en.md)
