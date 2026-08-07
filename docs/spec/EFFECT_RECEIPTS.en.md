# EffectReceipt

Status: Experimental, schema version 1.

An `EffectReceipt` is content-addressed evidence for an effect that happened or was denied. It is not an execution plan, log body, or domain object. It stores only the summaries and object/artifact references required for replay and audit.

## Artifact profile

- type URI: `urn:plurora:effect-receipt:v1`
- media type: `application/vnd.plurora.effect-receipt+json;version=1`
- digest: canonical `sha256:<hex>` computed by ObjectStore
- schema: [`v1/schemas/effect-receipt.schema.json`](v1/schemas/effect-receipt.schema.json)

The receipt itself is an `ArtifactDescriptor`. It references component evidence, input/output objects, external-effect summaries, authority, policy decisions, approvals, and parent receipts. ObjectStore verifies referenced object digests and sizes; missing or corrupt data is never replaced with an empty value.

## Terminal model

Terminal outcomes are distinct:

- `succeeded`
- `denied`
- `failed`
- `cancelled`
- `timed_out`
- `partial`

Current coverage:

| Effect path | Receipt kind |
| --- | --- |
| capability invoke | `capability.invoke` |
| capability stream terminal | `capability.stream` |
| outbound policy denial | `outbound.policy` |
| outbound HTTP | `outbound.execute` |
| outbound stream | `outbound.stream` |
| WebSocket open/send/close | `outbound.websocket.*` |
| WebSocket connection terminal | `outbound.websocket` |
| local exec start/stop/status terminal | `exec.start`, `exec.stop`, `exec.status`, `exec.run` |
| Proposal change operation/commit | `change.operation`, `change.commit`, `change.policy` |

Existing v1 terminal events remain compatible and gain an additive `receipt` descriptor. Capability results also expose `receipt` and `replay_mode`.

## Replay

`Runtime::replay_effect_receipt(digest)` performs historical replay. It reads the receipt and recorded output references from ObjectStore without resolving a provider or calling an outbound/exec executor. A missing receipt or referenced output fails explicitly with `incomplete history`.

`Runtime::replay_capability_receipt(digest)` reconstructs a successful capability result from its recorded output and still works when the provider package is unloaded.

`Runtime::reexecute_capability_receipt(context, digest)` is intentionally different. It reads the recorded input, forks the original session, resolves the current provider implementation, invokes it on the child session, and writes a new receipt with `replay_mode: reexecute` and the source digest in `parent_receipts`. The old receipt and branch remain unchanged.

## Data minimization and secret handling

Receipts do not inline raw HTTP bodies, headers, credentials, full prompts, full user content, WebSocket frames, process arguments, environment values, or log text. They retain safe counts, status, destination/component identity, resource limits, usage/cost summaries, and descriptors.

Referenced JSON values pass through the strict effect-evidence scanner, including content/body/text-like fields that the general asset scanner intentionally excludes. Obvious secret-bearing fields and values are replaced with `<secret:redacted>` before commit, including values nested in arrays. Receipt envelopes are scanned again before storage. `secret_ref` identifiers may remain as authority evidence; resolved secret values may not.

Historical replay therefore returns the recorded redacted representation whenever policy prevented raw material from being persisted.

## Failure behavior

- Missing receipt objects or digest/size mismatches produce explicit incomplete/corrupt-history errors.
- Provider/executor availability is irrelevant to historical replay because no provider/executor is called.
- Provider/executor failure during re-execution affects only the new branch and cannot mutate the old receipt.
- A partially applied Change produces a `partial` commit receipt whose parents include completed operation receipts.
- Live exec executors are actively monitored by the runtime, so natural exit and timeout produce a terminal receipt without caller polling. Repeated polling, stop races, and post-restart status reads do not create another terminal receipt after one has been recorded.

## Executable evidence

The conformance suite covers capability success/failure receipts, provider-free historical replay, outbound replay with every executor disabled, change authority and commit/rejection/failure receipts, active local-exec terminal monitoring plus restart deduplication, WebSocket timeout/cancellation linkage, missing-object errors, and raw-secret redaction including content fields.
