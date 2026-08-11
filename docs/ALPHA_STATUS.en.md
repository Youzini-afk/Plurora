# Platform status

> [English](./ALPHA_STATUS.en.md) · [中文](./ALPHA_STATUS.md)

A snapshot of Plurora's current implementation. It records what code, public contracts, and quality checks support, except where explicitly marked partial or deferred. It does not define why the platform exists or the only direction it may take.

For platform goals and principles, see [`CHARTER.md`](CHARTER.en.md) and [`architecture/VISION.md`](architecture/VISION.en.md). Official product responsibility is in [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.en.md); play-creation is only an optional profile. Construction direction is in [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md).

## Summary

- **Conformance:** named CLI conformance plus crate and service unit tests continue to pass; 200 v1 schemas validate (92 methods + 69 events + 39 top-level); Phase 5 adds six named Powerbox conformance cases.
- **Charter discipline:** content-free kernel; no privilege for first-party Packages; public protocol only; equal entry forms; capability handles, binding injection, Path A / Path B, the conformance kit, and generated SDKs are implemented; trusted paths block raw secrets and use manifest-declared `secret_ref` everywhere; permission grants rehydrate; network permissions are audited and redacted; generic streaming and cancel lifecycle; outbound execution has a boundary, deny-all by default; public HTTPS outbound uses the same host-policy / audit / redaction boundary; unary outbound, SSE/NDJSON/raw streams, and WebSocket all emit completion audit events.
- **Code health:** the CLI, runtime domain behavior, protocol dispatch, in-process handlers, and the event store are all split by domain. We're not stacking more onto single files.
- **Human-testing substrate:** Work sources safely pack into content-addressed WorkRevision, AssemblyRevision, and AssemblyLock artifacts. Install Lab produces an Installation candidate, while `host.installation.*` creates, updates, and removes Installations through a durable journal, idempotency claims, and revision CAS. Workspaces and Installations are separate, and linked-local sources are never deleted. RunRegistry, `host.run.*`, and Run lifecycle events now use an independent durable journal; Library actions explicitly start, inspect, and stop Runs, while opening detail never starts one. Phase 5 ExposureRegistry, Powerbox candidate disclosure, lease/revoke, runtime-handle injection, and invalidation on provider stop/revoke/expiry/version drift are implemented. The Surface bridge retains allowlists, stream ownership, redacted diagnostics, secret-input cleanup, and CSP/CORS; managed Realization remains the Phase 6 boundary.

The repository now has a substantial operational surface, but neither the platform nor the official product is “finished.” Further construction addresses openness, plurality, advanced execution and protocol capability, long-term data evolution, and complete experiences for users and creators.

## Kernel

