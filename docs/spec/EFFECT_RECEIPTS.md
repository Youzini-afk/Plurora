# EffectReceipt

状态：Experimental，schema version 1。

`EffectReceipt` 是一次已经发生或已经被拒绝的 effect 的内容寻址证据。它不是执行计划、日志正文或领域对象；它只保存重放与审计所需的摘要和 object/artifact 引用。

## Artifact profile

- type URI：`urn:plurora:effect-receipt:v1`
- media type：`application/vnd.plurora.effect-receipt+json;version=1`
- digest：ObjectStore 计算的 canonical `sha256:<hex>`
- schema：[`v1/schemas/effect-receipt.schema.json`](v1/schemas/effect-receipt.schema.json)

Receipt 自身是 `ArtifactDescriptor`。它引用 component evidence、input/output objects、external-effect summaries、authority、policy decision、approval 和 parent receipts。ObjectStore 对引用对象执行 digest/size 校验；缺失或损坏对象不能被替换为空值。

## Terminal model

终态明确区分：

- `succeeded`
- `denied`
- `failed`
- `cancelled`
- `timed_out`
- `partial`

当前覆盖：

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

`Runtime::replay_effect_receipt(digest)` is historical replay. It reads the receipt and recorded output refs from ObjectStore and does not resolve a provider or call an outbound/exec executor. A missing receipt or referenced output fails explicitly with `incomplete history`.

`Runtime::replay_capability_receipt(digest)` reconstructs a successful capability result from recorded output. It continues to work when the provider package is unloaded.

`Runtime::reexecute_capability_receipt(context, digest)` is intentionally different: it reads the recorded input, forks the original session, resolves the current provider implementation, invokes it on the child session, and writes a new receipt whose `replay_mode` is `reexecute` and whose `parent_receipts` contains the source digest. The old receipt and branch remain unchanged.

## Data minimization and secret handling

Receipts never inline raw HTTP bodies, headers, credentials, full prompts, full user content, WebSocket frames, process arguments, environment values, or log text. They retain safe counts, status, destination/component identity, resource limits, usage/cost summaries, and descriptors.

Referenced JSON values pass through the strict effect-evidence scanner, including content/body/text-like fields that the general asset scanner intentionally excludes. Obvious secret-bearing fields and values are replaced with `<secret:redacted>` before commit, including values nested in arrays. Receipt envelopes are scanned again before being written. `secret_ref` identifiers may be retained as authority evidence; resolved secret values may not.

Historical replay therefore returns the recorded redacted representation when policy prevented raw material from being persisted.

## Failure behavior

- Receipt object missing or digest/size mismatch: explicit incomplete/corrupt history error.
- Executor/provider unavailable during historical replay: irrelevant; no executor/provider is called.
- Executor/provider unavailable during re-execution: the new branch remains distinct and the re-execution fails without mutating the old receipt.
- A partially applied Change produces a `partial` commit receipt with completed operation receipts as parents.
- Live exec executors are actively monitored by the runtime, so natural exit and timeout produce a terminal receipt without caller polling. Repeated polling, stop races, and post-restart status reads do not create another terminal receipt after one was recorded.

## Executable evidence

The conformance suite covers capability success/failure receipts, provider-free historical replay, outbound replay with every executor disabled, change authority and commit/rejection/failure receipts, active local-exec terminal monitoring plus restart deduplication, WebSocket timeout/cancellation linkage, missing-object errors, and raw-secret redaction including content fields.
