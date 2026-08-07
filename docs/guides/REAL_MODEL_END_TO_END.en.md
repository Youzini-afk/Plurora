# Real Model End-to-End Calls

> [English](./REAL_MODEL_END_TO_END.en.md) · [中文](./REAL_MODEL_END_TO_END.md)

This guide follows the real path from a user pressing Send in the YdlTavern surface to an OpenAI/Anthropic/Gemini API response appearing back on screen. The path goes through the public protocol, a project session, package permissions, `secret_ref` resolution, and the host outbound executor.

This is not a kernel-level model API. Model semantics belong to the YdlTavern engine package. The kernel provides sessions, permissions, capability invocation, outbound execution, and audit boundaries.

## Complete call chain

```text
User types a message in the YdlTavern surface + clicks Send
  ↓
SendForm.onSend(text)
  ↓
TavernShell → TavernProvider.sendMessage(text)
  ↓ (local: add user message to chat state)
  ↓
invokeCapability("ydltavern/engine/model.live_call", { ... })
  ↓ (postMessage to the surface-host iframe parent)
  ↓
clients/web main thread receives RPC and calls client.invokeWithSession(method, params, sessionId)
  ↓ (HTTP POST /rpc with session_id)
  ↓
plurora host serve routes to dispatch_capability_invoke
  ↓ (sets ProtocolContext.session_id and ProtocolContext.principal=Package)
  ↓
inproc dispatcher finds the ydltavern-engine package (subprocess)
  ↓ (subprocess JSON-RPC call)
  ↓
ydltavern-engine runs the capability handler
  ↓ (builds an OpenAI/Anthropic/Gemini-shaped request)
  ↓
reverse-calls host.outbound.execute with secret_headers: {Authorization: secret_ref}
  ↓
host dispatch_outbound_execute handles:
  ✓ checks the package permissions.network.declarations allow this host
  ✓ checks the package permissions.secret_refs declare this ref
  ✓ Runtime::resolve_secret_ref_with_session resolves ref → real value
    ├─ through CompositeSecretResolver
    ├─ secret_ref:store:* → StoreSecretResolver → decrypt ~/.plurora/secrets.dat
    ├─ secret_ref:project:* → ProjectStoreSecretResolver
    │   ├─ uses ACTIVE_PROJECT_SCOPE task-local to read session.metadata.project_id
    │   ├─ decrypts ~/.plurora/projects/<id>/secrets.dat
    │   ├─ missing + fallback_to_platform default true → platform store
    │   └─ missing + fallback disabled → fail closed
    └─ secret_ref:env:* → EnvSecretResolver (allowlist)
  ↓
LiveHttpOutboundExecutor builds an HTTPS request and injects headers
  ↓
Real HTTPS call to api.openai.com / api.anthropic.com / etc.
  ↓
Response flows back up the reverse path until the surface receives a string
  ↓
TavernProvider.sendMessage parses text with extractContentFromResult
  ↓
Assistant message content updates → React re-renders → user sees the reply
```

## Streaming call chain (`settings.streaming = true`)

The non-streaming path returns one complete response. The streaming path pushes chunks incrementally to the surface through the kernel event stream.

```text
SendForm onSend(text)
  ↓
TavernProvider.sendMessage(text) (settings.streaming === true)
  ↓
streamCapability("ydltavern/engine/model.live_call.stream", { ... })
  ├─ Step 1: callHostRpc("capability.stream", { capability_id, input })
  │          → returns stream_id
  ├─ Step 2: postMessage to host: { type: "stream.subscribe", id, stream_id, session_id }
  └─ returns StreamHandle { streamId, frames: AsyncIterable<StreamFrame>, cancel() }
  ↓
host (surface-host.ts) receives stream.subscribe:
  ✓ subscribes to SSE through hostBridge.subscribeEvents(session_id, callback)
  ✓ filters capability/stream.* events and matches payload.stream_id
  ✓ forwards postMessage { type: "stream.frame" / "stream.ended" / "stream.error" }
  ↓
engine inside the subprocess:
  ✓ reverse-calls host.outbound.stream, not .execute
  ✓ parses SSE / chunked JSON
  ✓ normalizes frames such as { delta_text, kind: "chunk" }
  ✓ writes frames through the kernel into the session event stream (capability/stream.chunk)
  ↓
host SSE pushes those events into the surface-host subscribeEvents callback
  ↓
surface-host converts them into postMessage events for the iframe
  ↓
TavernProvider consumes frames in a for-await loop:
  - "started" / "progress": ignore
  - "chunk": extractStreamChunkDelta(frame.payload) → append to assistant message
  - "ended" / "final": mark streaming: false and exit the loop
  - "error" / "cancelled" / "timeout": show partial content or an error
  ↓
React re-renders and the user sees token-by-token streaming output
```

