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

### Installation model conformance cases

These cases cover the Installation journal, secret policy, and destructive no-alias boundary. Verify case ids with `cargo run -p plurora-cli -- conformance --list | grep -E "(installation|installation_secret)"`.

| Group | Case id | Coverage | Status |
|---|---|---|---|
| Installation protocol | `installation.protocol_crud_uses_service_registry` | five RPCs use the same Service registry owner | implemented |
| Installation idempotency | `installation.idempotency_replay_and_conflict` | same-fingerprint replay and different-fingerprint conflict | implemented |
| Installation concurrency | `installation.stale_revision_rejected` | stale expected revision rejects | implemented |
| Installation state | `installation.state_migration_explicit` | incompatible state requires migration/reset | implemented |
| Installation remove | `installation.remove_keep_delete_distinct` | keep/delete are distinct explicit terminals | implemented |
| Installation restart | `installation.journal_restart_rehydrate` | journal rebuilds projection after restart | implemented |
| Installation authority | `installation.exact_authority` | exact resource selectors prevent cross-resource access | implemented |
| Installation events | `installation.lifecycle_events` | created/updated/removed payloads match public schemas | implemented |
| destructive identity | `installation.retired_methods_invalid_request` | retired methods have no aliases | implemented |
| Installation secret | `installation_secret.put_resolve_owned_path` | reads/writes only the Installation-owned secret path | implemented |
| Installation secret | `installation_secret.policy_fallback_and_retired_scheme` | policy controls fallback and rejects the retired scheme | implemented |
| Installation secret | `installation_secret.isolation_and_list_redaction` | Installation isolation and value-free list output | implemented |

### End-to-end real-path conformance cases

These cases cover surface resolution and the Run entry boundary. Phase 4 now provides Run lifecycle through an independent Run journal; surface resolution must not bypass explicit Run authority.

| Group | Case id | Coverage | Status |
|---|---|---|---|
| dev bundle | `surface.resolve_via_dev_path` | dev-path surface bundle resolution | implemented |
| bundle rejection | `surface.resolve_unknown_fails` | unknown surface bundle fails closed | implemented |
| bundle authority | `surface.resolve_admin_principal_required` | resolve_bundle restricted to HostAdmin/HostDev | implemented |

Surface/static bundle and bridge coverage also includes these stable assertions:

| Assertion | Coverage | Status |
|---|---|---|
| static surface bundle | `surface_bundle` is a static browser entry and does not use the wasm sentinel or package execution | implemented |
| Package surface source | bundles resolve from Host-identity-gated `/surface-bundles/packages/<package-id>/...` | implemented |
| sandbox asset attenuation | authorized resolution issues a five-minute `/surface-assets/<lease>/...` handle bound to the grant/bundle root; cross-root, forged, expired, or revoked-grant access is denied | implemented |
| bridge allowlist | typed `allowed_capability_ids` precisely constrains callable surface-bridge capabilities | implemented |
| metadata not authority | surface metadata describes entries only and does not grant authority | implemented |
| stream ownership | stream subscribe/unsubscribe is bound to the owning surface and session | implemented |
| redacted diagnostics | bridge diagnostics, errors, and logs do not leak raw secrets or host absolute paths | implemented |
| uncontrolled secret input | secret inputs remain uncontrolled/short-lived and are cleared on close | implemented |
| schema timestamp stability | schema/export timestamps are stable and avoid nondeterministic generated timestamps | implemented |
| content-addressed freshness | bundle bytes enter the artifact closure, so changes alter the digest | implemented |
| no implicit Run | Installation detail returns only projection/affordances; it creates no Run or session, and start must use `host.run.start` | implemented |


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

## Named CLI cases

`cargo run -p plurora-cli -- conformance --list` is the executable source of truth for named cases; it currently emits 453 case ids plus tags. This document keeps only coverage that changes architectural judgment instead of duplicating a complete list that drifts.

The runner supports `--case`, `--tag`, `--fail-fast`, and `--slowest`. Every Host-required case must pass before the corresponding milestone is complete.
