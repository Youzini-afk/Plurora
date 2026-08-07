# Kernel v1 Contract

> [English](./KERNEL_V1_CONTRACT.en.md) · [中文](./KERNEL_V1_CONTRACT.md)

This document is the v1 specification for Yggdrasil's current platform contract. It defines the operative public boundary: methods, events, error codes, capability handles, manifest declarations, schemas, and conformance expectations. Any participant can call the platform through this contract; every implementation must satisfy the same schema and behavioral conformance. `kernel.v1.*` is a compatibility name rather than automatic membership in the long-term constitutional substrate.

v1 does not put content semantics into core mechanisms. Characters, worlds, prompts, models, messages, memory, and similar concepts belong to their protocols, components, or products rather than the constitutional substrate.

## Status language

- `implemented`: present in code and covered by tests or conformance.
- `partial`: the core path exists, but edge cases, transport parity, or production policy remain to be hardened.
- `planned`: reserved in the contract but not implemented; callers must not depend on it.

## Path A vs Path B

The v1 contract supports two first-class participation modes:

- **Path A** (default): a package sets `entry.contract: "v1"` and accepts contract enforcement. Its manifest declares capabilities, permissions, and effects; the runtime enforces permissions; invocation uses kernel-minted handles; lifecycle and audit events are recorded.
- **Path B**: a package sets `entry.contract: "none"` and opts out of contract enforcement. The kernel still hosts the process and emits lifecycle events, but does not enforce v1 capability/permission checks or inject v1 bindings.

Path A is for packages that need kernel capabilities, network, secrets, audit, and SDK support. Path B is for self-contained apps, migration tools, and third-party processes that do not need platform authority.

## Protocol method matrix (80)

Complete request/response schemas live under `docs/spec/v1/schemas/methods/`. Method names are stable public API. v1 only allows additive changes.

### `kernel.v1.session.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.session.open` | implemented | Open a content-free session and write `kernel/v1/session.opened`. |
| `kernel.v1.session.close` | implemented | Close a session and write `kernel/v1/session.closed`. |
| `kernel.v1.session.fork` | partial | Create branch lineage from a parent session/sequence without interpreting content. |
| `kernel.v1.session.branch.list` | partial | List branch records related to a session. |
| `kernel.v1.session.get` | partial | Query one session; behavior and error contracts continue to harden. |
| `kernel.v1.session.list` | planned | Reserved host-management list. |

### `kernel.v1.event.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.event.append` | implemented | Enforce writer namespace and `events.append` for non-kernel writers. |
| `kernel.v1.event.list` | partial | List events with sequence, limit, kind, writer filters, and permission gates; backend parity continues to harden. |
| `kernel.v1.event.subscribe` | planned | The SSE replay/tail route exists; public method dispatch and package-principal subscribe permission are not implemented. |

### `kernel.v1.package.*` (7)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.package.load` | partial | Validate manifest, host policy, Path A/Path B, entry constraints, register declarations, and emit lifecycle; some entry forms remain placeholders. |
| `kernel.v1.package.unload` | partial | Stop execution, remove declarations, revoke runtime handles, and emit stop/unload events; full symmetry across entry forms continues to harden. |
| `kernel.v1.package.list` | implemented | List in-memory package records. |
| `kernel.v1.package.status` | implemented | Return one package record. |
| `kernel.v1.package.restart` | partial | Supports subprocess restart; other entries are rejected by policy. |
| `kernel.v1.package.logs` | partial | Captures subprocess stderr; stdout remains JSON-RPC frames. |
| `kernel.v1.package.describe` | planned | Reserved descriptor query derivable from status manifest. |

### `kernel.v1.capability.*` (5)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.capability.discover` | implemented | List registered capability descriptors. |
| `kernel.v1.capability.describe` | planned | Reserved single-descriptor query. |
| `kernel.v1.capability.invoke` | partial | Enforce caller context and capability handles, validate schemas, attach EffectReceipt to completed/failed terminals, and support recorded replay plus branch re-execution; parity across entries and transports continues to harden. |
| `kernel.v1.capability.stream` / `cancel` | partial | Streaming lifecycle, cancellation, timeout, and events exist; ended/error/cancelled/timeout produce distinct terminal receipts; transport parity continues to harden. |

### `kernel.v1.cap.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.cap.attenuate` | partial | Derive a child handle from a parent; constraint-subset validation needs hardening. |
| `kernel.v1.cap.revoke` | partial | Immediately revoke a handle; complete descendant propagation needs hardening. |
| `kernel.v1.cap.list_for` | partial | List live handles held by a package; delegation and lease refresh are not complete. |