- Content-free sessions, append-only opaque events, manifest-driven packages, capability fabric, hook fabric, surface contributions, the proposal lifecycle, and the asset / branch / projection substrate.
- A SQLite event log with monotonic per-session sequence numbers and a rehydratable substrate.
- The Experimental SHA-256 ObjectStore and ArtifactDescriptor are implemented: in-memory/filesystem CAS, verified reads, streaming, and idempotent conversion of older FNV asset records; asset events now retain only descriptors/references rather than bodies.
- Experimental EffectReceipt and Change primitives are implemented: capability/outbound/stream/WebSocket/exec terminal paths produce content-addressed receipts; historical replay calls no executor; capability re-execution creates a new branch and parent-linked receipt; the current `change.proposal.*` facade maps approval-gated operations into Intent/ChangeSet/PolicyDecision/Commit evidence.
- The Experimental Protocol Commons registry is implemented: `host.info` publishes five descriptors for Change, Shell Default, World Bundle, Work, and Assembly; explicit protocol/profile negotiation precedes dispatch; unsupported majors reject with a structured reason; protocol, implementation, and package conformance are separate executable reports.
- `plurora-work` provides content-free wire models for portable Work / Assembly / Port / State Slot, Rights / Transparency, Operational Intent / Target Inventory, Installation / Run / Exposure, and Realization, plus safe source readers, Package/Foreign/content normalization, capability-to-Port projection, a digest-memoized recursive resolver, ordinary Component adapters, and bounded structured diagnostics. `plurora work init|check|pack|inspect` content-addresses the source and complete AssemblyLock closure; old Composition machine identities are removed.
- Host Installation lifecycle has replaced the old instance model. The EventStore journal is the sole mutable authority and `installation.json` is only a rebuildable projection. Create/update/remove support idempotent replay, revision CAS, explicit state keep/delete, state snapshots, and rollback on failure. The data layout is split into `objects/`, `installations/`, `workspaces/`, and `runtime/`; startup does not read retired directories.
- Package envelopes and component identity are separated: explicit component/behavior digests survive repackaging, runtime and effect evidence carry component trust/boundary data, AssemblyLock keeps component/profile/content pins separate, and `contract:none` is reported as a non-portable Foreign Capsule.
- The Experimental World Bundle is implemented and covered by the cross-Host conformance fixture in `plurora/playable-creation-board`: canonical archive descriptors preserve exact v1 envelopes and the complete SHA-256 closure; fresh SQLite/filesystem Hosts retain objects, lineage, and receipts; historical replay invokes no executor; replacement execution creates a child branch/head; and the headless CLI reads the same archive without Web Shell state.
- A JSON Schema subset validates capability I/O and package-declared event payloads.
- The Contract V1 principal union remains `host_admin`, `host_dev`, `package`, `human`, `assistant`, and `anonymous`. Paired devices use the fail-closed `anonymous` V1 sentinel at the remote RPC boundary while a Host-established authority envelope retains the grant, delegation chain, and resource constraints; an older runtime that ignores the envelope can only deny rather than amplify authority. Redacted Host control-plane audit records still identify the logical `host_device`; human and assistant principals get scoped grants.
- Audit events: `authority/grant.created|revoked`, `authority/denied`, the `host/package.*` lifecycle, and the `change/proposal.*` lifecycle; the Host control plane also writes redacted `host/control/v1/authority.decision` records outside Contract V1.
- Persistent grants: grant / revoke events rehydrate inside a SQLite-backed runtime.
- Contract V1 is the public platform spec: 92 protocol methods, 69 event kinds, and 200 JSON Schemas. `host.run.*`, Run lifecycle events, `host.exposure.*`, `host.binding.*`, Exposure/Binding lifecycle events, `authority.handle.*`, `host.package.audit`, capability handles, binding injection, Path B, the conformance kit, and SDK generation are implemented.

## Secure execution

- **`secret_ref` references:** `secret_ref:<vault>:<key>`, `secretRef:`, `secret-ref:`, and `host:` prefixes are all supported. Packages refer to secrets through these references; raw values never appear in events, proposals, logs, or audit records.
- **Environment-variable resolver:** a host-owned resolver with an explicit allowlist. Deny-all by default; an env name has to be allowed before it can be resolved. Errors carry only the env name, never the raw value.
- **Local encrypted secret store:** `secret_ref:store:NAME` resolves through `StoreSecretResolver` from `~/.plurora/secrets.dat`; `secret_ref:installation:NAME` reads the current Installation's `secrets.dat` first and falls back to the platform store only when `secret_policy.allow_platform_fallback` permits it. Stores use age (X25519), with a master key from OS keyring (deferred) or a 0600 local key file.
- **Raw secret blocking:** proposal operations and expected effects, plus asset metadata, are scanned conservatively. Obvious API keys, tokens, and password fields are rejected. Asset content and ordinary prose aren't scanned, to avoid false positives on user content.
- **Network permission declarations:** `permissions.network` in a manifest supports both flat `hosts` (backward compatible) and structured `declarations` with `host`, `methods`, and `purpose`. A package without a declaration can't reach the network. First-party Packages don't bypass.
- **Outbound audit and redaction:** every outbound request produces an audit record holding only the principal, the package id, the capability id, the destination host, the method, the purpose, the redaction state, and the `secret_ref`s used. Raw bodies, headers, prompts, and responses are never recorded.
- **Outbound executor boundary:** content-free HTTP and WebSocket executor traits. Default is deny-all (fail closed). They can switch to fake executors (with fixtures, used by conformance) or live executors (HTTP uses reqwest + rustls; WebSocket uses tokio-tungstenite + rustls; both off by default; HTTP is HTTPS-only; WebSocket is WSS-only; redirect fail closed). Secret headers are injected at execution time only — never into audit, response, or `Debug`. Real live model / WebSocket outbound requires explicit opt-in through profile and environment variables; default conformance does not use the network, and real WebSocket smoke also requires `PLURORA_LIVE_WEBSOCKET_TESTS=1`.
- **Protocol methods:** `host.outbound.audit` lists outbound audit events for a package; `host.outbound.execute` lets ordinary packages issue unary outbound requests through the host executor; `host.outbound.stream` provides SSE/NDJSON/raw streaming outbound; `host.outbound.websocket.open|send|close` provides bidirectional WebSocket outbound.
- **Completion audit events:** `host/outbound.execute.completed`, `host/outbound.stream.completed`, and `host/outbound.websocket.completed` cover all three outbound primitives; events record only status, counts, duration, executor kind, network_performed, redaction state, and `secret_ref` references.
- **Streaming lifecycle:** the stream registry tracks in-flight streaming invocations and emits `capability/stream.started|chunk|progress|ended|error|cancelled|timeout` in order. Cancel and timeout block further chunks. Non-streaming capabilities are rejected.

