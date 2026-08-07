# Public Protocol v0

> [English](./PROTOCOL_V0.en.md) · [中文](./PROTOCOL_V0.md)

Yggdrasil currently exposes substrate, Host, Protocol, and Shell Profile capability through one public contract. Official Web/Desktop, CLI, in-process Components, subprocesses, future WASM Components, and remote services share the same identity, authority, and behavioral semantics.

There is no private bypass. Official clients use this protocol; third parties use this protocol.

This document retains `kernel.v1.*` compatibility names for the current wire surface; those names do not determine permanent architectural ownership. See [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.en.md) for itemized owners.

## Transports

All transports eventually surface the same public behavior. The current Host implements an operational subset first; additional transports open only after identity, authority, error, cancellation, and terminal semantics are clear.

- In-process: a Rust API that mirrors the wire shape one-to-one.
- Subprocess: JSON-RPC over stdio. Required for the current host.
- HTTP: request/response for non-streaming methods. Required for the current host.
- Profile-backed HTTP host: `ygg host serve --http 127.0.0.1:8787 --profile profiles/forge-alpha.yaml` starts `/rpc` plus ad hoc SSE routes after autoloading profile packages.
- Host stdio: JSON-RPC for automation and conformance. Required for the current host.
- WebSocket: subscriptions and streaming methods. Planned after sequence-range replay.
- TCP: JSON-RPC over a local socket. Deferred.
- Remote endpoint: HTTP and WebSocket against a declared URL. Deferred.
- WASM host: marshalled calls into the kernel-provided ABI. Deferred.

Transport selection is a Host concern. Status documentation marks a method implemented only when a public transport path is operational, a behavioral check exists, and runtime authority is not bypassed.

## Protocol envelope

Canonical request/response transports use this shape:

```json
{
  "id": "request-1",
  "method": "kernel.v1.capability.invoke",
  "params": {}
}
```