## Cancelling generation

The user clicks Stop:

```text
SendForm "Stop" button onClick
  ↓
tavern.cancelGeneration()
  ↓
activeStreamRef.current.cancel()
  ├─ callHostRpc("capability.cancel", { stream_id })
  ├─ postMessage { type: "stream.unsubscribe", subscription_id }
  └─ closes AsyncQueue and removes event listeners
  ↓
host:
  ✓ capability.cancel cancels the engine reverse call (engine receives abort signal)
  ✓ capability/stream.cancelled is written into the session
  ✓ surface-host sees cancelled and forwards stream.error to the iframe
  ↓
TavernProvider exits the loop, keeps accumulated content, and sets isGenerating: false
```

## Only one active generation at a time

Current behavior: when `isGenerating` is true, `sendMessage` returns immediately. The user must click Stop or wait for completion before sending a new message.

Queueing may come later, but it is out of scope for v1.

## Configuring real calls (user view)

Install the YdlTavern project, then start the Plurora host and web shell:

```bash
# 1. Install the local native project into a test data dir/profile
plurora install ../YdlTavern --data-dir <data-dir> --profile <profile> -y

# 2. Start the host with the installed profile
plurora host serve --profile <data-dir>/profiles/<profile>.yaml --http 127.0.0.1:8787 &

# 3. Start clients/web
npm run dev --prefix clients/web

# 4. Open http://localhost:1420 in a browser
```

Then in the UI:

1. The Home screen shows the installed YdlTavern project card.
2. Click Play.
3. The project becomes Running.
4. The host creates a project session.
5. `host.surface.bundle.resolve` validates the project and returns a short-lived `/surface-assets/<lease>/projects/<project_id>/bundle.mjs`; the surface bundle mounts in the iframe while the raw internal `/surface-bundles/...` path is not exposed to the sandbox.
6. Open the API Connections drawer.
7. Choose an OpenAI / Anthropic / Gemini provider.
8. Paste the API key.
9. Choose a save scope: Platform-wide (default) or This project only.
10. Click Save.
11. Close the drawer.
12. Type a message.
13. Click Send.
14. A real provider response should appear in the chat.

Platform-wide saves as `secret_ref:store:*`. This project only saves as `secret_ref:project:*` and is preferred only for the current project.

## Configuring real calls (developer view)

The host profile must explicitly enable resolvers and live outbound. Example:

```yaml
# profiles/forge-with-live-models.yaml
secret_resolver:
  store_enabled: true              # resolves secret_ref:store:* / project:*
  env_allowlist:                   # allowlist for secret_ref:env:*
    - OPENAI_API_KEY
    - ANTHROPIC_API_KEY
    - GEMINI_API_KEY

outbound:
  execute:
    enabled: true
    https_only: true
    executor: live                 # real HTTPS
    allowed_hosts:
      - api.openai.com
      - api.anthropic.com
      - generativelanguage.googleapis.com
      # Add OpenRouter / DeepSeek / xAI / Fireworks as needed.

surface_dev_paths:
  ydltavern: ../YdlTavern/packages/ydltavern-surface/dist
```

`surface_dev_paths` is only for development-time mounting of local build output. Installed projects do not need it; the host serves project dist files under `/surface-bundles/projects/<project_id>/...`. The web dev server port is `localhost:1420`. CLI `plurora project start/status/stop` commands are project-state commands; Home's Play/session flow uses the Web public protocol to call `host.project.start`, receive a `session_id`, resolve the surface, and mount it. Do not treat the CLI command path and the Web Play/session flow as equivalent entrypoints.

Three gates must all pass:

1. the profile permits the outbound executor to use the network;
2. the engine package manifest declares the target host;
3. the engine package manifest declares the `secret_ref` it uses.

Any missing gate fails closed. Default conformance does not use the network.

## Semantics of the three `secret_ref` forms

| Ref shape | Resolution path | Use case |
|---|---|---|
| `secret_ref:env:NAME` | `EnvSecretResolver` (allowlist) | Development / CI / Docker |
| `secret_ref:store:NAME` | `StoreSecretResolver` (local encrypted store) | Desktop users, platform-wide sharing |
| `secret_ref:project:NAME` | `ProjectStoreSecretResolver` | Project isolation with optional platform fallback |

See [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md).

## Where `session_id` comes from