## Public protocol and transport

- A canonical request / response envelope carrying a host-bound principal context. Callers can't claim to be a package or admin.
- The same dispatcher handles HTTP `POST /rpc` and host JSON-RPC stdio (`plurora host-stdio`).
- Contract Registry `0.1.0` publishes 92 exact owner-based method IDs, two explicit contract profiles, per-layer version requirements, and fail-closed Protocol Commons negotiation. HTTP RPC, Host stdio, in-process calls, and subprocess reverse stdio use the same exact resolver. The generated SDKs expose one method identity per wire ID, and the Web client sends only those IDs.
- Event subscription via SSE, with `after_sequence` replay and live tailing.
- Profile-driven `plurora host serve` autoloads packages and exposes both `/rpc` and SSE.
- The Host control plane remains separate from Contract V1: the root token is the root credential, while durable device grants attenuate both action scopes and Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization resource selectors, with bounded delegation, ancestor-revocation cascade, expiry, single revoke, and atomic administrator bulk revoke. HTTP and RPC preserve the same device identity and authority before entering the runtime, and every device protocol call records a redacted allow/deny decision. The mobile PWA and `plurora host access` CLI manage authority through the same Host API; pairing still exchanges a one-time HTTPS token for a Secure/HttpOnly cookie. See [`architecture/HOST_REMOTE_ACCESS.md`](architecture/HOST_REMOTE_ACCESS.en.md).
- TCP transport is reserved for later. WASM and remote entries are first-class manifest forms; execution is deferred.

## Package execution

- `rust_inproc` packages run through host-provided traits and a catalog. Manifests that declare an in-process provider but aren't in the catalog are rejected.
- `subprocess` packages run over JSON-RPC on stdio: handshake, invoke, timeouts, degraded state, restart, kill-on-unload, stderr capture.
- `wasm` and `remote` entries: manifests support them; execution is deferred.
- Path A (`entry.contract: "v1"`) receives capability-handle bindings and permission enforcement; Path B (`entry.contract: "none"`) runs self-contained with no v1 authority, while lifecycle remains observable.
- Capability routing supports explicit provider selection and simple exact / `^x.y` version constraints. Ambiguous routes are rejected unless the caller supplies `provider_package_id`.
- Hook fabric: deterministic subscription ordering, veto reporting, metadata/input mutation paths, and unload cleanup for `journal/before_append|after_append` and `capability/before_invoke|after_invoke`. Generic arbitrary handler-capability execution, independent async delivery, and per-handler quotas remain incomplete.

## Substrate

