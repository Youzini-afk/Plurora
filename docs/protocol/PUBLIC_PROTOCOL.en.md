# Public Protocol

> [English](./PUBLIC_PROTOCOL.en.md) · [中文](./PUBLIC_PROTOCOL.md)

Plurora exposes Substrate, Host, Protocol, and Shell Profile capability through one public contract. First-party Web/Desktop, CLI, in-process Components, subprocesses, future WASM Components, and remote services share the same identity, authority, and behavioral semantics.

There is no private bypass. First-party clients and third parties use the same protocol.

Every public method has one owner-based wire ID. The first segment identifies Substrate, Host, Protocol, or Shell ownership; see [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) for the itemized boundary.

## Transports

All transports eventually surface the same public behavior. The current Host implements an operational subset first; additional transports open only after identity, authority, error, cancellation, and terminal semantics are clear.

- In-process: a Rust API that mirrors the wire shape one-to-one.
- Subprocess: JSON-RPC over stdio. Required for the current host.
- HTTP: request/response for non-streaming methods. Required for the current host.
- Profile-backed HTTP Host: `plurora host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml` starts `/rpc` plus the public Host SSE routes after autoloading profile Packages.
- Host stdio: JSON-RPC for automation and conformance. Required for the current host.
- WebSocket: subscriptions and streaming methods. Planned after sequence-range replay.
- TCP: JSON-RPC over a local socket. Deferred.
- Remote endpoint: HTTP and WebSocket against a declared URL. Deferred.
- WASM Host: marshalled calls into a runtime-provided Component ABI. Deferred.

Transport selection is a Host concern. Status documentation marks a method implemented only when a public transport path is operational, a behavioral check exists, and runtime authority is not bypassed.

## Protocol envelope

Canonical request/response transports use this shape:

```json
{
  "id": "request-1",
  "method": "capability.invoke",
  "params": {}
}
```

The Host attaches principal and transport context. Callers cannot self-assert Package/admin identity through request JSON. Requests may also carry optional `session_id` and `contract` selection fields; omitted contract selection uses `plurora.contract.default/v1`.

Success:

```json
{
  "id": "request-1",
  "result": {}
}
```

Failure:

```json
{
  "id": "request-1",
  "error": {
    "code": "runtime/error/permission_denied",
    "message": "...",
    "details": {}
  }
}
```

## Method shape

Every method has:

- `id`: an exact owner-based public method ID such as `context.open`, `host.installation.list`, or `shell.contribution.list`.
- `input`: a JSON value validated against a published schema.
- `output`: a JSON value, possibly a stream.
- `errors`: a structured error model with `code`, `message`, `details`.

## Public methods

The public contract exposes a bounded method set. Package-specific behavior remains Package-owned capabilities.

### Contexts

```text
context.open      open a Context with labels and an active Package set
context.close     close a Context
context.fork      fork a Context at an event sequence
context.branch.list list branch lineage records
context.get       get Context metadata
context.list      list Contexts visible to the caller
```

The substrate stores no content-level Context state. Labels, active Package set, lineage, journal ordering, and authority scope are the bounded platform concerns.

### Events

```text
journal.append      append an event under the caller's namespace
journal.list        list events for a session by sequence range
journal.subscribe   stream events as they are appended (resumable)
```

`journal.append` requires `events.append` in the caller's manifest. `journal.list` and `journal.subscribe` require `events.read` for Package principals. The current Host exposes HTTP SSE as a Host-dev stream:

```text
GET /journal/subscribe/:session_id?after_sequence=42&kind_prefix=host/&writer_package_id=plurora/runtime
```

`journal.list` accepts `session_id`, `after_sequence`, `limit`, `kind_prefix`, and `writer_package_id`.

### Packages

```text
host.package.list      list packages visible in the host
host.package.describe  fetch a manifest snapshot
host.package.load      load a package from a manifest reference
host.package.unload    stop and remove a package
host.package.status    current state and health
host.package.restart   restart a package when its entry form supports restart
host.package.logs      read captured package logs
```

Loading a package may be host-policy-restricted.

### Capabilities

```text
capability.discover    enumerate capabilities, optionally filtered
capability.describe    fetch input/output schemas and metadata
capability.invoke      invoke a capability with input
capability.stream      invoke a capability that streams
capability.cancel      cancel an in-flight invocation
```