Each running project has a kernel session, created by the host during `project.start`:

```text
session.id = ksess_xxx
session.metadata.project_id = "youzini-afk__YdlTavern__d2a47e5c"
session.labels = ["project:youzini-afk__YdlTavern__d2a47e5c"]
```

The `clients/web` main thread receives `session_id` from `host.project.start`. It then calls `host.surface.bundle.resolve` to get the surface bundle URL and uses `mountSurface` to create the iframe.

The iframe `initialProps` include:

```json
{
  "projectId": "youzini-afk__YdlTavern__d2a47e5c",
  "sessionId": "ksess_xxx"
}
```

Inside the surface, `callHostRpc` / `invokeCapability` automatically carries this `session_id`. When the host receives an RPC with `session_id`, it sets `ProtocolContext.session_id` and carries it to outbound dispatch.

There, the runtime reads `project_id` from session metadata, sets the `ACTIVE_PROJECT_SCOPE` task-local, and resolves `secret_ref:project:*` within that scope.

See [`PROJECT_MODEL.md`](PROJECT_MODEL.en.md).

## How project scope affects secrets

Project scope is not decided by a string the surface claims. It is decided by the session the host created:

1. `project.start` creates or reuses a project session.
2. Session metadata stores `project_id`.
3. Later RPCs carry `session_id`.
4. `dispatch_outbound_execute` finds the session from `ProtocolContext.session_id`.
5. The runtime sets `ProjectScopeContext`.
6. `ProjectStoreSecretResolver` reads the matching project store.

This prevents a surface from reading another project's secrets by forging `projectId`. The current model is still soft isolation; stronger multi-tenant project identity in `ProtocolContext` is planned.

## Permission and audit boundaries

A real model call must pass all of these boundaries:

- `capability.invoke` checks caller context and capability handles.
- The engine package manifest declares `ydltavern/engine/model.live_call`.
- The engine package manifest declares `permissions.network.declarations`.
- The engine package manifest declares `permissions.secret_refs`.
- The host profile enables a live executor and allowlists the target host.
- The secret resolver successfully resolves the reference.

Audit records store only the target host, method, package/capability, redaction state, executor kind, `secret_ref` references, and related metadata. They do not store raw API keys, prompt bodies, or provider responses.

## Troubleshooting

### `no project resolver configured`

The host profile has `secret_resolver.store_enabled: false`, but the user attempted `secret_ref:project:*`. Set it to true or use `secret_ref:env:*`.

### `session has no metadata.project_id`

Starting through Home Play's public-protocol start/session flow sets this automatically. CLI `plurora project start` is useful for state/diagnostics but is not the same as the Web Play → session → surface mount flow. If the surface bypasses the project flow, create a session with `metadata.project_id` manually or avoid project refs.

### `host '...' not in outbound.allowed_hosts`

The profile's `outbound.execute.allowed_hosts` is missing this provider host. Add it and restart the host.

### `secret_ref '...' not declared in package permissions`

The engine package manifest did not declare the ref in `permissions.secret_refs`. Edit the manifest and reload the package.

### `401 Unauthorized` from provider

The secret store value is usually wrong, or the provider changed the auth header format. Paste the API key again and verify that the provider/profile matches the key type.

### The surface receives no reply

Check that Play returned `session_id`, iframe `initialProps.sessionId` is non-empty, `callHostRpc` carries `session_id`, and the host outbound executor is `live` rather than the default deny/fake path.

## Implementation locations

- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.en.md) — resolver chain and project fallback.
- [`PROJECT_MODEL.md`](PROJECT_MODEL.en.md) — project + session pairing.
- `../YdlTavern/packages/ydltavern-surface/src/app/TavernProvider.tsx::sendMessage`
- `../YdlTavern/packages/ydltavern-engine/src/capabilities/model-live-call.ts`
- `crates/plurora-runtime/src/runtime/protocol_dispatch.rs::dispatch_outbound_execute`
- `crates/plurora-runtime/src/runtime/outbound.rs::LiveHttpOutboundExecutor`
- the `clients/web` surface-host iframe bridge and `mountSurface`.

## Deferred items

- Concurrent active projects: current Host scope flows through project sessions; stronger multi-tenant `project_id` in `ProtocolContext` remains incomplete.
- Production cross-origin surface-bundle allowlists, origin validation, and the corresponding CSP policy.
- Concurrent generations within one chat, token-rate presentation, and Realtime/WebSocket interaction UX.

Desktop already manages a loopback Host sidecar and reuses the same project, secret, surface, and outbound boundaries; it is no longer a missing part of this path.