- Asset registry: opaque `id`, `mime`, `hash`, `size`, `origin_package_id`, `metadata`. Rehydrates from SQLite. Permission enforcement and content-addressed blob storage are next.
- Session fork / branch lineage rehydrates from the event log.
- Generic projection registry. Rebuilds filter the event log by `kind_prefix` and `writer_package_id` and write `projection/updated`. Package-owned projection execution is next.
- Installation control plane: EventStore journal, rebuildable projections, ObjectStore artifact validation, explicit state actions, restart rehydration, and `plurora installation list/info/create/update/remove` are implemented.
- Deployment runtime: `host.target.*`, `host.exec.*`, `host.port.*`, and `host.proxy.*` are implemented; default is deny-all and profiles may opt into `LiveLocalExecExecutor`. Built-in `local` and enrolled Agents use the same durable operation/artifact/verifier/deployment-receipt contracts; ports remain loopback-only, proxy upstreams must reference active port leases, and Agent traffic returns to the Host proxy only through authenticated tunnels. `ProxyRouteAccess` defaults to `host_authenticated`; only an explicit `public` route enables the optional unauthenticated `<slug>.apps.<host>/` vhost, while `/p/<route_id>/...` remains inside Host authentication. The web project console supports explicit Docker Deploy / Stop, Dockerfile / nixpacks Build & Deploy, and immutable-build-context-driven verified ChangeSet private preview, separate approval, `VerifiedActivate` revisions, and explicit reconcile/recover/rollback. Verified replay rebuilds on the recorded target without reading the live workspace or refetching source. A real MDN repository and a structurally different Python fixture cover faults, Host restart, and rollback in GitHub CI.
- Surface contributions: descriptors with version, slot, activation, required permissions, approval policy, and metadata. Slots are `experience_entry`, `home_card`, `quick_action`, `workshop_card`, `play_renderer`, `forge_panel`, `asset_editor`, and `assistant_action`. `quick_action`, `workshop_card`, and `home_card` entries with `metadata.shell_schema_version: 1` are structured shell descriptors: the web shell reads only bounded text, icon hints, order, and same-package targets, then renders them itself. It does not load package JavaScript, parse HTML, or mount iframes for those entries. Complex project surfaces still use `surface_bundle` plus sandboxed iframe hosting. Discoverable via `shell.contribution.list` and `.describe`.
- Surface bundles: `surface_bundle` is a static browser-bundle entry in the manifest, not an executable package entry. Bundles resolve from Package sources and raw paths require a Host identity. An opaque-origin sandbox receives a five-minute `/surface-assets/<lease>/...` read-only handle bound to its grant and bundle root, without a Host credential.
- Proposal lifecycle: `change.proposal.create|get|list|approve|reject|apply`. `apply` currently runs the generic operations `asset.put` and `projection.rebuild`. Broader transactions and revert / compensation are next.

## Work, Workspace, and Installation

| Capability | Status |
|---|---|
| `plurora work init/check/pack/inspect` | implemented |
| Work / Assembly source, Package, Foreign, and content-only normalization | implemented |
| Recursive resolver, explicit Adapters, and Port / Binding diagnostics | implemented |
| AssemblyLock plus complete content-addressed closure | implemented |
| `plurora/install-lab` Work candidate orchestration | implemented |
| ObjectStore persistence for Work / Assembly / Lock closure | implemented |
| `plurora installation list/info/create/update/remove` | implemented |
| `host.installation.list/get/create/update/remove` | implemented |
| `host/installation.created/updated/removed` | implemented |
| Installation journal / projection restart rehydration | implemented |
| idempotency fingerprints and revision CAS | implemented |
| update state snapshot, explicit migration/reset, and rollback on failure | implemented |
| remove `keep` / `delete` state decision | implemented |
| linked-local source is never deleted | implemented |
| managed Workspace containment / symlink defense | implemented |
| new `objects/`, `installations/`, `workspaces/`, and `runtime/` layout | implemented |
| `secret_ref:installation:NAME` plus optional platform fallback | implemented |
| Web Home Installation cards / detail | implemented |
| Run lifecycle (`host.run.*` plus five Run events) | implemented |
| Exposure lifecycle | implemented |
| Binding lifecycle / Powerbox | implemented |
| Planner / Realization | Phase 6 planned |

Work packing and Installation creation are separate: pack produces immutable artifacts only; create/update/remove mutate the Host journal. Workspace locations are Host-local bindings and never enter portable Work identity.

## Real model end-to-end path

| Capability | Status |
|---|---|
| `huggingface-fetcher` tests passing | implemented |
| Surface bundle resolution metadata-driven | implemented |
| `host.surface.bundle.resolve` | implemented |
| Host-identity-gated `/surface-bundles/<prefix>/<file>` route | implemented |
| grant/root-bound five-minute `/surface-assets/<lease>/...` sandbox handle | implemented |
| Installation-scoped secret and authority context | implemented |
| Installation → Run start and stop | implemented |
| Exposure lifecycle / Binding lifecycle | implemented |
| Surface receives `session_id` via `initialProps` | implemented |
| `TavernProvider.sendMessage` invokes engine `model.live_call` | implemented |
| API Connections drawer scope toggle (platform / Installation) | implemented |
| Engine manifest declares `secret_ref:installation:*` | implemented |
| Surface streaming response UX | implemented |
| Surface-host stream postMessage protocol | implemented |
| Surface bridge allowlist / stream ownership / diagnostics redaction / secret input cleanup / CSP/CORS hardening | implemented |
| `streamCapability` helper (YdlTavern host-rpc) | implemented |
| `AsyncIterable<StreamFrame>` consumption + iterator early-return cleanup | implemented |
| `cancelGeneration` action + Stop button | implemented |
| Multi-concurrent generation in single chat | deferred |
| Token-rate UI | deferred |
| Realtime / WebSocket streaming UX | deferred |