The host attaches principal and transport context. Callers cannot self-assert package/admin identity through request JSON.

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
    "code": "kernel/v1/error/permission_denied",
    "message": "...",
    "details": {}
  }
}
```

## Method shape

Every method has:

- `id`: namespaced under `kernel/v1/...` for kernel methods, or under a package id for package methods.
- `input`: a JSON value validated against a published schema.
- `output`: a JSON value, possibly a stream.
- `errors`: a structured error model with `code`, `message`, `details`.

## Kernel methods

The kernel exposes a minimal set. Anything not listed is owned by a package.

### Sessions

```text
kernel.v1.session.open      open a session with labels and a package set
kernel.v1.session.close     close a session
kernel.v1.session.fork      fork a session at an event sequence
kernel.v1.session.branch.list list branch lineage records
kernel.v1.session.get       get session metadata
kernel.v1.session.list      list sessions visible to the caller
```

The kernel stores no content-level session state. Labels and package set are the only opinions.

### Events

```text
kernel.v1.event.append      append an event under the caller's namespace
kernel.v1.event.list        list events for a session by sequence range
kernel.v1.event.subscribe   stream events as they are appended (resumable)
```

`event.append` requires `events.append` in the caller's manifest. `event.list` and `event.subscribe` require `events.read` for package principals. The current host exposes HTTP SSE as a host-dev stream:

```text
GET /kernel/v1/event.subscribe/:session_id?after_sequence=42&kind_prefix=kernel/v1/&writer_package_id=kernel
```

`kernel.v1.event.list` accepts `session_id`, `after_sequence`, `limit`, `kind_prefix`, and `writer_package_id`.

### Packages

```text
kernel.v1.package.list      list packages visible in the host
kernel.v1.package.describe  fetch a manifest snapshot
kernel.v1.package.load      load a package from a manifest reference
kernel.v1.package.unload    stop and remove a package
kernel.v1.package.status    current state and health
kernel.v1.package.restart   restart a package when its entry form supports restart
kernel.v1.package.logs      read captured package logs
```

Loading a package may be host-policy-restricted.

### Capabilities

```text
kernel.v1.capability.discover    enumerate capabilities, optionally filtered
kernel.v1.capability.describe    fetch input/output schemas and metadata
kernel.v1.capability.invoke      invoke a capability with input
kernel.v1.capability.stream      invoke a capability that streams
kernel.v1.capability.cancel      cancel an in-flight invocation
```

`invoke` resolves to a provider by id, optional `provider_package_id`, optional version constraint, and eventually session package set. If multiple providers match and the caller did not specify `provider_package_id`, the kernel returns an ambiguous-route error. The current host supports exact version or same-major `^x.y` constraints.

### Extension points and hooks

```text
kernel.v1.extension_point.list        list live extension points
kernel.v1.extension_point.describe    fetch payload schema and timing
kernel.v1.hook.list                   list subscribers to a point
```

The kernel does not expose a method to inject hooks at runtime. Subscriptions are declared in manifests. Live registration is allowed only through package lifecycle.

### Assets

```text
kernel.v1.asset.put         store an asset blob under the caller's namespace
kernel.v1.asset.get         fetch an asset by id
kernel.v1.asset.list        list assets visible to the caller
```

The kernel records `mime`, `hash`, `size`, and `origin_package`. It does not parse or interpret asset content.

### Projections

```text
kernel.v1.projection.register  register a generic projection definition
kernel.v1.projection.rebuild   rebuild projection state from event filters
kernel.v1.projection.get       fetch projection state
kernel.v1.projection.list      list projection records
```

The current runtime manages projection records and rebuild lifecycle without interpreting domain-state meaning. Shared Projection contracts belong to optional Protocols, concrete materializers are implemented by Components, and Contract V1 registers and distributes them through Package writers.

### Health and identity

```text
kernel.v1.host.info         host version, kernel ABI, transports
kernel.v1.host.principal    the calling principal (user, package, remote)
kernel.v1.host.ping         liveness
kernel.v1.host.diagnostics  local host diagnostics for package/capability/hook observability
```

### Outbound

```text
kernel.v1.outbound.execute    unary HTTP-style outbound through the host executor
kernel.v1.outbound.stream     streaming outbound through SSE / NDJSON / raw frames
kernel.v1.outbound.websocket.open   open an outbound WebSocket stream and return connection_id
kernel.v1.outbound.websocket.send   send one outbound WebSocket frame
kernel.v1.outbound.websocket.close  close an outbound WebSocket connection
kernel.v1.outbound.audit      list redacted outbound audit records for a package
```

The outbound protocol has three outbound primitives: `execute` is a unary HTTP-style request, `stream` is an SSE / NDJSON / raw one-way stream, and `kernel.v1.outbound.websocket.*` is bidirectional WebSocket. `websocket.open` is a streaming method that establishes a WSS connection and returns `connection_id`; `websocket.send` and `websocket.close` are unary methods. `connection_id` is also the `stream_id`; passing it to `kernel.v1.capability.cancel` uses the same cancel/close path.

Request/response shapes are defined by runtime types and protocol dispatch parsing, not repeated in full here: HTTP/stream types live in `crates/ygg-runtime/src/runtime/outbound.rs`, WebSocket types live in `crates/ygg-runtime/src/runtime/outbound_websocket.rs`, and protocol parsing lives in `crates/ygg-runtime/src/runtime/protocol_dispatch.rs`. Core fields include `capability_id`, `destination_host`, `method`, optional `path`, `body_shape`, `metadata`, `secret_headers`, `static_headers`, and `timeout_ms`; `stream` also accepts `stream_format` (`sse` / `ndjson` / `raw`) and frame/duration limits; `websocket.open` accepts destination host/path, optional subprotocols, headers, `secret_refs`, and connection/frame/byte limits.

Outbound requests pass two fail-closed gates: the package manifest must declare matching `permissions.network.declarations` (WebSocket uses the `WEBSOCKET` method), and every `secret_headers` / `secret_refs` entry must be declared in `permissions.secret_refs`. The host profile must also explicitly enable the relevant outbound primitive, the destination host must match the allowlist by equality (or `*.suffix`), HTTP/SSE use HTTPS-only, WebSocket defaults to WSS-only, and redirects are rejected by default. `capability_id` must be in the caller package namespace; subprocess reverse kernel calls use the host-bound package principal and cannot spoof another package.

WebSocket-specific events use `kernel/v1/outbound.websocket.*`: `opened` records handshake success and connection/subprotocol metadata; `frame` records inbound/outbound direction, frame kind, byte count, and sequence number without payload; `error` records a redacted error; `completed` records close code, reason, frame/byte counts, duration, executor kind, network_performed, redaction state, and secret_ref references.

All three outbound primitives emit completion audit events: `kernel/v1/outbound.execute.completed`, `kernel/v1/outbound.stream.completed`, and `kernel/v1/outbound.websocket.completed`. These events record only status, counts, duration, executor kind, network_performed, redaction state, and `secret_ref` references; they do not record raw headers, bodies, secrets, frame payloads, or responses.

`kernel.v1.outbound.audit` returns only redacted audit records: package, capability, destination host, method, purpose, used `secret_ref`s, and redaction state. Raw headers, bodies, secrets, and responses are not written to audit or protocol responses.

Git installation is not a kernel transport. Future `yg install <github-url>` support will be implemented by an ordinary capability package using `kernel.v1.outbound.execute` and filesystem write permission, not by a kernel git fetch method.

## Package methods

Each package contributes its own protocol methods through capability registrations and extension-point declarations. Their schemas are discoverable via `kernel.v1.capability.describe` and `kernel.v1.extension_point.describe`.

The kernel does not predefine methods like `session.input`, `prompt_frame.get`, `model.call`, `memory.search`. If those exist, they belong to specific packages.

## Errors

```text
kernel/v1/error/transport
kernel/v1/error/schema_validation
kernel/v1/error/manifest
kernel/v1/error/permission_denied
kernel/v1/error/ambiguous_route
kernel/v1/error/not_found
kernel/v1/error/timeout
kernel/v1/error/cancelled
kernel/v1/error/capacity
kernel/v1/error/package_state
```

Package errors travel inside `capability.invoke` responses as `package_error` with provider-defined details.

## Streaming

Streaming flows over WebSocket or an equivalent transport. Streams carry typed frames whose schema is published with the method.

For `event.subscribe`, frames are event envelopes plus a `cursor` for resume.

For `capability.stream`, frames are provider-defined chunks plus a terminal status frame.

## Authentication and principals

A host enforces authentication at the transport layer. Each connection is associated with a principal: a user, assistant, package, host tool, anonymous caller, or remote system. The kernel checks permissions against the principal on every operation.

The kernel does not ship an identity provider. Hosts plug one in.

The v0 principal model includes:

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
kernel.v1.permission.grant
kernel.v1.permission.revoke
kernel.v1.permission.list
kernel.v1.permission.audit
```