`invoke` resolves to a provider by capability ID, optional `provider_package_id`, optional version constraint, and the active Context Package set. If multiple providers match and the caller did not specify `provider_package_id`, the runtime returns an ambiguous-route error. The current Host supports exact versions or same-major `^x.y` constraints.

### Extension points and hooks

```text
protocol.extension.list        list live extension points
protocol.extension.describe    fetch payload schema and timing
protocol.hook.list                   list subscribers to a point
```

The public contract does not expose arbitrary hook injection. Subscriptions are declared in Manifests and enter the live registry through Package lifecycle. The runtime currently dispatches only the four documented journal/capability points.

### Objects

```text
object.put         store an asset blob under the caller's namespace
object.get         fetch an asset by id
object.list        list assets visible to the caller
```

The runtime records object metadata such as `mime`, `hash`, `size`, and `origin_package`. It verifies storage boundaries but does not interpret content.

### Projections

```text
projection.register  register a generic projection definition
projection.rebuild   rebuild projection state from event filters
projection.get       fetch projection state
projection.list      list projection records
```

The current runtime manages projection records and rebuild lifecycle without interpreting domain-state meaning. Shared Projection contracts belong to optional Protocols, concrete materializers are implemented by Components, and Contract V1 registers and distributes them through Package writers.

### Health and identity

```text
host.info         protocol/registry versions, methods, profiles, layers, Protocol Commons, and transports
identity.current    the calling principal (user, package, remote)
host.ping         liveness
host.diagnostics  local host diagnostics for package/capability/hook observability
```

### Outbound

```text
host.outbound.execute    unary HTTP-style outbound through the host executor
host.outbound.stream     streaming outbound through SSE / NDJSON / raw frames
host.outbound.websocket.open   open an outbound WebSocket stream and return connection_id
host.outbound.websocket.send   send one outbound WebSocket frame
host.outbound.websocket.close  close an outbound WebSocket connection
host.outbound.audit      list redacted outbound audit records for a package
```

The outbound protocol has three outbound primitives: `execute` is a unary HTTP-style request, `stream` is an SSE / NDJSON / raw one-way stream, and `host.outbound.websocket.*` is bidirectional WebSocket. `websocket.open` is a streaming method that establishes a WSS connection and returns `connection_id`; `websocket.send` and `websocket.close` are unary methods. `connection_id` is also the `stream_id`; passing it to `capability.cancel` uses the same cancel/close path.

Request/response shapes are defined by runtime types and protocol dispatch parsing, not repeated in full here: HTTP/stream types live in `crates/plurora-runtime/src/runtime/outbound.rs`, WebSocket types live in `crates/plurora-runtime/src/runtime/outbound_websocket.rs`, and protocol parsing lives in `crates/plurora-runtime/src/runtime/protocol_dispatch.rs`. Core fields include `capability_id`, `destination_host`, `method`, optional `path`, `body_shape`, `metadata`, `secret_headers`, `static_headers`, and `timeout_ms`; `stream` also accepts `stream_format` (`sse` / `ndjson` / `raw`) and frame/duration limits; `websocket.open` accepts destination host/path, optional subprotocols, headers, `secret_refs`, and connection/frame/byte limits.

Outbound requests pass two fail-closed gates: the Package Manifest must declare matching `permissions.network.declarations` (WebSocket uses the `WEBSOCKET` method), and every `secret_headers` / `secret_refs` entry must be declared in `permissions.secret_refs`. The Host profile must explicitly enable the relevant outbound primitive; the destination must match the allowlist by equality or `*.suffix`; HTTP/SSE require HTTPS; WebSocket defaults to WSS; redirects are rejected by default. `capability_id` must be in the caller Package namespace. Subprocess reverse public calls use the Host-bound Package principal and cannot spoof another Package.

WebSocket-specific events use `host/outbound.websocket.*`: `opened` records handshake success and connection/subprotocol metadata; `frame` records inbound/outbound direction, frame kind, byte count, and sequence number without payload; `error` records a redacted error; `completed` records close code, reason, frame/byte counts, duration, executor kind, network_performed, redaction state, and secret_ref references.

All three outbound primitives emit completion audit events: `host/outbound.execute.completed`, `host/outbound.stream.completed`, and `host/outbound.websocket.completed`. These events record only status, counts, duration, executor kind, network_performed, redaction state, and `secret_ref` references; they do not record raw headers, bodies, secrets, frame payloads, or responses.