## First-party capability packages

All ordinary packages, no kernel privilege. They live in `packages/plurora/` and load through ordinary manifests.

**Platform foundation**

- `package-lab`, `schema-tools`, `event-tools`, `asset-lab`, `projection-lab`, `assistant-lab`.
- Package installation foundation: `plurora/git-tools-lab`, `plurora/integrity-lab`, and `plurora/install-lab`. Git tree fetches have bounded materialization by default and a measured, interruptible pack-download budget; direct callers cannot disable either hard ceiling.

**Creative capability families**

- `persona-lab`, `knowledge-lab`, `context-lab`, `text-transform-lab`.

**Model integration**

- `model-connector-lab` — offline provider metadata, profile validation, secret masking, discovery plans, compatibility reports.
- `model-provider-lab` — a cloud-API adapter lab covering OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, and Fireworks. Provides request builders, fake invocations, stream normalization, live loopback shapes, and per-provider quirks. It's not a platform model abstraction and not an API gateway.
- `model-routing-lab` — consumer-slot binding, route planning, fallback planning, and parameter normalization, with no inference of its own.

**Agents and inference**

- `pi-agent-runtime-lab` — a reference agent package: deterministic, no-network run plan, trace summary, proposal draft, and echo.
- `capability-tool-bridge-lab` — capability discovery, permission preview, explicit provider selection, invocation / streaming plans, covering nested delegation, target-branch writes, prompt injection, secret exfiltration, outbound expansion, and large-output redaction.
- `agentic-forge-lab` — the core of Agentic Forge: package-owned run lifecycle, working state, plan graph, scratch branch / candidate / compare / promote, inference nodes (deterministic / recorded / cloud-adapter plan / local fake), replay, output validation, and a 9-class failure taxonomy.
- `inference-local-lab` — a local fake inference provider that proves the inference seam doesn't depend on cloud APIs, HTTP, or bearer tokens.
- `inference-playtest-lab` — the Plurora-native "inference → proposal → inspect → approve / reject → apply → fork" vertical slice.

**Experience**

- `experience-runtime-lab` — the experience runtime contract: experience descriptors, state projection, checkpoint, recovery, and Play / Forge / Assist surface bindings.
- `playable-creation-board` — the first real playable vertical slice. Package-owned board / module / constraint / marker state, 14 capabilities, 4 surfaces.
- `experience-observability-lab` — package-owned observability: session health, package health, agent run health, proposal causal chain, cost / latency summaries, failure breadcrumbs, guardrail summaries.
- `memory-lab` — long-term memory and knowledge: record, retrieve, retrieval trace, approval-gated update, correction, forget / redaction, branch view, provenance.
- `sharing-lab` — sharing and distribution: composition bundle import / export, branch / session bundle manifests, package-set lockfiles, compatibility reports, AI disclosure metadata, read-only share manifests, async fork plans. No marketplace, no billing, no signing network.
- `playable-seed`, `blank-experience` — reference and minimum experiences.

**Storage and external projects**

- `storage-lab` — a preview of storage / data contracts: layered model, backend-class candidates, package-level state stores, document CRUD previews, content-addressed blob contract proofs, projection materialization, retrieval / vector / multimodal provider contracts.
- `tdb-retrieval-lab` — TDB as a retrieval / multimodal provider contract; not the event-log authority.
- `project-intake-lab` — external-project classification, stack detection, npm lifecycle risk, workspace plans, adapter plans, wrapper / fixture / readiness previews. No network, no filesystem.
- `workspace-lab` — workspace action policy boundary, a 10-action taxonomy, deny-by-default fake executor, deterministic fixture workspace.

**Third-party replacement proofs**

- `thirdparty/playable-seed`, `thirdparty/agent-runtime`, `thirdparty/agentic-forge`, `thirdparty/memory-lab` — show that each first-party Package can be replaced by a third party with no priority for the first-party version.