### `kernel.v1.permission.*` (4)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.permission.grant` | partial | Host-dev grants scoped permission to human/assistant principals and writes audit. |
| `kernel.v1.permission.revoke` | partial | Revoke scoped permission and write audit. |
| `kernel.v1.permission.list` | partial | List current grants. |
| `kernel.v1.permission.audit` | partial | Query grant/revoke audit. |

### `kernel.v1.proposal.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.proposal.create` | partial | Create approval-gated generic changes. |
| `kernel.v1.proposal.get` | partial | Fetch a proposal. |
| `kernel.v1.proposal.list` | partial | List proposals. |
| `kernel.v1.proposal.approve` | partial | Require proposal-scoped review authority, mark approved, and emit an event. |
| `kernel.v1.proposal.reject` | partial | Require proposal-scoped review authority, mark rejected, and emit a denied receipt/event. |
| `kernel.v1.proposal.apply` | partial | Recheck apply plus required authority, adapt the old Proposal into Intent/ChangeSet/PolicyDecision/Commit, preflight asset/projection operations, and CAS-record operation plus committed/failed/partial receipts. |

### `kernel.v1.asset.*` (3)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.asset.put` | partial | Store opaque asset metadata, block raw secrets, write `kernel/v1/asset.put`. |
| `kernel.v1.asset.get` | partial | Read an asset record. |
| `kernel.v1.asset.list` | partial | List asset records. |

### `kernel.v1.projection.*` (4)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.projection.register` | partial | Register a generic projection descriptor. |
| `kernel.v1.projection.rebuild` | partial | Rebuild from event filters and write `kernel/v1/projection.updated`. |
| `kernel.v1.projection.get` | partial | Read projection state. |
| `kernel.v1.projection.list` | partial | List projections. |

### `kernel.v1.outbound.*` (6)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.outbound.audit` | partial | Query outbound audit and terminal receipt descriptors; cross-executor view parity continues to harden. |
| `kernel.v1.outbound.execute` | partial | Manifest-gated unary HTTPS outbound with `secret_ref` support; denied/error/success all produce receipts. |
| `kernel.v1.outbound.stream` | partial | Manifest-gated SSE/NDJSON/raw streaming outbound with terminal completion receipts. |
| `kernel.v1.outbound.websocket.*` | partial | Manifest-gated WSS open/send/close; connection lifecycle and event coverage continue to harden. |

Git installation is not a kernel transport; future support belongs in the ordinary official capability package `official/git-tools-lab` using `kernel.v1.outbound.execute` plus `permissions.filesystem.write`.

### `kernel.v1.target.*` / `exec.*` / `port.*` / `proxy.*` (17)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.target.list` | partial | HostAdmin/HostDev only; list execution targets. |
| `kernel.v1.target.status` | partial | HostAdmin/HostDev only; inspect one target. |
| `kernel.v1.target.register` | partial | HostAdmin/HostDev only; register a controlled target. |
| `kernel.v1.target.unregister` | partial | HostAdmin/HostDev only; unregister a target. |
| `kernel.v1.exec.start` | partial | HostAdmin/HostDev only; start controlled execution through the host `LocalExecExecutor`; deny-all by default, with receipts on denied/failed terminal paths. |
| `kernel.v1.exec.stop` | partial | HostAdmin/HostDev only; stop a known execution and produce a cancelled/failed/denied receipt. |
| `kernel.v1.exec.status` | partial | HostAdmin/HostDev only; return the runtime-observed status. Live executors are actively monitored, and status/stop/restart races reuse the unique persisted terminal receipt. |
| `kernel.v1.exec.logs` | partial | HostAdmin/HostDev only; read redacted log tail. |
| `kernel.v1.exec.list` | partial | HostAdmin/HostDev only; list execution records. |
| `kernel.v1.port.lease` | partial | HostAdmin/HostDev only; lease a loopback port. |
| `kernel.v1.port.release` | partial | HostAdmin/HostDev only; release a port lease. |
| `kernel.v1.port.status` | partial | HostAdmin/HostDev only; inspect a port lease. |
| `kernel.v1.port.list` | partial | HostAdmin/HostDev only; list port leases. |
| `kernel.v1.proxy.register` | partial | HostAdmin/HostDev only; register an HTTP/WebSocket route; upstream must reference an active port lease and matching `port_name`. |
| `kernel.v1.proxy.unregister` | partial | HostAdmin/HostDev only; unregister a route. |
| `kernel.v1.proxy.status` | partial | HostAdmin/HostDev only; inspect a route. |
| `kernel.v1.proxy.list` | partial | HostAdmin/HostDev only; list routes. |


