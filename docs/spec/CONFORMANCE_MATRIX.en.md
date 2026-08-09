# Conformance Matrix

> [English](./CONFORMANCE_MATRIX.en.md) · [中文](./CONFORMANCE_MATRIX.md)

The conformance suite is the executable guardian of the charter. It proves both positive behavior and rejection behavior. New cases land here as they are added. Cases marked partial or future remain in scope for later hardening; see `docs/roadmap/NEXT_STEPS.md`.

## Current release-gate command

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
```

The current matrix records implemented conformance coverage. Named CLI cases and crate/service unit tests support these results. Current CLI conformance total: **453**.

## Conformance Feedback Loop

The conformance command supports filtering, timing, and diagnostics. See [`docs/performance/CONFORMANCE_FEEDBACK.en.md`](../performance/CONFORMANCE_FEEDBACK.en.md) and [`docs/performance/PERFORMANCE_AND_CODE_HEALTH.en.md`](../performance/PERFORMANCE_AND_CODE_HEALTH.en.md).

```bash
# List all case ids and tags
cargo run -p plurora-cli -- conformance --list

# Filter by substring
cargo run -p plurora-cli -- conformance --case sharing_lab

# Filter by tag
cargo run -p plurora-cli -- conformance --tag sharing

# Fail-fast
cargo run -p plurora-cli -- conformance --fail-fast

