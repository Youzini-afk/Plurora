# Plurora first-party packages

These packages are infrastructure examples and host tooling batteries. They are not privileged by the platform.

- `plurora/package-lab`
- `plurora/schema-tools`
- `plurora/event-tools`
- `plurora/asset-lab`
- `plurora/projection-lab`
- `plurora/persona-lab`
- `plurora/knowledge-lab`
- `plurora/context-lab`
- `plurora/text-transform-lab`
- `plurora/model-connector-lab`
- `plurora/model-provider-lab`
- `plurora/model-routing-lab`
- `plurora/inference-local-lab`
- `plurora/assistant-lab`
- `plurora/pi-agent-runtime-lab`
- `plurora/capability-tool-bridge-lab`
- `plurora/blank-experience`
- `plurora/playable-seed`

They load through ordinary manifests, provide ordinary capabilities, and contribute ordinary surface descriptors.

`plurora/asset-lab` inspects opaque assets and drafts import/diff plans; asset writes still go through protocol/proposal paths.

`plurora/projection-lab` explains projection snapshots, diffs, rebuild plans, and source events without private runtime reads.

`plurora/persona-lab` imports and normalizes persona-like profiles without making chat characters or Tavern cards canonical.

`plurora/knowledge-lab` normalizes structured knowledge collections, matches entries deterministically, and drafts injection plans without making lorebooks canonical.

`plurora/context-lab` assembles generic bounded context blocks, reports omissions and budget accounting, and renders templates without model calls or chat ontology.

`plurora/text-transform-lab` imports, validates, previews, and explains deterministic text transform rules without mutating trusted state.

`plurora/model-connector-lab` validates provider profiles, masks secret references, and drafts discovery plans without network calls or inference.

`plurora/model-provider-lab` is a cloud API adapter lab, not the Plurora model abstraction. It builds adapter-local request shapes across eight cloud families (OpenAI, Anthropic, Gemini, OpenAI-compatible, OpenRouter, DeepSeek, xAI, Fireworks), validates profiles rejecting raw secrets, provides fake/local invoke for all eight families with auditable outbound request shapes, normalizes provider stream events (delta SSE, semantic SSE, typed chunk stream) into StreamFrameEnvelope frames, and explains provider errors, all without private platform privilege.

`plurora/model-routing-lab` resolves package-owned consumer slots to static model profile route plans with explicit fallbacks and normalized params, without inference.

`plurora/inference-local-lab` is a deterministic non-HTTP fake local inference provider proof. It proves inference capability seams can work without HTTP, bearer tokens, JSON cloud provider schemas, network access, or secrets. It is not a local model platform.

`plurora/assistant-lab` intentionally produces proposals that require user approval. It is not a privileged mutation path.

`plurora/pi-agent-runtime-lab` is a reference agent runtime package. It produces deterministic run plans, trace summaries, proposal drafts, and echo payloads without real model inference or network access. It is not a privileged agent path.

`plurora/capability-tool-bridge-lab` discovers capabilities, previews permissions, resolves explicit provider selection, and drafts invocation/streaming plans through capability.invoke/stream. It does not perform real capability calls and gives no priority to first-party providers.

`plurora/blank-experience` is a loop fixture, not a canonical game/runtime model.

`plurora/playable-seed` is a reference playable package. It proves launch/render/inspect/propose flows without becoming a canonical game runtime.