The Forge profile (`profiles/forge-alpha.yaml`) autoloads these and the example fixture packages.

## TypeScript SDKs

Under `sdk/typescript/`:

- `subprocess` — subprocess-package scaffolding and template runtime.
- `secure-execution` — `secret_ref` construction and validation, network declarations, outbound audit, faux stream-frame client.
- `inference-capability` — transport-neutral inference contract.
- `model-provider-adapter` — cloud-provider adapter helpers.
- `agent-adapter` — maps Plurora capabilities into pi-style tools.
- `agentic-forge` — run lifecycle, plan graph, working state, candidate / compare / promote, inference nodes, tool bridge v2 helpers.
- `experience-runtime` — experience runtime types and constructors.
- `text-surface` — frontend text-surface helpers (streaming buffer, frame adapter, scroll anchor, font loading).

`text-surface`, `agentic-forge`, `inference-capability`, and several others ship pure-TS self-tests.

## Contract v1 and SDK generation

- `docs/spec/PUBLIC_CONTRACT.md` is the public platform spec.
- `docs/spec/v1/schemas/` is the single source of truth for SDKs and conformance: 92 methods, 69 events, 39 top-level schemas, 200 total; the Phase 5 generation-chain hash is `9c3923c6ffd365a3b7a0a5e64a177a9d89708e70e215e454bfd9b1a5098d6e94`.
- `sdk/typescript/contract-sdk/` and `sdk/rust/plurora-contract-sdk/` are generated from schemas; the TypeScript package can be consumed through npm, workspace path, or independent codegen.
- `plurora conformance package --contract v1 --path <package>` provides 8 third-party package acceptance checks.

## Package templates

`plurora init-package --template <name>`: `basic`, `experience`, `play-renderer`, `forge-panel`, `assistant-action`, `asset-editor`, `full-surface`, `networked`, `streaming`, `agent-runtime`, `experience-runtime`, `playable-board`, `playable-experience`. Generated packages are safe by default — no raw secrets, no implicit network.

## Web shell (`clients/web`)

The platform user-facing chrome — Home, Settings, Installation flow, Installation frame, and the toast system. Built as a React 19 + Tailwind v4 + Motion + Radix + Phosphor SPA, bundled by Vite with route- and modal-level lazy splitting. Visual rules and the design system live in [`design/PLATFORM_UI_DESIGN.md`](design/PLATFORM_UI_DESIGN.en.md); detailed shell documentation is in [`../clients/web/README.md`](../clients/web/README.md).

- **Home:** the Installation shelf is backed by `host.installation.list`, with detail from `host.installation.get`. Home also consumes structured shell descriptors; package actions remain discovery affordances and do not bypass proposal / permission / audit. `⌘N` opens Installation creation.
- **Settings:** six panels, all wired to real data.
  - API Connections — `plurora/secret-store-lab/{list,put,delete}_secret` plus health. The UI never reads raw secret values; secret-edit modals wipe their input state on close.
  - Installed Packages — `host.package.list`, with Cmd/Ctrl+F focus.
  - Profiles — `host.diagnostics` (active profile, packages_loaded, network allowlist).
  - Storage — storage-area summary plus the live event-store kind (sqlite / postgres / memory), without exposing host absolute paths in the Web UI.
  - Host Access — current root/device identity, action scopes, Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization resource selectors, delegation chains, HTTPS pairing links, pending invitations, device expiry, and cascading grant revocation; new invitations select only `observe` by default.
  - About — platform identity, license, links, gratitude.