### `kernel.v1.project.*` (5)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.project.list` | implemented | HostAdmin/HostDev only; list installed projects and state. |
| `kernel.v1.project.get` | implemented | HostAdmin/HostDev only; return one project's full descriptor and registry record; includes `running_session_id` when running. |
| `kernel.v1.project.start` | implemented | HostAdmin/HostDev only; transition an Installed/Stopped project to Running, open a project session, return `session_id` and `already_running`, and emit lifecycle events. |
| `kernel.v1.project.stop` | implemented | HostAdmin/HostDev only; stop a Running project and emit lifecycle events. |
| `kernel.v1.project.status` | implemented | HostAdmin/HostDev only; return project state and last error; includes `running_session_id` when running. |

### `kernel.v1.host.*` (4)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.host.info` | implemented | Return protocol version, methods, statuses, and transport labels. |
| `kernel.v1.host.ping` | partial | Reserved lightweight health check. |
| `kernel.v1.host.diagnostics` | partial | Return local package/capability/hook diagnostics. |
| `kernel.v1.host.principal` | planned | Reserved identity-provider integration. |

### `kernel.v1.audit.*` (1)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.audit.package` | partial | Report declared vs used authority for `yg audit --package <id>`; actual-use tracking continues to expand. |

### Surface / extension point / hook (6)

| Method | Status | Contract |
|---|---:|---|
| `kernel.v1.surface.contribution.list` | partial | List typed package-declared surface contributions. |
| `kernel.v1.surface.contribution.describe` | partial | Describe one contribution. |
| `kernel.v1.surface.resolve_bundle` | partial | HostAdmin/HostDev only; resolve a mountable bundle URL from a surface contribution, project dev path, or installed project; cross-source parity continues to harden. |
| `kernel.v1.extension_point.list` | implemented | List extension points. |
| `kernel.v1.extension_point.describe` | planned | Describe one extension point. |
| `kernel.v1.hook.list` | partial | List hook subscriptions. |

## Event kind matrix (59)

The full registry is [`v1/EVENT_KIND_REGISTRY.md`](v1/EVENT_KIND_REGISTRY.en.md). Event payload schemas live under `docs/spec/v1/schemas/events/`.

| Group | Count | Examples |
|---|---:|---|
| session | 3 | `kernel/v1/session.opened`, `.closed`, `.forked` |
| package lifecycle | 9 | `loading`, `starting`, `ready`, `loaded`, `stopping`, `stopped`, `unloaded`, `degraded`, `log` |
| project lifecycle | 4 | `project.installed`, `.started`, `.stopped`, `.uninstalled` |
| capability lifecycle | 3 | `capability.invoked`, `.completed`, `.failed` |
| stream lifecycle | 7 | `stream.started`, `.chunk`, `.progress`, `.ended`, `.error`, `.cancelled`, `.timeout` |
| permissions | 3 | `permission.granted`, `.revoked`, `.denied` |
| proposals | 5 | `proposal.created`, `.approved`, `.rejected`, `.applied`, `.failed` |
| assets / projections | 2 | `asset.put`, `projection.updated` |
| outbound / websocket | 8 | `outbound.request`, `.denied`, completion events, websocket frames |
| exec | 6 | `exec.request`, `.started`, `.completed`, `.failed`, `.stopped`, `.denied` |
| port | 3 | `port.leased`, `.released`, `.denied` |
| proxy | 3 | `proxy.registered`, `.unregistered`, `.denied` |
| deployment | 2 | `deployment.reconciled`, `deployment.health` |
| error | 1 | `kernel/v1/error` |

Non-kernel event kinds must start with the writer package id followed by `/`. The kernel must reject package attempts to write `kernel/v1/...` or another package namespace.

## Capability handle model

Manifest strings are the **authority ceiling**. Runtime handles are the **actual authority**. A package cannot gain authority by forging strings; it must use handles minted by the kernel during load, handshake, or init.

- `kernel.v1.cap.attenuate(parent, constraints)` → child handle.
- `kernel.v1.cap.revoke(handle)` → immediately invalid.
- `kernel.v1.cap.list_for(package_id)` → all live handles currently held.

Handle fields:

- `id`: unforgeable kernel-minted identifier.
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
| `subprocess` | `package.handshake` receives/returns a `bindings` dictionary; SDK exposes `kernelClient` and handles. | implemented |
| `rust_inproc` | `KernelEnv` is passed to `InprocPackage::init` with runtime bindings. | implemented |
| `wasm` | WIT resource imports. | planned |
| `remote` | SPIFFE + Biscuit token exchange. | planned |