## Surface contributions

Packages may declare UI surface descriptors in their manifests. The kernel does not render or interpret these descriptors as content; it only exposes them for public clients:

```text
kernel.v1.surface.contribution.list
kernel.v1.surface.contribution.describe
```

Current slots are `experience_entry`, `home_card`, `quick_action`, `workshop_card`, `play_renderer`, `forge_panel`, `asset_editor`, and `assistant_action`.

`quick_action`, `workshop_card`, and `home_card` entries with `metadata.shell_schema_version: 1` are structured shell descriptors. They allow only bounded text, icon hints, order, and same-package targets. The web shell renders these entries itself. It does not load package JavaScript, parse HTML, or mount iframes for them. Package-contributed actions are discovery affordances today; if they become executable later, they still have to cross the public protocol, permission, proposal, and audit boundaries.

Surface descriptors may include a version, launch capability, session template, input schema, permission UX metadata, and an approval policy. They remain descriptors; the kernel does not turn them into built-in experience/game semantics.

## Proposal lifecycle

Assistant and package-driven changes use generic proposal envelopes instead of privileged mutation paths:

```text
kernel.v1.proposal.create
kernel.v1.proposal.get
kernel.v1.proposal.list
kernel.v1.proposal.approve
kernel.v1.proposal.reject
kernel.v1.proposal.apply
```

Proposal statuses are `created`, `approved`, `rejected`, `applied`, and `failed`. Initial operation support is intentionally generic, such as `asset.put` and `projection.rebuild`. These operations must produce kernel audit/proposal events.

## Versioning

The protocol carries `protocol_version`. The kernel publishes the schema set per version. Breaking changes require a new version; the kernel may serve multiple concurrently.

Method schemas may evolve in backward-compatible ways within a version (additive fields). Breaking method changes require a new method id.

## Stability

Anything resembling `session.input`, `prompt_frame.get`, `model.call`, or any other content method is out of scope for kernel protocol forever. Adding such methods to the kernel is a charter violation.