`host.outbound.audit` returns only redacted audit records: package, capability, destination host, method, purpose, used `secret_ref`s, and redaction state. Raw headers, bodies, secrets, and responses are not written to audit or protocol responses.

Git installation is not a public-contract transport. The current `plurora install <github-url>` path composes ordinary first-party Packages including `plurora/git-tools-lab`, `plurora/integrity-lab`, and `plurora/install-lab`, under declared network/filesystem authority; it does not add a substrate Git method.

## Package methods

Each Package contributes Package-owned capabilities and may declare extension contracts. Capability descriptors are discoverable through `capability.discover`, while schemas are published by Manifests and generated contract artifacts. `capability.describe` and `protocol.extension.describe` remain reserved planned queries.

The public contract does not predefine content methods such as `session.input`, `prompt_frame.get`, `model.call`, or `memory.search`. Such behavior belongs to explicit Protocols or Package-owned capabilities.

## Errors

```text
runtime/error/internal
runtime/error/invalid_request
runtime/error/permission_denied
runtime/error/not_found
runtime/error/ambiguous_route
runtime/error/schema_invalid
runtime/error/package_state
protocol/error/unsupported_contract
```

Provider failures travel inside capability results or the common `ProtocolError` envelope according to the method schema. The stable string identifiers and numeric JSON-RPC equivalents are listed in [`../spec/v1/ERROR_CODES.md`](../spec/v1/ERROR_CODES.en.md).

## Streaming

Streaming flows over WebSocket or an equivalent transport. Streams carry typed frames whose schema is published with the method.

For `journal.subscribe`, frames are event envelopes plus a `cursor` for resume.

For `capability.stream`, frames are provider-defined chunks plus a terminal status frame.

## Authentication and principals

A Host enforces authentication at the transport layer. Each connection is associated with a principal: a user, assistant, Package, Host tool, anonymous caller, or remote system. The runtime checks authority against the Host-established principal on every operation.

The substrate does not prescribe an identity provider. Hosts integrate one and map authenticated identities to public principals.

The current v1 principal classes include:

```text
host_admin
host_dev
package { package_id }
human { user_id }
assistant { assistant_id, delegated_user_id? }
anonymous
```

Human and assistant principals require explicit scoped grants for sensitive operations:

```text
authority.grant.create
authority.grant.revoke
authority.grant.list
authority.decision.list
```

## Surface contributions

Packages may declare UI surface descriptors in their Manifests. The runtime does not render or interpret these descriptors as content; it exposes them through the Shell Profile methods for public clients:

```text
shell.contribution.list
shell.contribution.describe
```

Current slots are `experience_entry`, `home_card`, `quick_action`, `workshop_card`, `play_renderer`, `forge_panel`, `asset_editor`, and `assistant_action`.

`quick_action`, `workshop_card`, and `home_card` entries with `metadata.shell_schema_version: 1` are structured shell descriptors. They allow only bounded text, icon hints, order, and same-package targets. The web shell renders these entries itself. It does not load package JavaScript, parse HTML, or mount iframes for them. Package-contributed actions are discovery affordances today; if they become executable later, they still have to cross the public protocol, permission, proposal, and audit boundaries.

Surface descriptors may include a version, launch capability, Context template, input schema, permission UX metadata, and an approval policy. They remain descriptors; the runtime does not turn them into built-in experience/game semantics.

## Proposal lifecycle

Assistant and package-driven changes use generic proposal envelopes instead of privileged mutation paths:

```text
change.proposal.create
change.proposal.get
change.proposal.list
change.proposal.approve
change.proposal.reject
change.proposal.apply
```

Proposal statuses are `created`, `approved`, `rejected`, `applied`, and `failed`. Initial operation support remains generic, such as `object.put` and `projection.rebuild`. Operations produce registered Change/Object/Projection events and durable effect evidence.

## Versioning

The request envelope may carry explicit `contract` selection. `host.info` publishes the supported registry, profiles, layer versions, methods, and Protocol Commons descriptors. The current Host supports the exact v1 boundary and fails closed on unsupported selections.

Method schemas may evolve additively within v1. Breaking changes require a new explicit contract/profile/layer version boundary, migration tooling, readable prior data, and conformance vectors; they are not hidden behind aliases.

## Stability

Content-specific methods such as `session.input`, `prompt_frame.get`, or `model.call` are outside the constitutional substrate. Adding them as universal public-contract ontology without an adopted Protocol and owner is a Charter violation.