Bindings must contain only the caller's granted authority. Path B packages do not receive v1 capability bindings.

## Effect audit

`yg audit --package <id>` and `kernel.v1.audit.package` report declared vs used authority. Audit input comes from:

1. manifest permissions, capabilities, secret_refs, and network hosts;
2. kernel-minted and attenuated capability handles;
3. `capability.invoked|completed|failed` and outbound audit events;
4. permission grants/revokes and package lifecycle;
5. Path B's `contract_mode: "none"` marker.

Audit reports find unused declarations, undeclared use, authority expansion, expired-handle use, use after revoke, undeclared `secret_ref`s, and undeclared network targets. See the audit section in [`../guides/CAPABILITY_HANDLES.md`](../guides/CAPABILITY_HANDLES.en.md).

## Conformance kit

Third-party packages can run:

```bash
yg conformance package --contract v1 --path <package>
```

The kit has 8 acceptance checks: manifest parse, contract mode, entry support, bindings/handshake, capability declarations, permission declarations, audit visibility, and fixture invocation. It outputs PASS/FAIL/SKIP/WARNING and a compliance percentage. Path A packages must pass applicable checks; Path B packages skip capability/permission checks but must remain self-contained and lifecycle-observable.

See [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.en.md).

## SDK generation

`docs/spec/v1/schemas/` is the single source of truth. SDKs are available through three channels:

- npm: `@yggdrasil/kernel-sdk` (`sdk/typescript/kernel-sdk/`).
- workspace path: `file:../yggdrasil/sdk/typescript/kernel-sdk`.
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

All 161 schemas must pass `cargo run -p ygg-cli --bin validate-schemas`.

## Content-free invariant

The current kernel crates must not define or require content-shaped concepts such as `Turn`, `Message`, `PromptFrame`, `ModelCall`, `Agent`, `World`, `Scene`, `Director`, or `Memory`. Such concepts are owned by the relevant Protocol, Component, Product, or Client rather than the constitutional substrate; Contract V1 may carry them through Package-writer events, projections, and capabilities.

## Object contracts

### `KernelSession`

`KernelSession` is a content-free execution context. It may hold identity, labels, active package set, principal scope, status, timestamps, and metadata. It must not hold messages, turns, prompts, characters, worlds, memory, or model calls.

A session id only identifies current runtime ordering and permission scope. Protocols or Components may express domain state through the current Package writer's event payloads, objects, or projections, while the runtime treats them as opaque data and public descriptors.

### `EventEnvelope`

`EventEnvelope` is append-only fact storage. Each envelope contains at least session id, sequence, writer package id, kind, schema version, timestamp, payload, and metadata. Sequence is monotonic per session.

The kernel validates namespace, permission, and schema shape only. Event meaning belongs to the writer package.

### `PackageManifest`

A manifest declares package identity, entry, contract mode, provided capabilities, consumed capabilities, surface contributions, hooks, extension points, asset/schema declarations, permissions, and sandbox policy.

## Package dependencies (manifest.requires)

`requires` is the first-class package dependency declaration field in the manifest. It expresses which other packages must be resolved and installed for this package. It is distinct from `consumes`: `consumes` declares capability requirements, while `requires` declares package dependency data. It is not a protocol method and does not grant runtime authority; installers use it to resolve dependencies and write lockfiles, while runtime authorization still comes from permissions, bindings, and capability handles.

```yaml
requires:
  - id: official/model-provider-lab
    source:
      kind: git
      url: https://example.com/yggdrasil/model-provider-lab.git
      ref: v1.2.3
    version: "^1.2"
    minimum_signed_by:
      - "0123456789ABCDEF0123456789ABCDEF01234567"
```

Actual install and resolution are handled by `official/install-lab`; the kernel does not participate in dependency resolution.
See [`docs/guides/PACKAGE_INSTALLATION.md`](../guides/PACKAGE_INSTALLATION.en.md).

The manifest is audit and handle-minting input, not runtime authority. Runtime authority is expressed through bindings and capability handles.

### `PackageRecord`

`PackageRecord` tracks package id, version, entry kind, contract mode, trust level, state, manifest summary, capability/hook/surface counts, and state timestamps. Records support host diagnostics, package status, lifecycle audit, and the conformance kit.

### `CapabilityDescriptor`

A descriptor describes a provider-owned capability: id, version, input schema, output schema, streaming flag, side effects, description, and metadata. It does not grant call authority; call authority comes from the caller's handle.

### `HookSubscription`