# Custom slowest report
cargo run -p plurora-cli -- conformance --slowest 3
```

## Current conformance coverage

### Project model conformance cases

The current matrix includes the following project-model cases. Verify the actual case ids with `cargo run -p plurora-cli -- conformance --list | grep -E "(host_profile|project|protocol\.project)"`.

| Group | Case id | Coverage | Status |
|---|---|---|---|
| secret resolver | `secret_store_resolver.host_profile_installs_composite_resolver` | host profile installs env+store/project composite resolver | implemented |
| project secrets | `project_secret.put_then_resolve_via_project_ref` | `secret_ref:project:*` reads project store | implemented |
| project secrets | `project_secret.fallback_to_platform_when_missing` | project miss falls back to platform store by policy | implemented |
| project secrets | `project_secret.no_fallback_when_disabled` | disabled fallback fails closed | implemented |
| project secrets | `project_secret.require_per_project_blocks_fallback` | `require_per_project` blocks platform fallback | implemented |
| project secrets | `project_secret.isolation_between_projects` | project secret stores are softly isolated | implemented |
| project secrets | `project_secret.no_session_context_fails_closed` | missing project/session context fails closed | implemented |
| project secrets | `project_secret.list_returns_names_not_values` | project secret list returns names, not values | implemented |
| project install | `project.detect_native_yaml` | detects native `project.yaml` | implemented |
| project install | `project.detect_no_yaml` | no `project.yaml` enters external-project path | implemented |
| project install | `project.detect_invalid_yaml_rejected` | invalid descriptor rejected | implemented |
| project install | `project.register_creates_project_dir` | project registration creates data directory | implemented |
| project registry | `project.list_returns_registered` | registry/list returns registered projects | implemented |
| project runtime | `project.state_transitions` | start/stop state transitions | implemented |
| project uninstall | `project.archive_keeps_data` | uninstall keep-data archives project directory | implemented |
| project protocol | `protocol.project_list_returns_registered_projects` | `host.project.list` returns projects | implemented |
| project protocol | `protocol.project_get_returns_full_descriptor` | `host.project.get` returns full descriptor | implemented |
| project protocol | `protocol.project_start_transitions_state` | `host.project.start` transitions state | implemented |
| project protocol | `protocol.project_methods_require_admin_principal` | project methods restricted to HostAdmin/HostDev | implemented |
| project protocol | `protocol.project_lifecycle_event_emitted_on_start` | start emits project lifecycle event | implemented |

### End-to-end real-path conformance cases

The current matrix includes the following end-to-end-real-path cases. Verify the actual case ids with `cargo run -p plurora-cli -- conformance --list | grep -E "(surface\.resolve|project\.start_returns|session_metadata|running_session|stop_closes)"`.

| Group | Case id | Coverage | Status |
|---|---|---|---|
| dev bundle | `surface.resolve_via_dev_path` | dev-path surface bundle resolution | implemented |
| installed bundle | `surface.resolve_via_installed_project` | installed-project surface bundle resolution | implemented |
| bundle rejection | `surface.resolve_unknown_fails` | unknown surface bundle fails closed | implemented |
| bundle authority | `surface.resolve_admin_principal_required` | resolve_bundle restricted to HostAdmin/HostDev | implemented |
| project session | `project.start_returns_session_id` | `project.start` returns a project session id | implemented |
| project session | `project.start_idempotent_returns_existing_session` | repeated start returns the existing session | implemented |
| project session | `project.session_metadata_carries_project_id` | session metadata carries project_id | implemented |
| project session | `project.stop_closes_session` | stop closes the project session | implemented |
| project session | `project.get_returns_running_session_id` | get/status return running_session_id while Running | implemented |

Surface/static bundle and bridge coverage also includes these stable assertions:

| Assertion | Coverage | Status |
|---|---|---|
| static surface bundle | `surface_bundle` is a static browser entry and does not use the wasm sentinel or package execution | implemented |
| project-root install surface dist | native project dist lives under Host/project-authority-gated `/surface-bundles/projects/<project_id>/...` | implemented |
| sandbox asset attenuation | authorized resolution issues a five-minute `/surface-assets/<lease>/...` handle bound to the grant/bundle root; cross-root, forged, expired, or revoked-grant access is denied | implemented |
| bridge allowlist | typed `allowed_capability_ids` precisely constrains callable surface-bridge capabilities | implemented |
| metadata not authority | surface metadata describes entries only and does not grant authority | implemented |
| stream ownership | stream subscribe/unsubscribe is bound to the owning surface and session | implemented |
| redacted diagnostics | bridge diagnostics, errors, and logs do not leak raw secrets or host absolute paths | implemented |
| uncontrolled secret input | secret inputs remain uncontrolled/short-lived and are cleared on close | implemented |
| schema timestamp stability | schema/export timestamps are stable and avoid nondeterministic generated timestamps | implemented |
| surface-bundle freshness | `dist/` participates in `tree_hash`, so bundle-only changes trigger updates | implemented |
| store schema migration | a store schema bump clears old store entries so stale hashes are not reused | implemented |
| orphan store GC | install/update/uninstall remove store entries no longer referenced by lockfiles/profiles | implemented |
| project updates | `plurora/install-lab/check_for_updates` and `update_project` power CLI and web update entries | implemented |


| Area | Case | Status |
|---|---|---:|
| session | open content-free session | implemented |
| events | authorized package appends own namespace event | implemented |
| events | package denied when writing without `events.append` | implemented in unit tests |
| events | package denied when reading without `events.read` | implemented |
| events | package denied when writing another namespace | implemented in unit tests |
| events | Package denied when writing a registered platform-owned kind such as `context/opened` | implemented in unit tests |
| events | closed session rejects append | implemented |
| events | sequence-range replay with filters | implemented |
| package | valid manifest loads | implemented |
| package | lifecycle timeline emits loading/starting/ready/loaded | implemented |
| package | restart subprocess package | implemented |
| package | capture subprocess stderr logs | implemented |
| package | host policy rejects disallowed entry | implemented in unit tests |
| package | unload removes registry record | implemented in unit tests |
| package | unload removes capability provider | implemented |
| capability | discover registered capability | implemented |
| capability | invoke rust_inproc echo through package trait | implemented |
| capability | ambiguous provider rejected | implemented in unit tests |
| capability | explicit provider selection resolves duplicate providers | implemented |
| capability | version constraint filters providers | implemented |
| publisher equality | a `plurora/*`-looking Package has no route precedence | implemented |
| hooks | veto fixture reports veto | implemented in unit tests |
| hooks | stable ordering by precedence/package/handler | implemented |
| hooks | before event append veto blocks operation | implemented |
| hooks | before event append metadata mutation is applied | implemented |
| hooks | package-owned hook handler capability is invoked | implemented |
| hooks | unload removes hook subscription | implemented |
| storage | SQLite persists/replays events | implemented in unit tests |
| assets | put/get/list adapt through SHA-256 descriptors and keep event bodies out of the journal | implemented |
| assets | legacy FNV inline events migrate idempotently while retaining old id/hash/event provenance | implemented |
| object store | cross-host digest equality, unknown-type copy/stream, and tamper rejection | implemented |
| sessions | fork session and list branch lineage | implemented |
| projections | register and rebuild generic event-count projection | implemented |
| substrate | SQLite event log rehydrates assets, branches, and projections | implemented |
| substrate | permission grant survives SQLite-backed runtime rehydrate | implemented |
| effect receipts | historical replay reads recorded output after provider unload, missing objects report incomplete history, and re-execution creates a new branch plus parent-linked receipt | implemented |
| effect receipts | secret-bearing input/output enters receipts only as redacted object references and the receipt envelope scans clean | implemented |
| secret refs | `secret_ref:`, `secretRef:`, `secret-ref:`, `host:` reference pattern validation | implemented |
| secret refs | raw secret in proposal payload is rejected | implemented |
| secret refs | raw secret in asset metadata is rejected | implemented |
| secret refs | first-party Package has no secret-scanning bypass | implemented |
| env resolver | `EnvSecretResolver` allows resolution when env name is in allowlist (`secret_ref:env`, `secretRef:env`, `secret-ref:env`, `host:env`) | implemented |
| env resolver | `EnvSecretResolver` denies resolution when env name is not in allowlist; non-env vault and `host:<key>` rejected | implemented |
| env resolver | `EnvSecretResolver` missing env var returns typed error without leaking raw value | implemented |
| secret store | 10 secret_store cases: put / has / list / delete / health plus env/store/composite resolver paths | implemented |
| protocol | method list contains no content methods | implemented in unit tests |
| protocol | structured permission error code | implemented |
| protocol / identity | the registry exposes one wire ID per method, publishes no aliases, and rejects unknown identities | implemented |
| protocol / owner namespaces | smoke calls only Host/Shell/Change/Projection owner IDs and explicitly negotiates the default and Shell Default profiles | implemented |
| protocol / negotiation | unknown layer versions return `unsupported_contract` explicitly | implemented |
| protocol / negotiation | failed negotiation never silently downgrades and has zero handler side effects | implemented |
| protocol | in-process protocol dispatcher calls host.info | implemented |
| protocol | in-process protocol dispatcher invokes capability | implemented |
| protocol | HTTP `/rpc` returns protocol envelope | implemented in service tests |
| protocol | host stdio responds to protocol envelope | implemented by CLI validation |
| principal | package context overrides caller-supplied event writer | implemented |
| principal | package context overrides caller-supplied capability caller | implemented |
| principal | human and assistant protocol principals exist | implemented |
| permissions | grant/revoke/list/audit protocol | implemented |
| permissions | assistant capability invoke requires explicit grant | implemented |
| schema | capability input schema rejects invalid input | implemented |
| schema | event payload schema rejects invalid payload | implemented |
| subprocess | JSON-RPC stdio package loads and reports ready | implemented |
| subprocess | JSON-RPC stdio capability invoke works | implemented |
| subprocess | bad handshake is rejected | implemented |
| subprocess | invoke timeout degrades package | implemented |
| subprocess | invalid subprocess output schema is rejected | implemented |
| subprocess | unload removes subprocess capability | implemented |
| service | SSE event subscribe endpoint replays and tails events | implemented |
| host | diagnostics reports packages/capabilities/hooks | implemented |
| host | profile autoload loads configured packages | implemented |
| surfaces | package-contributed typed surface descriptors can be listed, described, and filtered | implemented |
| first-party Packages | foundation packages load and invoke without privilege | implemented |
| first-party Packages | asset-lab previews assets and drafts approval-gated import plans without privilege | implemented |
| first-party Packages | projection-lab drafts rebuild plans and explains source events without privilege | implemented |
| first-party Packages | playable-seed exposes reference entry/play/Forge/assistant surfaces and approval-gated edits | implemented |
| first-party Packages | persona-lab imports and renders persona profiles with provenance without kernel ontology | implemented |
| first-party Packages | knowledge-lab normalizes collections, matches entries, and returns plan-only injection output | implemented |
| first-party Packages | context-lab assembles generic blocks with budget omissions and template rendering | implemented |
| first-party Packages | text-transform-lab previews deterministic text transforms with trace and validation diagnostics | implemented |
| first-party Packages | model-connector-lab validates profiles, rejects raw secrets, and returns no-network discovery plans | implemented |
| first-party Packages | model-provider-lab as cloud API adapter lab lists eight cloud provider families, validates profiles rejecting raw secrets, package-local normalize_request covers eight dialects/endpoints, explains errors (401/429/529), outputs network_performed:false/inference_performed:false, no raw secret echoed; it is not the platform model abstraction | implemented |
| first-party Packages | model-provider-lab cloud adapter invoke all eight provider families (OpenAI chat/responses, Anthropic messages, Gemini generateContent, OpenAI-compatible chat, OpenRouter chat/responses, DeepSeek chat, xAI chat/responses, Fireworks chat/responses; fake/local, auditable outbound_request_shape, raw credential rejected, openai_compatible missing/http base_url rejected, unsupported family diagnostic, executor_kind fake_local, live_call_supported false) | implemented |
| first-party Packages | model-provider-lab cloud adapter normalize_stream eight families stream normalization (delta SSE, semantic SSE, typed chunk stream → StreamFrameEnvelope frames: start/chunk/progress/end/error/cancelled/timeout; terminal_frame_consistent; provider event input normalization; no raw secret echo; unsupported family empty frames + terminal_frame_consistent false) | implemented |
| outbound | model provider outbound shape fake executor (three-provider host/method/path/secret_ref shapes pass outbound boundary, call_count=3, executor_kind Fake) | implemented |
| first-party Packages | model-routing-lab resolves deterministic route plans with explicit fallbacks and normalized params | implemented |
| first-party Packages | pi-agent-runtime-lab produces no-inference/no-network run plans, approval-gated proposals, trace summaries, and discoverable surfaces | implemented |
| first-party Packages | capability-tool-bridge-lab marks ambiguous provider rejected, explicit third-party provider available, first-party provider not preferred, missing provider rejected, denied preview reports missing permission, raw secret unsafe_blocked | implemented |
| first-party Packages | inference-local-lab describe_capabilities: no network/secret required, transports include in_memory/local_process, operation_kinds include generate/classify/transform | implemented |
| first-party Packages | inference-local-lab invoke non-HTTP succeeds with no URL/header/status/messages fields, network_performed=false, transport_performed=in_memory_fake | implemented |
| first-party Packages | inference-local-lab invoke rejects http transport, HTTP-shaped fields (url/header/status_code), messages-shaped fields (messages/system/user/assistant), raw secret | implemented |
| first-party Packages | inference-local-lab stream emits deterministic start/chunk/progress/end frames, no URL/header/status/provider_schema | implemented |
| first-party Packages | inference-local-lab explain_error covers local/resource error classes (local_process_failed/local_resource_exhausted/local_model_not_loaded/local_inference_error/timeout/cancelled) | implemented |
| first-party Packages | inference-playtest-lab draft_proposal produces proposal_draft with requires_user_approval=true, asset.put, source_inference provenance, no raw secret, not a chat message | implemented |
| first-party Packages | inference-playtest-lab inspect_proposal returns risk/operations/permissions/provenance summary without applying | implemented |
| first-party Packages | inference-playtest-lab rejected proposal cannot apply | implemented |
| first-party Packages | inference-playtest-lab approve/apply succeeds, asset written, branch_plan + fork creates branch with proposal/source inference provenance | implemented |
| first-party Packages | inference-playtest-lab output contains no messages/prompt/chat/platform.model terms | implemented |
| in-process packages | non-first-party `/preview` suffix does not receive first-party asset-lab fallback behavior | implemented |
| in-process packages | unknown registered in-process capability fails loudly instead of returning generic fallback success | implemented |
| first-party Packages | assistant-lab returns approval-gated proposals through grants | implemented |
| play-creation | blank loop exercises assistant proposal, branch, asset, projection | implemented |
| proposals | approved proposals can apply generic asset/projection operations | implemented |
| proposals | rejected or unapproved proposals cannot apply | implemented |
| proposals | v1 Proposal maps to Intent/ChangeSet/PolicyDecision/Commit and apply/reject produce operation/final receipts | implemented |
| package authoring | generated Python subprocess package passes local conformance | implemented |
| package authoring | generated TypeScript subprocess package passes local conformance | implemented |
| package authoring | generated experience package surfaces pass local conformance | implemented |
| Work / Assembly | nested Assembly Port exposure resolves while preserving the complete exposure chain | implemented |
| Work / Assembly | repeated packing of the same Work source produces the same content digest | implemented |
| replacement | third-party playable-seed surfaces discoverable through shell.contribution.list | implemented |
| replacement | third-party playable-seed capability invocation works through normal routing | implemented |
| replacement | ambiguous first-party + third-party equivalent capability rejects route without publisher priority | implemented |
| replacement | Work source validates the third-party playable-seed replacement shape | implemented |
| replacement | third-party agent-runtime surfaces (assistant_action/forge_panel/home_card) discoverable through shell.contribution.list | implemented |
| replacement | third-party agent-runtime capability invocation produces no-inference/no-network, approval-gated proposal, provenance match | implemented |
| replacement | Work source validates the third-party agent-runtime replacement shape without publisher priority | implemented |
| network | package without network permission denied outbound, produces outbound.denied audit | implemented |
| network | allowlisted host+method allowed, produces redacted outbound.request audit | implemented |
| network | host/method mismatch denied | implemented |
| network | first-party Package has no network bypass | implemented |
| network | audit records contain no raw secrets/bodies, only secret_ref and redaction_state | implemented |
| network | check_network_policy pure function tests | implemented |
| outbound | no permission executor not called — denied request never reaches executor | implemented |
| outbound | policy/audit request and executor request package/capability/host/method/secret_refs mismatch fails closed and never calls executor | implemented |
| outbound | allowlisted fake executor returns network_performed:false, executor_kind:fake, redacted audit | implemented |
| outbound | raw body_shape not persisted in audit; audit redaction_state redacted/not_captured | implemented |
| outbound | secret_refs stored as references only; raw secrets rejected/not echoed | implemented |
| outbound | host mismatch redirect denied; redirect_target check reserved for later hardening | implemented |
| stream | normal lifecycle emits ordered frames/events | implemented |
| stream | cancel marks invocation cancelled and blocks further chunks | implemented |
| stream | timeout marks invocation timeout and blocks further chunks | implemented |
| stream | error terminal frame works | implemented |
| stream | non-streaming capability (streaming=false) rejected from stream | implemented |
| stream | no model/agent methods added to protocol | implemented |
| stream | capability.stream and capability.cancel dispatchable through protocol | implemented |
| package authoring | generated networked template passes check/conformance with network declarations, no raw secrets | implemented |
| package authoring | generated streaming template passes check/conformance with streaming capability | implemented |
| no-network readiness | faux-model-readiness package declares network permissions, provides streaming capability, uses secret_ref, no raw secrets | implemented |
| no-network readiness | faux-agent-readiness package has no network permissions, provides streaming capability, uses proposal/trace patterns, no raw secrets | implemented |
| outbound | live HTTP executor disabled by default; RuntimeConfig::default remains DenyAll | implemented |
| outbound | live HTTP executor rejects non-HTTPS URLs; no network attempted | implemented |
| outbound | live HTTP executor response shape contains no raw body/header/secret | implemented |
| outbound | host.outbound.execute public protocol: package principal determined from context (no spoofing), FakeOutboundExecutor + allowed network declaration succeeds with audit | implemented |
| outbound | host.outbound.execute spoofed package_id rejected, cannot act as another package | implemented |
| outbound | host.outbound.execute no network permission denied, executor not called | implemented |
| outbound | host.outbound.execute response contains no raw secret (secret_refs as references only) | implemented |
| outbound | host.outbound.execute `secret_headers` params parsed correctly, raw secret never in response | implemented |
| outbound_execute | profile default deny-all, fake/live executor config, package permission, capability namespace, no-permission denial, secret_ref declarations, response redaction | implemented |
| outbound_stream | `host.outbound.stream` profile default denial, fake stream frames, secret_ref declarations, capability namespace, HTTPS-only policy | implemented |
| outbound_websocket | `platform.outbound.websocket.*` profile default deny-all, fake executor open/send/close, live executor denial when disabled | implemented |
| outbound_websocket | undeclared secret_ref fails closed, capability namespace enforcement, default WSS-only | implemented |
| outbound_websocket | idle timeout emits error + completed, inbound max_total_bytes terminates, max_concurrent_connections enforced, cancel via `capability.cancel` | implemented |
| outbound | `host/outbound.execute.completed` completion audit event emitted | implemented |
| outbound | `host/outbound.stream.completed` completion audit event emitted | implemented |
| outbound | `host/outbound.websocket.completed` completion audit event emitted | implemented |
| outbound | HTTP/stream/WebSocket completion attaches terminal receipts; invalid policy/executor pairings emit failed receipts; timeout/cancel do not produce duplicate stream terminals; historical replay works with every executor disabled | implemented |
| deployment exec | deny-all start and fake stop produce denied/cancelled receipts; live terminal state is actively observed; natural exit/timeout, repeated denial, stop/status races, and restart hydration preserve one terminal receipt | implemented |
| secret_ref | manifest `permissions.secret_refs` declaration: undeclared refs fail closed, declared refs resolve via host resolver | implemented |
| subprocess_outbound | subprocess SDK reverse kernel call: principal binding, execute dispatch, stream chunks piped back | implemented |
| sse_parser | outbound stream SSE parser basic smoke and partial chunk coalescing | implemented |
| live_model | live smoke is skipped by default; real calls require `PLURORA_LIVE_MODEL_TESTS=1` plus provider env vars | implemented |
| outbound | local loopback HTTP server secret injection: Authorization header actually arrives at server, raw secret not in protocol response/audit/log | implemented |
| outbound | DeepSeek SSE stream normalize canary: delta_sse start→chunk→end lifecycle, terminal_frame_consistent, no raw secrets | implemented |
| outbound | opt-in live DeepSeek conformance: default skip, only when PLURORA_LIVE_MODEL_TESTS=1 + DEEPSEEK_API_KEY | implemented |
| outbound | canary DeepSeek profile shape: normalize_request endpoint/dialect/stream_family correct, secret_ref placeholder no raw key | implemented |
| outbound | OpenAI Chat Completions loopback: Authorization Bearer arrives at server, POST /v1/chat/completions, body shape model+messages, raw secret not in response/audit | implemented |
| outbound | OpenAI Responses loopback: Authorization Bearer arrives, POST /v1/responses, body shape uses input field, raw secret not in response/audit | implemented |
| outbound | Anthropic Messages loopback: x-api-key secret header + anthropic-version static header arrive at server, POST /v1/messages, body shape content blocks, raw secret not in response/audit | implemented |
| outbound | Gemini generateContent loopback: x-goog-api-key secret header arrives at server, POST /v1beta/models/{model}:generateContent, body shape contents/parts, raw secret not in response/audit | implemented |
| outbound | missing secret fails closed: unavailable secret_ref produces error, no outbound request sent, no raw secret in error | implemented |
| outbound | provider normalize_request alignment: OpenAI chat+responses, Anthropic messages, Gemini generateContent endpoints/dialects match outbound.execute params, credential placeholders not raw | implemented |
| outbound | no raw secret leak across all providers: OpenAI/Anthropic/Gemini shapes through FakeOutboundExecutor, response+audit contain no raw secrets | implemented |
| outbound | static_headers safe allowlist: anthropic-version accepted, safe non-secret headers injected | implemented |
| outbound | static_headers block secrets: Authorization/x-api-key/Cookie in static_headers rejected, must use secret_headers | implemented |
| outbound | OpenRouter loopback headers: Authorization Bearer + HTTP-Referer + X-Title static headers arrive at server, POST /api/v1/chat/completions, raw secret not in response/audit | implemented |
| outbound | xAI loopback: Authorization Bearer arrives at server, POST /v1/chat/completions, reasoning/usage sanitized, raw secret not in response/audit | implemented |
| outbound | Fireworks loopback: Authorization Bearer arrives at server, POST /inference/v1/chat/completions, perf/usage metadata sanitized, raw secret not in response/audit | implemented |
| stream | DeepSeek reasoning stream normalization: reasoning_content → reasoning_delta frames, cache usage → progress frames, terminal_frame_consistent, no raw secrets | implemented |
| stream | OpenRouter mid-stream error normalization: error object after HTTP 200 → error frame with mid_stream_error provider_event | implemented |
| outbound | provider quirks sanitized fixtures: integrations/model-providers/fixtures/*.json contain no real keys or provider-looking raw keys, scan finds nothing | implemented |
| outbound | static_headers OpenRouter safe: http-referer/x-title on allowlist, not secret-bearing; Authorization/x-api-key still blocked | implemented |
| first-party Packages | experience-observability-lab describe_observability returns 8 capabilities, 3 surfaces, output shapes, no forbidden namespace | implemented |
| first-party Packages | experience-observability-lab summarize_session_health derives status from protocol-visible refs, no SQLite reads | implemented |
| first-party Packages | experience-observability-lab summarize_package_health returns package health from protocol-visible refs | implemented |
| first-party Packages | experience-observability-lab summarize_agent_run_health returns agent run health from protocol-visible refs | implemented |
| first-party Packages | experience-observability-lab trace_proposal_causality returns causal chain with content_address per step | implemented |
| first-party Packages | experience-observability-lab summarize_cost_latency returns cost/latency summary from outbound audit refs, no raw secrets | implemented |
| first-party Packages | experience-observability-lab list_failure_breadcrumbs returns breadcrumbs from protocol-visible event refs | implemented |
| first-party Packages | experience-observability-lab summarize_guardrails returns guardrail/audit summary from protocol-visible audit refs | implemented |
| first-party Packages | experience-observability-lab no platform.observability.* / platform.experience.* namespace in any output | implemented |
| first-party Packages | experience-observability-lab raw secret blocked in all capability inputs | implemented |
| first-party Packages | memory-lab describe_memory_contract returns 9 capabilities, 3 surfaces, output shapes, no forbidden namespace | implemented |
| first-party Packages | memory-lab record_memory produces memory_record with content_address / branch_ref / knowledge_refs | implemented |
| first-party Packages | memory-lab retrieve_memory deterministic keyword match, branch-aware filtering, no embedding/network | implemented |
| first-party Packages | memory-lab trace_retrieval produces deterministic retrieval trace | implemented |
| first-party Packages | memory-lab draft_memory_update produces proposal/update draft only, no direct state mutation, requires_user_approval=true | implemented |
| first-party Packages | memory-lab apply_memory_correction produces correction shape, proposal-gated | implemented |
| first-party Packages | memory-lab draft_forget_redaction produces redaction plan, not deletion | implemented |
| first-party Packages | memory-lab branch_memory_view filters memory records by branch | implemented |
| first-party Packages | memory-lab no output contains platform.memory.* / platform.experience.* namespace | implemented |
| first-party Packages | memory-lab raw secret blocked in all capability inputs | implemented |
| first-party Packages | sharing-lab describe_sharing_contract returns 9 capabilities, 3 surfaces, output shapes, red lines, no forbidden namespace | implemented |
| first-party Packages | sharing-lab export_work_bundle produces a content-addressed bundle pinning WorkRevision, root AssemblyRevision, AssemblyLock, and Package pins, with no marketplace/billing fields | implemented |
| first-party Packages | sharing-lab import_work_bundle recomputes bundle/lock identity, validates the typed closure and no raw secrets, and remains plan-only | implemented |
| first-party Packages | sharing-lab create_branch_session_bundle produces branch/session bundle manifest with content_address and AI disclosure | implemented |
| first-party Packages | sharing-lab create_package_set_lockfile pins package versions and content addresses | implemented |
| first-party Packages | sharing-lab compatibility_report compares two bundle versions, deterministic, detects incompatibilities | implemented |
| first-party Packages | sharing-lab ai_disclosure_bundle produces AI disclosure metadata marking content provenance | implemented |
| first-party Packages | sharing-lab read_only_share_manifest read-only shared session manifest, local_file proof, no remote service | implemented |
| first-party Packages | sharing-lab async_fork_share_plan async fork sharing plan, draft/plan-only/requires_user_approval | implemented |
| first-party Packages | sharing-lab no marketplace/billing/signing fields, no raw secrets, no platform.sharing/marketplace/billing namespace | implemented |
| storage backend | in-memory EventStore satisfies append/list/range/next_sequence basic contract | implemented |
| storage backend | SQLite EventStore satisfies append/list/range/next_sequence basic contract | implemented |
| storage backend | in-memory and SQLite kind-prefix query results are semantically identical | implemented |
| storage backend | in-memory and SQLite concurrent append produces no duplicate sequences | implemented |
| storage backend | in-memory and SQLite subscription broadcast behavior matches after append | implemented |
| storage backend | in-memory and SQLite rehydrate event replay semantics are identical | implemented |
| storage lab | storage-lab contract shape contains no kernel database terms (platform.sqlite/postgres/tdb/vector/embedding/collection/sql/database) | implemented |
| storage lab | storage-lab backend class candidates contain capability flags only, no secret-bearing backend config | implemented |
| storage lab | package state plan namespace belongs to owning package, no publisher priority | implemented |
| storage lab | put document preview does not perform real write (write_performed=false) | implemented |
| storage lab | get document preview does not perform real read (read_performed=false) | implemented |
| storage lab | query prefix preview does not execute real query (query_performed=false) | implemented |
| storage lab | delete tombstone preview does not perform real deletion (delete_performed=false) | implemented |
| storage lab | export snapshot preview output is redacted (snapshot_exported=false) | implemented |
| storage lab | raw secret is blocked in all capability inputs | implemented |
| storage lab | unsafe ID (path traversal / special characters) is blocked | implemented |
| storage lab | blob store contract shape contains content-addressed type, backend candidates, red lines, no kernel database/blob namespace | implemented |
| storage lab | put blob preview content address deterministic (content_hash normalized with sha256: prefix, same sample → same hash) | implemented |
| storage lab | put blob preview does not perform real storage or include blob content (blob_stored=false, event_payload_contains_blob=false) | implemented |
| storage lab | get blob metadata preview does not return blob content (blob_read=false, content_returned=false) | implemented |
| storage lab | export blob manifest preview contains refs only, no content (content_included=false) | implemented |
| storage lab | blob raw secret, unsafe ID, oversized inline sample are blocked | implemented |
| storage lab | projection contract shape — backend candidates, red lines, no DB table/collection/vector/database namespace | implemented |
| storage lab | projection materialization plan only (materialized=false, write_performed=false, backend_selected=false) | implemented |
| storage lab | projection query preview no execution (query_executed=false, rows_returned=false) | implemented |
| storage lab | projection migration plan no rewrite (migration_applied=false, data_rewritten=false, requires_rebuild=true) | implemented |
| storage lab | projection rejects raw secret in all projection capability inputs | implemented |
| storage lab | projection no DB table leakage — no SQL/table/collection/vector/database terms across all projection capabilities | implemented |
| storage lab | retrieval provider contract shape — backend candidates, red lines, no kernel vector/embedding namespace | implemented |
| storage lab | multimodal index plan — no embedding generation, no index creation, no vector storage | implemented |
| storage lab | multimodal index rejects invalid modality or too many asset_refs | implemented |
| storage lab | vector search plan — no search execution, no embedding, no vector loading | implemented |
| storage lab | backend fit TDB is a provider slot, with real Rust adapter as opt-in proof — no kernel vector namespace, no credentials | implemented |
| storage lab | retrieval rejects raw secret in all retrieval capability inputs | implemented |
| storage lab | retrieval no kernel vector/embedding namespace or credentials across all retrieval capabilities | implemented |
| creator loop | generated playable-board template passes check/conformance with 4 surfaces, 7 capabilities, no network | implemented |
| creator loop | generated playable-experience template passes check/conformance with 4 surfaces, 9 capabilities including checkpoint/recovery | implemented |
| creator loop | experience_entry surface without play_renderer/forge_panel/assistant_action produces creator warnings | implemented |
| creator loop | missing create_checkpoint capability warns for experience packages | implemented |
| creator loop | dangerous permissions (wildcard invoke, empty network methods) produce creator warnings | implemented |
| creator loop | network access triggers non-deterministic hint in package diagnostics | implemented |
| creator loop | Work check provides experience Port coverage, replacement hints, checkpoint/recovery coverage, and memory/observability hints | implemented |
| creator loop | playable-creation-board package check output is verifiable with expected diagnostic fields | implemented |
| creator loop | third-party playable-seed replaces first-party playable-seed without privilege | implemented |
| capability handles | package load auto-mints capability handles from manifest declarations | implemented |
| capability handles | `authority.handle.attenuate` creates a narrower child handle and cannot expand authority | implemented |
| capability handles | `authority.handle.revoke` immediately invalidates handles and related calls | implemented |
| capability handles | `authority.handle.list` returns current live handles for a package | implemented |
| invoke instrumentation | capability invoke emits `capability/invoked` | implemented |
| invoke instrumentation | successful capability invoke emits `capability/completed` | implemented |
| invoke instrumentation | failed capability invoke emits `capability/failed` | implemented |
| invoke instrumentation | completed/failed events and successful results attach the same EffectReceipt descriptor | implemented |
| bindings | subprocess handshake injects the v1 bindings dictionary | implemented |
| bindings | rust_inproc `ComponentEnv` injects bindings | implemented |
| package | `package.audit_report` / `host.package.audit` reports declared vs used authority | implemented |
| package | `package.path_b_self_contained` validates the `entry.contract: none` self-contained path | implemented |
| git tools | 5 git-tools cases: URL/path validation and signed-tag fixture | implemented |
| integrity | 7 integrity cases: tree hash, manifest hash, GPG verify, fingerprint | implemented |
| install lab | 8+ install-lab cases: resolve_plan, execute_plan, uninstall, list, check_lockfile, cycle detection | implemented |
| install gating | 4 install conformance-gating cases: runs_conformance, strict_conformance_blocks (renamed from the existing blocks shape), lenient_conformance_warns_not_blocks, transitive_propagates | implemented |
| install lab | `install_lab.lenient_conformance_warns_not_blocks` verifies default conformance warnings do not block install | implemented |
| install real smoke | `install.real_github_smoke` opt-in real GitHub smoke | implemented |

## Required rejection conformance for the host

| Area | Required case | Target status |
|---|---|---|
| package execution | `rust_inproc` capability executes through package ABI, not hardcoded id logic | implemented |
| package execution | subprocess package completes JSON-RPC stdio handshake | current host baseline |
| package execution | subprocess timeout/crash/degraded behavior is enforced | current host baseline |
| package execution | package load goes through loading/starting/ready states | implemented |
| capability | anonymous/dev caller behavior is explicitly marked host-only, not package privilege | current host baseline |
| capability | package caller without declared invoke permission is denied | current host baseline |
| capability | version mismatch fails | partial |
| capability | duplicate providers produce ambiguous route unless caller selects provider | implemented |
| capability | unloaded provider cannot be invoked | implemented |
| events | package without `events.read` cannot list events | implemented |
| events | closed session rejects append | implemented |
| events | sequence-range replay works | implemented |
| protocol | HTTP `/rpc` and in-process runtime share authorization behavior | current host baseline |
| protocol | host JSON-RPC stdio transport passes core conformance | current host baseline |
| hooks | hook ordering is stable | implemented |
| hooks | unload removes hook subscribers | implemented |
| hooks | before/after lifecycle hooks are dispatched by kernel operations | partial |
| hooks | package-owned hook handler capability is invoked | implemented |
| schema | manifest schema refs are resolvable | future |
| schema | capability input schema rejects invalid input | implemented |
| schema | capability output schema rejects invalid output | implemented in runtime path |
| schema | event payload schema rejects invalid payload when schema is declared | implemented |
| publisher equality | an `plurora/...` package has no special routing or permissions | implemented |
| publisher equality | kernel starts and conformance passes with no first-party Packages loaded | implemented |

## CLI target output

`cargo run -p plurora-cli -- conformance` should evolve from a smoke test into a named case runner:

```text
session.open_empty                         PASS
event.append_authorized                    PASS
event.append_without_permission_denied     PASS
event.kernel_namespace_denied              PASS
event.read_without_permission_denied       PASS
event.closed_session_rejects_append        PASS
event.range_replay                         PASS
package.load_valid_manifest                PASS
package.unload_removes_capabilities        PASS
capability.invoke_rust_inproc              PASS
capability.ambiguous_provider_denied       PASS
capability.explicit_provider_selected      PASS
first_party.no_privilege                      PASS
schema.capability_input_rejects_invalid    PASS
schema.event_payload_rejects_invalid       PASS
protocol.structured_permission_error       PASS
permission.grant_revoke_audit              PASS
permission.assistant_capability_grant      PASS
protocol.call_host_info                    PASS
protocol.commons_advertised                PASS
protocol.major_mismatch_rejected           PASS
protocol.legacy_adapter_is_explicit        PASS
protocol.reports_are_separate              PASS
protocol.alias_equivalent                  PASS
protocol.layered_namespace_smoke           PASS
protocol.unsupported_version_rejected      PASS
protocol.no_silent_downgrade               PASS
protocol.call_capability_in_process        PASS
principal.package_cannot_self_assert_writer PASS
principal.package_cannot_self_assert_capability_caller PASS
subprocess.load_ready                      PASS
subprocess.invoke_echo                     PASS
package.lifecycle_timeline                 PASS
package.logs_capture                       PASS
package.restart_subprocess                 PASS
host.diagnostics                           PASS
host.profile_autoload                      PASS
surface.contribution_list                  PASS
first_party.foundation_packages               PASS
first_party.assistant_lab_proposal            PASS
play_creation.blank_loop                   PASS
proposal.lifecycle_apply                   PASS
proposal.reject_and_apply_denied           PASS
asset.put_get_list                         PASS
asset.legacy_fnv_migration                 PASS
object_store.portability_integrity         PASS
session.fork_branch                        PASS
projection.rebuild                         PASS
substrate.sqlite_rehydrate                 PASS
subprocess.bad_handshake                   PASS
subprocess.invoke_timeout                  PASS
subprocess.invalid_output_schema           PASS
subprocess.unload_removes_capability       PASS
hook.ordering_stable                       PASS
hook.veto_blocks_event_append              PASS
hook.metadata_mutation_allowed             PASS
hook.package_owned_handler                 PASS
hook.unload_removes_subscription           PASS
package.generated_subprocess_conformance   PASS
package.generated_typescript_subprocess_conformance PASS
package.generated_experience_template      PASS
work.nested_exposure                       PASS
work.digest_is_deterministic               PASS
assembly.component_identity_independent_of_package_envelope PASS
assembly.component_replacement_preserves_content_roots PASS
work.contract_none_is_foreign_capsule      PASS
first_party.asset_lab                         PASS
first_party.projection_lab                    PASS
first_party.playable_seed                     PASS
first_party.persona_lab                       PASS
first_party.knowledge_lab                     PASS
first_party.context_lab                       PASS
first_party.text_transform_lab                PASS
first_party.model_connector_lab               PASS
first_party.model_provider_lab                 PASS
first_party.model_provider_lab_invoke_core       PASS
first_party.model_provider_lab_normalize_stream  PASS
first_party.model_routing_lab                 PASS
first_party.pi_agent_runtime_lab              PASS
first_party.capability_tool_bridge_lab         PASS
first_party.inference_local_lab_describe_capabilities PASS
first_party.inference_local_lab_invoke          PASS
first_party.inference_local_lab_invoke_rejects_http PASS
first_party.inference_local_lab_stream          PASS
first_party.inference_local_lab_explain_error   PASS
first_party.inference_playtest_lab_draft         PASS
first_party.inference_playtest_lab_inspect       PASS
first_party.inference_playtest_lab_reject_apply_denied PASS
first_party.inference_playtest_lab_apply_and_branch PASS
plurora.inference_playtest_lab_no_chat_platform_terms PASS
inproc.non_first_party_preview_rejected       PASS
inproc.unknown_capability_errors           PASS
replacement.thirdparty_seed_surfaces         PASS
replacement.thirdparty_seed_invocation       PASS
replacement.ambiguous_no_publisher_priority   PASS
replacement.work_thirdparty                  PASS
replacement.thirdparty_agent_runtime_surfaces   PASS
replacement.thirdparty_agent_runtime_invocation PASS
replacement.work_agent_runtime_replacement  PASS
substrate.permission_grant_rehydrate          PASS
secret.ref_validation                        PASS
secret.raw_blocked_in_proposal               PASS
secret.raw_blocked_in_asset_metadata         PASS
first_party.no_secret_bypass                    PASS
secret.env_resolver_allowed                  PASS
secret.env_resolver_denied                   PASS
secret.env_resolver_missing_no_leak          PASS
network.no_permission_denied                  PASS
network.allowlisted_host_method_allowed       PASS
network.host_method_mismatch_denied           PASS
network.first_party_no_network_bypass            PASS
network.audit_no_raw_secrets                  PASS
network.policy_pure_function                  PASS
outbound.no_permission_executor_not_called      PASS
outbound.allowlisted_fake_executor              PASS
outbound.raw_body_not_audited                   PASS
outbound.model_provider_shape_fake_executor   PASS
outbound.secret_refs_only                       PASS
outbound.host_mismatch_redirect_denied          PASS
stream.normal_lifecycle                       PASS
stream.cancel_blocks_chunks                   PASS
stream.timeout_blocks_chunks                  PASS
stream.error_terminal                         PASS
stream.non_streaming_rejected                 PASS
stream.no_model_agent_methods                 PASS
stream.protocol_dispatch                      PASS
package.generated_networked_template           PASS
package.generated_streaming_template           PASS
package.faux_model_readiness                   PASS
package.faux_agent_readiness                   PASS
outbound.live_http_default_disabled             PASS
outbound.live_http_rejects_insecure_url         PASS
outbound.live_http_redacted_shape               PASS
outbound.execute_package_allowed                 PASS
outbound.execute_spoofed_package_id_rejected     PASS
outbound.execute_no_permission_denied             PASS
outbound.execute_no_raw_secret_in_response        PASS
outbound.secret_headers_parsed                    PASS
outbound.live_loopback_secret_injection            PASS
stream.sse_normalize_deepseek_canary              PASS
outbound.live_deepseek_opt_in                     PASS
canary.deepseek_profile_shape                     PASS
outbound.openai_chat_loopback                     PASS
outbound.openai_responses_loopback                 PASS
outbound.anthropic_messages_loopback               PASS
outbound.gemini_generate_content_loopback          PASS
outbound.missing_secret_fails_closed               PASS
outbound.provider_normalize_request_alignment      PASS
outbound.no_raw_secret_leak_all_providers          PASS
outbound.static_headers_safe_allowlist             PASS
outbound.static_headers_block_secrets              PASS
outbound.openrouter_loopback_headers               PASS
outbound.xai_loopback                              PASS
outbound.fireworks_loopback                        PASS
stream.deepseek_reasoning_stream                   PASS
stream.openrouter_midstream_error                   PASS
outbound.provider_quirk_fixtures_no_secrets        PASS
outbound.static_headers_openrouter_safe             PASS
agentic_forge.describe_contract                       PASS
agentic_forge.start_run_plan_graph_working_state      PASS
agentic_forge.inspect_cancel_summarize                PASS
agentic_forge.raw_secret_blocked                      PASS
agentic_forge.no_kernel_agent_namespace                PASS
agentic_forge.create_candidate_branch_aware            PASS
agentic_forge.compare_candidate_stale_detection        PASS
agentic_forge.draft_promote_proposal_no_mutation       PASS
agentic_forge.stale_promote_blocked                    PASS
agentic_forge.archive_candidate_target_unchanged       PASS
agentic_forge.inference_node_deterministic_candidate_seed PASS
agentic_forge.replay_match_mismatch_flagged             PASS
agentic_forge.inference_output_privilege_escalation_rejected PASS
agentic_forge.cloud_adapter_needs_host_policy_no_network PASS
agentic_forge.inference_failure_taxonomy_recovery_hints PASS
agentic_forge.explain_tool_call_scoped_no_ambient_authority PASS
agentic_forge.record_observation_untrusted_large_output_redaction PASS
agentic_forge.tool_risk_injection_exfiltration_outbound    PASS
agentic_forge.replay_tool_plan_mismatch_flagged             PASS
agentic_forge.plan_toolchain_requires_explicit_provider_nested_delegation_blocked PASS
agentic_forge.thirdparty_work_shape_no_publisher_priority PASS
agentic_forge.no_publisher_priority_ordinary_package PASS
agentic_forge.hostile_injection_secret_blocked_cross_package PASS
agentic_forge.budget_deadline_contract_cancellation_consistent PASS
agentic_forge.cross_package_replay_mismatch_flagged PASS
playable_board.describe_contract_shape PASS
playable_board.launch_and_player_actions PASS
playable_board.checkpoint_recovery_shape PASS
playable_board.request_change_no_chat PASS
playable_board.bind_agent_run_scoped PASS
playable_board.candidate_proposal_no_target_mutation PASS
playable_board.reject_approve_fork_proof PASS
playable_board.thirdparty_no_publisher_priority PASS
playable_board.no_forbidden_namespace PASS
playable_board.no_raw_secrets PASS
playable_board.content_address_stable PASS
playable_board.checkpoint_metadata PASS
playable_board.provenance_graph PASS
playable_board.state_diff_preview PASS
playable_board.describe_asset_provenance PASS
playable_board.beta2_no_raw_secrets PASS
first_party.asset_lab_content_address PASS
first_party.asset_lab_provenance_graph PASS
first_party.projection_lab_state_snapshot PASS
experience_observability.contract_shape PASS
experience_observability.session_health PASS
experience_observability.package_health PASS
experience_observability.agent_run_health PASS
experience_observability.proposal_causality PASS
experience_observability.cost_latency_summary PASS
experience_observability.failure_breadcrumbs PASS
experience_observability.guardrail_audit_summary PASS
experience_observability.no_forbidden_namespace PASS
experience_observability.no_raw_secrets PASS
memory_lab.contract_shape PASS
memory_lab.record_memory PASS
memory_lab.retrieve_memory PASS
memory_lab.trace_retrieval PASS
memory_lab.draft_update_proposal_only PASS
memory_lab.correction_proposal_gated PASS
memory_lab.forget_redaction_plan PASS
memory_lab.branch_view PASS
memory_lab.no_forbidden_namespace PASS
memory_lab.no_raw_secrets PASS
creator_loop.playable_board_template PASS
creator_loop.playable_experience_template PASS
creator_loop.experience_surface_warnings PASS
creator_loop.missing_checkpoint_warning PASS
creator_loop.dangerous_permissions_warning PASS
creator_loop.network_nondeterministic_hint PASS
creator_loop.work_experience_diagnostics PASS
creator_loop.walkthrough_reference PASS
creator_loop.thirdparty_no_privilege PASS
sharing_lab.contract_shape PASS
sharing_lab.export_work_bundle PASS
sharing_lab.import_work_bundle PASS
sharing_lab.branch_session_bundle PASS
sharing_lab.package_set_lockfile PASS
sharing_lab.compatibility_report PASS
sharing_lab.ai_disclosure_bundle PASS
sharing_lab.read_only_share_manifest PASS
sharing_lab.async_fork_share_plan PASS
sharing_lab.no_marketplace_no_raw_secrets PASS
storage_backend.in_memory_event_store_contract_append_range PASS
storage_backend.sqlite_event_store_contract_append_range PASS
storage_backend.backend_parity_kind_prefix PASS
storage_backend.backend_parity_concurrent_append PASS
storage_backend.backend_parity_subscription PASS
storage_backend.rehydrate_parity PASS
storage_lab.contract_shape_no_kernel_database_terms PASS
storage_lab.backend_classes_no_secret_backend_config PASS
storage_lab.package_state_plan_scoped PASS
storage_lab.put_document_preview_no_write PASS
storage_lab.get_document_preview_no_read PASS
storage_lab.query_prefix_preview_no_query_execution PASS
storage_lab.delete_tombstone_preview_no_delete PASS
storage_lab.export_snapshot_preview_redacted PASS
storage_lab.raw_secret_rejected PASS
storage_lab.unsafe_id_rejected PASS
storage_lab.blob_contract_shape PASS
storage_lab.put_blob_preview_content_address_deterministic PASS
storage_lab.put_blob_preview_no_storage_no_content_event PASS
storage_lab.get_blob_metadata_preview_no_content PASS
storage_lab.export_blob_manifest_refs_only PASS
storage_lab.blob_raw_secret_and_unsafe_id_rejected PASS
storage_lab.projection_contract_shape PASS
storage_lab.projection_materialization_plan_only PASS
storage_lab.projection_query_preview_no_execution PASS
storage_lab.projection_migration_plan_no_rewrite PASS
storage_lab.projection_rejects_raw_secret PASS
storage_lab.projection_no_db_table_leakage PASS
storage_lab.retrieval_contract_shape PASS
storage_lab.multimodal_index_plan_no_embedding_no_storage PASS
storage_lab.multimodal_index_rejects_invalid_modality_or_too_many_refs PASS
storage_lab.vector_search_plan_no_execution PASS
storage_lab.backend_fit_mentions_tdb_future_only PASS
storage_lab.retrieval_rejects_raw_secret PASS
storage_lab.retrieval_no_kernel_vector_namespace_or_credentials PASS
tdb_retrieval_lab.contract_shape PASS
tdb_retrieval_lab.index_plan_no_execution PASS
tdb_retrieval_lab.query_plan_no_execution PASS
tdb_retrieval_lab.backend_fit_boundary PASS
tdb_retrieval_lab.invalid_input_rejected PASS
tdb_retrieval_lab.raw_secret_and_unsafe_id_rejected PASS
tdb_retrieval_lab.real_tdb_opt_in_seam_crate_adapter_available PASS
integrity.tree_hash_deterministic PASS
integrity.tree_hash_excludes_metadata PASS
integrity.manifest_hash_yaml_json_equivalent PASS
integrity.gpg_verify_valid_signature PASS
integrity.gpg_verify_wrong_key_fails PASS
integrity.gpg_verify_invalid_signature_no_panic PASS
integrity.fingerprint_extraction_consistent PASS
git_tools.url_validation_https_only PASS
git_tools.url_validation_no_userinfo PASS
git_tools.path_validation_absolute PASS
git_tools.path_validation_no_traversal PASS
git_tools.read_signed_tag_unsigned PASS
install_lab.resolve_plan_local_source PASS
install_lab.project_root_install_registers_surface_dist PASS
install_lab.resolve_plan_runs_conformance PASS
install_lab.resolve_plan_blocks_when_strict PASS
install_lab.strict_conformance_blocks PASS
install_lab.lenient_conformance_warns_not_blocks PASS
install_lab.transitive_conformance_propagates PASS
install_lab.resolve_plan_with_transitive PASS
install_lab.resolve_plan_cycle_detection PASS
install_lab.execute_plan_local PASS
install_lab.execute_plan_consent_mismatch PASS
install_lab.uninstall_removes_from_profile PASS
install_lab.list_installed_reflects_lockfile PASS
install_lab.check_lockfile_drift_detection PASS
install_lab.check_for_updates_local_dangling_unsupported PASS
install_lab.check_for_updates_external_project_not_applicable PASS
install_lab.update_project_local_replaces_dist_and_lockfile PASS
install_lab.update_project_local_current_noop PASS
install_lab.update_project_local_force_reinstalls_current PASS
install_lab.update_project_external_not_applicable PASS
install_lab.update_project_permission_drift_blocks_before_mutation PASS
install.real_github_smoke PASS
secret_store_resolver.host_profile_installs_composite_resolver PASS
project_secret.put_then_resolve_via_project_ref PASS
project_secret.fallback_to_platform_when_missing PASS
project_secret.no_fallback_when_disabled PASS
project_secret.require_per_project_blocks_fallback PASS
project_secret.isolation_between_projects PASS
project_secret.no_session_context_fails_closed PASS
project_secret.list_returns_names_not_values PASS
project.detect_native_yaml PASS
project.detect_no_yaml PASS
project.detect_invalid_yaml_rejected PASS
project.register_creates_project_dir PASS
project.list_returns_registered PASS
project.state_transitions PASS
project.archive_keeps_data PASS
protocol.project_list_returns_registered_projects PASS
protocol.project_get_returns_full_descriptor PASS
protocol.project_start_transitions_state PASS
protocol.project_methods_require_admin_principal PASS
protocol.project_lifecycle_event_emitted_on_start PASS
surface.resolve_via_dev_path PASS
surface.resolve_via_installed_project PASS
surface.resolve_unknown_fails PASS
surface.resolve_admin_principal_required PASS
project.start_returns_session_id PASS
project.start_idempotent_returns_existing_session PASS
project.session_metadata_carries_project_id PASS
project.stop_closes_session PASS
project.get_returns_running_session_id PASS
tdb_rust_adapter.subprocess_adapter_shell_invokes_disabled_smoke PASS
tdb_rust_adapter.subprocess_adapter_rejects_secret_and_raw_path PASS
tdb_rust_adapter.real_crate_smoke_opt_in PASS
capability_handles.auto_mint PASS
capability_handles.attenuate PASS
capability_handles.revoke PASS
capability_handles.list_for PASS
invoke_instrumentation.invoked_event PASS
invoke_instrumentation.completed_event PASS
invoke_instrumentation.failed_event PASS
bindings.subprocess_injection PASS
bindings.rust_inproc_kernel_env PASS
package.audit_report PASS
package.path_b_self_contained PASS
```

The suite should fail closed: any case listed as required for the host must pass before the corresponding milestone is declared complete.
