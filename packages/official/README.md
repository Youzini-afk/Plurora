# Official foundation packages

These packages are infrastructure examples and host tooling batteries. They are not privileged by the kernel.v1.

- `official/package-lab`
- `official/schema-tools`
- `official/event-tools`
- `official/composition-lab`
- `official/asset-lab`
- `official/projection-lab`
- `official/persona-lab`
- `official/knowledge-lab`
- `official/context-lab`
- `official/text-transform-lab`
- `official/model-connector-lab`
- `official/model-provider-lab`
- `official/model-routing-lab`
- `official/inference-local-lab`
- `official/assistant-lab`
- `official/pi-agent-runtime-lab`
- `official/capability-tool-bridge-lab`
- `official/blank-experience`
- `official/playable-seed`

They load through ordinary manifests, provide ordinary capabilities, and contribute ordinary surface descriptors.

`official/composition-lab` explains package compositions, launch plans, permission previews, and surface graphs without private host access.

`official/asset-lab` inspects opaque assets and drafts import/diff plans; asset writes still go through protocol/proposal paths.

`official/projection-lab` explains projection snapshots, diffs, rebuild plans, and source events without private runtime reads.

`official/persona-lab` imports and normalizes persona-like profiles without making chat characters or Tavern cards canonical.

`official/knowledge-lab` normalizes structured knowledge collections, matches entries deterministically, and drafts injection plans without making lorebooks canonical.

`official/context-lab` assembles generic bounded context blocks, reports omissions and budget accounting, and renders templates without model calls or chat ontology.

`official/text-transform-lab` imports, validates, previews, and explains deterministic text transform rules without mutating trusted state.

`official/model-connector-lab` validates provider profiles, masks secret references, and drafts discovery plans without network calls or inference.

`official/model-provider-lab` is a cloud API adapter lab, not the Plurora model abstraction. It builds adapter-local request shapes across eight cloud families (OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, Fireworks), validates profiles rejecting raw secrets, provides fake/local invoke for all eight families with auditable outbound request shapes, normalizes provider stream events (delta SSE, semantic SSE, typed chunk stream) into StreamFrameEnvelope frames, and explains provider errors, all without private kernel privilege.

`official/model-routing-lab` resolves package-owned consumer slots to static model profile route plans with explicit fallbacks and normalized params, without inference.

`official/inference-local-lab` is a deterministic non-HTTP fake local inference provider proof. It proves inference capability seams can work without HTTP, bearer tokens, JSON cloud provider schemas, network access, or secrets. It is not a local model platform.

`official/assistant-lab` intentionally produces proposals that require user approval. It is not a privileged mutation path.

`official/pi-agent-runtime-lab` is a reference agent runtime package. It produces deterministic run plans, trace summaries, proposal drafts, and echo payloads without real model inference or network access. It is not a privileged agent path.

`official/capability-tool-bridge-lab` discovers capabilities, previews permissions, resolves explicit provider selection, and drafts invocation/streaming plans through kernel.v1.capability.invoke/stream. It does not perform real capability calls and gives no priority to official providers.

`official/blank-experience` is a loop fixture, not a canonical game/runtime model.

`official/playable-seed` is a reference playable package. It proves launch/render/inspect/propose flows without becoming a canonical game runtime.