- **Installation flow:** the UI uses the real public `host.installation.create|update|remove` DTOs. Update carries an expected revision plus explicit state action; remove requires keep/delete.
- **Installation Frame:** `/installation/<id>` displays Installation detail, Library affordances, and current Run status. Start and stop use explicit `host.run.start` / `host.run.stop`; missing local matches or Realization requirements show structured gaps and next steps, while opening the page never starts a Run.
- **Failure Modal:** Deep Rust accent stripe, two-column diagnosis / impact, redacted stderr panel (with Copy log), and Restart / Stop-and-uninstall / Close actions. Data comes from `host.package.list/status/logs`; raw logs are not copied into the UI.
- **Toast system:** five variants (info/success/warning/error/progress), bottom-right queue; honors `prefers-reduced-motion`.
- **Responsive and dark mode:** explicit `data-theme` switch (system/light/dark); `@custom-variant dark` binds Tailwind's `dark:` to that attribute; the modal overlay uses a dedicated `--color-overlay` token that does not flip with theme; `prefers-reduced-motion` collapses motion; `:focus-visible` paints a keyboard navigation ring.
- **PWA and mobile control:** manifest, service worker, and offline app-shell caching are wired. `/pair` renders before the authentication gate, then uses a same-origin device cookie after claim. The mobile topbar retains Settings, locale, and logout without persisting the root token on the device.
- **SurfaceHost:** mounts third-party web surface bundles through sandboxed iframes; no implicit platform authority by default; only host-configured bridge methods reach the public protocol. The bridge limits callable authority with typed `allowed_capability_ids` and method allowlists, binds stream subscriptions to the owning surface, redacts diagnostics/logs, clears secret input state on close, and keeps the same-origin project boundary through CSP/CORS plus short-lived asset leases. Streaming subscription bridges `capability/stream.*` through postMessage.
- **No hardcoded first-party Packages — the shell is a public-protocol client like any other.**

## Desktop and releases

- `clients/desktop` provides a Tauri 2.x wrapper and owns a loopback-only managed Host sidecar. Startup generates a root token and one-time bootstrap nonce, launches `host serve` on a random port, waits for health, then navigates the hidden window. It uses a durable SQLite desktop profile, bundled Web dist, dependency-ordered first-party Package autoload, and stops the subprocess on exit. Web/PWA and Desktop reuse the same client core and public Host boundary.
- GitHub Actions CI and the `v*` tag release workflow are in place, building cross-platform Tauri installers and creating a draft release. `scripts/release-version.sh` synchronizes Cargo, the web package, the desktop package, and Tauri config.
- Build notes are in [`../BUILDING.md`](../BUILDING.md); changes are in [`../CHANGELOG.md`](../CHANGELOG.md). Signing, notarization, and auto-update are not enabled.

## Authoring flow

- `plurora init-package` generates Python or TypeScript subprocess scaffolding. `--template` chooses the surface descriptors. `--language *-experience` without `--template` still generates the legacy 4-surface experience for back-compat.
- `plurora work init/check/pack/inspect` provides the local Work/Assembly authoring flow: it safely reads `work.yaml` / `assembly.yaml` plus Package/Component sources, validates recursive Ports and Bindings, and emits content-addressed WorkRevision, AssemblyRevision, and authoring AssemblyLock artifacts. `pack` writes only to the ObjectStore; it never installs or runs the Work.
- `plurora package check` prints structured diagnostics: entry kind, trust level, capability count, surfaces by slot, permission summary, sandbox policy. Warns on packages with no capabilities or no surfaces.
- `plurora package conformance` validates a generated package locally.
- `plurora package reload <manifest>` loads the package into an in-memory runtime, restarts (subprocess only), shows before / after status and log counts, and unloads.
- `plurora package run-fixture` invokes every non-streaming capability with deterministic fixture input and prints a JSON summary.
- `plurora play-create-demo` runs the blank play-creation loop end to end.
- `plurora perf baseline` runs deterministic baseline measurements (in-process invoke, first-party capability invoke, event store append / list / range, Work check, profile load, subprocess echo) in text or JSON. See [`performance/BASELINE.md`](performance/BASELINE.en.md).

## Code organization

- `crates/plurora-cli/src/main.rs` is a thin entry. CLI types live in `cli.rs`, commands under `commands/`, and package templates under `templates/`. The conformance runner and case registry are split: `conformance/runner.rs` owns `--list`, `--case`, `--tag`, `--fail-fast`, and `--slowest`; `conformance/registry/` registers the 453 `ConformanceCase { id, tags, run }` entries by domain.
- `crates/plurora-cli/src/schema_export/` owns v1 schema export; `src/bin/export-schemas.rs` is a thin entry. Generated files still come from the exporter only — SDKs and schemas are not hand-edited.
- `crates/plurora-runtime/src/runtime/` splits runtime behavior into session, events, packages, capabilities, hooks, permissions, assets, branches, projections, and proposals. `runtime/protocol_dispatch.rs` is now the public router facade; concrete public-protocol handlers live under `runtime/protocol/` by domain. `runtime/mod.rs` keeps the public `Runtime<S>` API.
- Protocol metadata and dispatch share a single source of truth (`PlatformMethod`), with a registry / dispatch consistency unit test.
- `crates/plurora-runtime/src/inproc/` splits first-party Package behavior by domain; `plurora/install-lab` is split into `install_lab/` modules (types/source/detection/planner/candidate/executor/layout/intake/fs_copy). The shared helper routes by provider package plus local capability name, not suffix-only fallback.
- `clients/web` Home and Install flow are split into page shells plus hooks/helpers/step components. The UI still uses public protocol only and does not read the local filesystem or private runtime state.