Hook subscriptions come from manifests. The kernel owns ordering, unload cleanup, and event/capability lifecycle dispatch. Hook handlers still execute through ordinary capabilities and permission boundaries.

### `AssetRecord`

An asset record is opaque metadata: id, origin package, mime, hash, size, metadata. The kernel does not interpret asset content. Content-addressed blob storage and package-principal asset permissions remain later substrate work.

## Permission and denial semantics

Permission checks must fail closed. Missing handle, expired handle, revoked handle, scope mismatch, schema mismatch, host-policy denial, and missing manifest declaration all deny the call.

Denials should produce structured errors and, where applicable, audit events. Errors must not leak raw secrets, full request bodies, user content, or provider credentials.

Host-dev operations must be explicit in protocol context. Anonymous host calls must not become package privilege.

## Namespace rules

Protocol methods use `kernel.v1.<namespace>.<name>`. Kernel events use `kernel/v1/<kind>`. Package events must start with package id followed by `/`.

Reserved rules:

- `kernel.v1.*` methods belong only to the kernel.
- `kernel/v1/*` events are written only by the kernel.
- `kernel.v2.*` and `kernel/v2/*` are reserved for breaking changes.
- Packages must not declare capability ids that look like kernel namespaces.

Experimental canonical IDs and legacy aliases introduced by the layered migration are managed
centrally by [`CONTRACT_REGISTRY.md`](CONTRACT_REGISTRY.en.md). They do not remove or rename any
`kernel.v1.*` v1 entry point.

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

In-process packages do not gain official privilege. They still participate in v1 through `KernelEnv`, bindings, handles, schemas, and audit.

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

Store-backed references are resolved via the `StoreSecretResolver` against an age-encrypted file at `~/.yggdrasil/secrets.dat`. See [`docs/guides/SECRET_MANAGEMENT.md`](../guides/SECRET_MANAGEMENT.en.md).

Undeclared secret refs, resolution failure, resolver denial, and raw secrets in protected payloads must fail closed.

## Proposal contract

A proposal is an approval-gated change, not a content model. The kernel only manages lifecycle: create, approve, reject, apply, failed. Approval/apply boundaries enforce explicit authority, terminal transitions are compare-and-set guarded, and operation payload remains opaque JSON subject to raw-secret scanning and basic schema checks.

Current apply supports generic asset/projection operations. Broader transactions, compensation, and revert are later work.

## Surface contract

A surface contribution is a package-declared UI/UX entry descriptor. The kernel stores and lists descriptors; it does not render UI or interpret content semantics. The host shell decides how to mount iframe, bundle, or native surfaces.

Official and third-party surfaces use the same descriptors, permission declarations, and review path.

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

## Relationship to older docs

The old alpha contract has been replaced by this file. Long-term references should point to `KERNEL_V1_CONTRACT.md`. The registry, error codes, versioning, and schemas under `docs/spec/v1/` are machine-readable companions to this contract.

## Appendix A: method namespace counts

| Namespace | Count |
|---|---:|
| `kernel.v1.session.*` | 6 |
| `kernel.v1.event.*` | 3 |
| `kernel.v1.package.*` | 7 |
| `kernel.v1.capability.*` | 5 |
| `kernel.v1.cap.*` | 3 |
| `kernel.v1.permission.*` | 4 |
| `kernel.v1.proposal.*` | 6 |
| `kernel.v1.asset.*` | 3 |
| `kernel.v1.projection.*` | 4 |
| `kernel.v1.outbound.*` | 6 |
| `kernel.v1.target.*` | 4 |
| `kernel.v1.exec.*` | 5 |
| `kernel.v1.port.*` | 4 |
| `kernel.v1.proxy.*` | 4 |
| `kernel.v1.project.*` | 5 |
| `kernel.v1.host.*` | 4 |
| `kernel.v1.audit.*` | 1 |
| `kernel.v1.surface.*` | 3 |
| `kernel.v1.extension_point.*` | 2 |
| `kernel.v1.hook.*` | 1 |

## Appendix B: release checks

Before releasing a v1-compatible host, run:

```bash
cargo test -p ygg-core
cargo test -p ygg-runtime
cargo test -p ygg-cli
cargo run -p ygg-cli -- conformance
cargo run -p ygg-cli --bin export-schemas
cargo run -p ygg-cli --bin validate-schemas
cargo run -p ygg-cli --bin generate-sdks
```

Also run package conformance against representative Path A and Path B examples.

## Appendix C: non-goals

v1 does not promise:

- chat, agent, model, world, memory, or director semantics in the kernel;
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
