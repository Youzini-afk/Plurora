# pi integration boundary

> [English](./PI_INTEGRATION.en.md) · [中文](./PI_INTEGRATION.md)

This document fixes the boundary for learning from or hosting agent frameworks such as [pi](https://github.com/earendil-works/pi). pi may be an implementation source for ordinary capability packages or an input to SDK adapters; it is not the Plurora kernel, public contract, or product shell.

## Core stance

Plurora must be able to host, constrain, observe, and replace agent Components and Products without owning an agent ontology. Shared meaning for runs, steps, tools, traces, prompts, models, memory, and coding workflows belongs to optional Protocols and Profiles; concrete state and behavior belong to Components or Products rather than the constitutional substrate.

Agent infrastructure reuses existing public primitives:

- `capability.discover/describe` finds capabilities that may be adapted as tools;
- `capability.invoke/stream/cancel` executes, advances, streams, and cancels work;
- `platform.proposal.*` or the generic Change workflow carries reviewed mutations;
- `journal.event.*` carries traces, tool calls, and run events under the current Package-writer namespace;
- `platform.surface.contribution.*` lets shells discover agent actions, traces, and review panels;
- capability handles, permissions, `secret_ref`, network declarations, outbound audit, and stream ownership constrain effects.

There is no private `platform.agent.*` path and no additional authority for an agent package merely because it is maintained by the project.

## Layered use of pi

| pi layer | Plurora treatment | Boundary |
|---|---|---|
| `pi-ai` | Implementation reference for provider, streaming, and tool-call adapters | Provider semantics stay in ordinary inference/model packages; Host boundaries enforce secrets, network, and audit. |
| `pi-agent-core` | May be wrapped by an SDK or capability package | `AgentEvent`, tool adapters, and steer/follow-up queues may stay package-local; messages, system prompts, and thinking levels do not enter the kernel. |
| `pi-coding-agent` | Reference for a complete product and workflow | TUI, bash/read/write/edit tools, session format, skills, and coding policy are not Plurora platform defaults. |

The detailed upstream ledger is in [`../../integrations/pi/README.md`](../../integrations/pi/README.md).

## Concept mapping

| Agent concept | Plurora public primitive | Rule |
|---|---|---|
| run / turn / step | Component capability call, stream, or protocol-owned state | The substrate gains no agent lifecycle. |
| cancellation | `capability.cancel` | Only caller-owned invocations and streams may be cancelled. |
| tool discovery | `capability.discover/describe` | A tool is an adapter view of a capability. |
| tool execution | `capability.invoke/stream` | Preserve caller, provider, session, permission, and receipt. |
| provider ambiguity | Explicit `provider_package_id` | Never prefer an official provider implicitly. |
| proposed mutation | `platform.proposal.*` / Change workflow | An agent does not directly mutate trusted state. |
| trace | writer-scoped event, stream frame, or artifact | The runtime does not interpret trace payloads. |
| working state | Component or Product event, object, projection, or capability | No substrate agent state is added. |
| model / prompt / memory | Optional Protocols and ordinary Components | They remain composable and replaceable outside the constitutional substrate. |
| UI | Surface contribution + public client | A shell does not read private agent-runtime state. |

## Ordinary repository components

The repository implements and continuously checks this boundary through ordinary SDKs, Component Packages, and integration fixtures:

- `sdk/typescript/agent-adapter` maps Ygg capabilities to pi-style tools;
- `sdk/typescript/agentic-forge` provides package-owned run lifecycle, plan graph, working state, and candidate helpers;
- `official/pi-agent-runtime-lab` is a no-network-by-default reference agent package;
- `official/capability-tool-bridge-lab` handles capability discovery, permission preview, explicit provider selection, and controlled invocation;
- `official/agentic-forge-lab` provides scratch branches, candidates, comparison, promotion, and replay;
- third-party replacement fixtures check that official implementations receive no implicit priority.

Real model outbound execution is already supplied by separate model/inference packages and the Host outbound boundary. An agent package may consume those capabilities when its manifest authority, capability bindings, and user/Host policy allow it, but it may not read raw API keys, bypass network declarations, or persist unredacted prompts and responses in audit records.

## Package and SDK prohibitions

Agent adapters, reference packages, and shell integrations must not:

- import private runtime modules;
- bypass package, capability, permission, proposal, or Change boundaries;
- hardcode official package IDs in the UI as preferred implementations;
- expose raw secrets in events, proposals, receipts, or audit;
- provide unrestricted bash/edit/write or arbitrary remote shell by default;
- treat caller-supplied sessions, targets, paths, or network destinations as authorization evidence;
- make agent traces, prompts, or model taxonomies into kernel schemas.

## Kernel non-goals

The kernel does not add or standardize:

- `platform.agent.*`
- `platform.model.*`
- `platform.prompt.*`
- `platform.memory.*`
- `platform.turn.*`
- agent state, chat transcripts, prompt templates, provider registries, thinking/reasoning, or memory taxonomies.

Those concepts may be defined by optional Protocols, implemented by Components, and composed by Products. Concrete status and construction direction live in [`../ALPHA_STATUS.md`](../ALPHA_STATUS.en.md) and [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.en.md).