These splits don't change behavior — they keep the codebase reviewable as more packages, conformance cases, handlers, and UI flows land.

## Conformance

`cargo run -p plurora-cli -- conformance` runs 453 named CLI cases. Flags:

- `--list` — list ids and tags.
- `--case <pattern>` — substring filter.
- `--tag <tag>` — filter by tag.
- `--fail-fast` — stop at the first failure.
- `--slowest <N>` — report the slowest N.

Every case has tags (runtime / event / capability / package / subprocess / first_party / network / outbound / stream / agentic / experience / memory / sharing / secret / composition / replacement / surface / protocol / permission / hook / host / asset / projection / substrate / storage / live / external_project / project_intake / workspace_lab / retrieval, and so on). See [`performance/CONFORMANCE_FEEDBACK.md`](performance/CONFORMANCE_FEEDBACK.en.md).

Plus crate and service unit tests via `cargo test --workspace`, and `npm run check --prefix clients/web` / `npm run build --prefix clients/web` for the web shell.

## Partial (started, not finished)

- `event.subscribe` permission for package principals.
- Timeout / error audit for package-owned hook handlers.
- Persistent capability-provider selection policy beyond explicit per-call selection.
- Richer resource policy (filesystem enforcement matrix).
- Content-addressed asset blob storage and package-principal asset permissions: stable content-address helpers and metadata conventions are done; full blob storage and runtime enforcement aren't.
- Package-owned projection execution.
- Active health probes, automatic recovery/rebind, and richer failure monitoring; definitively observed permanent JSON-RPC stdio transport loss already durably fails affected Runs.
- Broader transport consistency coverage.
- Desktop release code signing / notarization, auto-updater, real app icons, plus richer managed-Host crash recovery and sidecar-update coordination.
- Surface lifecycle callbacks such as `onClose` and `onProposalDraft`, plus a cross-origin surface-bundle allowlist.
- Full surfacing of `context.get|list`, `host.package.describe`, `capability.describe`, `protocol.extension.describe`, `identity.current`, `host.ping`.

## Deferred (explicitly out of kernel scope)

These will arrive as ordinary packages or future work — not as kernel features:

- Conversation runtime, prompts, models, sampling, message / turn semantics.
- Memory models, retrieval, summarization, agent loops, directors.
- World, scene, character, rule, dice, inventory semantics.
- SillyTavern compatibility lives in the YdlTavern integration project on top of Plurora (see [`tavern/TAVERN_COMPAT.md`](tavern/TAVERN_COMPAT.en.md)).
- Production-grade long-running autonomous agents, multi-agent collaboration, production memory systems, fuller live-ops.
- External game-engine bridges (UE5, Godot, Unity, web clients).
- Marketplace, package signing, dependency resolution (local sharing proof is done; see [`guides/SHARING_DISTRIBUTION.md`](guides/SHARING_DISTRIBUTION.en.md)).
- Final UI visual design, full Studio, ComfyUI-like node editors.
- WASM and remote package execution.

## How to verify this snapshot

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- conformance --list
cargo run -p plurora-cli -- conformance --tag sharing --slowest 3
cargo run -p plurora-cli -- play-create-demo
npm run check --prefix clients/web
npm run build --prefix clients/web
```

If anything fails, the code is the source of truth — update this document.

## Further reading

- [`CHARTER.md`](CHARTER.en.md) — principles that don't change.
- [`architecture/`](architecture/README.en.md) — architecture, kernel, package contract, extension points, events, lifecycles.
- [`product/`](product/README.en.md) — play-creation stance.
- [`protocol/PUBLIC_PROTOCOL.md`](protocol/PUBLIC_PROTOCOL.en.md) — public protocol.
- [`spec/`](spec/README.en.md) — executable contract matrix and conformance roadmap.
- [`guides/`](guides/README.en.md) — capability-package authoring guides.
- [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.en.md) — what's next.
