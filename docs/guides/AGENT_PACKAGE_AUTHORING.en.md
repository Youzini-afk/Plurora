# Agent Package Authoring Guide

> [English](./AGENT_PACKAGE_AUTHORING.en.md) · [中文](./AGENT_PACKAGE_AUTHORING.md)

This guide explains how to implement agent-like capability with ordinary Packages and Components under the current Contract V1. Shared Agent meaning may be defined by optional Protocols or Profiles, while concrete run state belongs to a Component or Product rather than the constitutional substrate.

## What to use

- Declare agent-like capabilities in ordinary manifests.
- Start runs through `kernel.v1.capability.invoke` or `kernel.v1.capability.stream`.
- Cancel streaming invocations through `kernel.v1.capability.cancel`.
- Produce, approve, and apply changes through `kernel.v1.proposal.*`.
- Record traces through the current Package-writer namespace, Artifacts, or stream frames.
- Expose `assistant_action`, `forge_panel`, or `home_card` through surface contributions.
- Use `secret_ref` instead of raw secrets.
- Use explicit `provider_package_id` when providers conflict.

## What not to use

- Do not add or depend on `kernel.v1.agent.*`, `kernel.v1.model.*`, `kernel.v1.prompt.*`, `kernel.v1.memory.*`, or `kernel.v1.turn.*`.
- Do not store agents directly in kernel state.
- Do not let agents mutate trusted asset/projection/session state directly; create proposals first.
- Do not borrow another package's permissions through a tool bridge.
- Do not automatically select official providers.
- Do not store raw secrets in traces, proposals, events, audit records, or errors.
- Do not provide bash/read/write/edit coding-agent tools by default.

## Start from the template

Generate a local, replayable agent runtime package:

```bash
cargo run -p ygg-cli -- init-package /tmp/ygg-agent \
  --id example/agent-runtime \
  --entry subprocess \
  --language typescript \
  --template agent-runtime
```

The template generates:

- `example/agent-runtime/run`: streaming run capability.
- `example/agent-runtime/explain-run`: run trace explanation.
- `example/agent-runtime/draft-proposal`: approval-gated proposal draft.
- `example/agent-runtime/echo`: local conformance compatibility capability.
- `assistant_action` and `forge_panel` surfaces.
- defaults that avoid network calls, real model calls, and raw secrets.

Validate the generated package:

```bash
cargo run -p ygg-cli -- package check /tmp/ygg-agent/manifest.yaml
cargo run -p ygg-cli -- package conformance /tmp/ygg-agent/manifest.yaml
```

## Use the `ygg-agent-adapter` SDK

`sdk/typescript/ygg-agent-adapter` is a thin adapter, not a full agent framework. It helps you:

- map Ygg capability descriptors to pi-style tool descriptors;
- build `kernel.v1.capability.invoke` / `kernel.v1.capability.stream` request payloads;
- build package-owned trace event payloads;
- build approval-gated proposal draft payloads;
- diagnose provider ambiguity, permission previews, and raw-secret blocking.

Example:

```ts
import { createYggAgentAdapter } from "../../sdk/typescript/ygg-agent-adapter/index.js";

const adapter = createYggAgentAdapter({
  protocolClient,
  packageId: "example/agent-runtime",
});

const tool = adapter.createCapabilityTool({
  capability_id: "example/tool/plan",
  provider_package_ids: ["example/tool"],
  streaming: false,
});

const plan = await adapter.invokeCapabilityTool(tool, {
  input: { topic: "safe plan" },
  provider_package_id: "example/tool",
});
```

If multiple providers expose the same capability, explicitly choose `provider_package_id`. Do not choose the first provider automatically and do not prefer `official/*`.

## Official reference packages

`official/pi-agent-runtime-lab` is an ordinary reference package. It provides local, replayable capabilities:

- run plans;
- trace summaries;
- proposal drafts;
- echo.

It has no official privilege. It is not a real agent runtime and performs no model inference.

`official/capability-tool-bridge-lab` is also an ordinary package. It only produces tool discovery, permission previews, and invocation plans. It does not execute target capabilities on behalf of agents, which avoids confused-deputy behavior.

## Third-party replacement proof

See:

- `examples/packages/thirdparty-agent-runtime/manifest.yaml`
- `examples/compositions/agent-runtime-replacement/composition.yaml`

This example shows that a third-party agent runtime can expose equivalent surface, capability, proposal, and trace shapes. The official package is only a `replacement_candidate`, not a priority path.

Validate it:

```bash
cargo run -p ygg-cli -- package check examples/packages/thirdparty-agent-runtime/manifest.yaml
cargo run -p ygg-cli -- composition check examples/compositions/agent-runtime-replacement/composition.yaml
```

## UI observability

Forge's Agent Observability section and the Assist Drawer Agent Readiness panel derive information only from public protocol data: surface contributions, capabilities, events, and proposals. They do not hardcode official packages and do not start real agents or models.

## Relationship to pi

[pi](https://github.com/earendil-works/pi) is a reference source:

- `pi-agent-core` event/tool/gate/queue ideas may be absorbed inside ordinary packages.
- `pi-ai` faux provider and stream shapes may inform future model packages.
- `pi-coding-agent` is only a product and observability reference; it is not embedded into Yggdrasil.

For more boundaries, see [`../architecture/PI_INTEGRATION.md`](../architecture/PI_INTEGRATION.en.md) and [`../../integrations/pi/README.md`](../../integrations/pi/README.en.md).
