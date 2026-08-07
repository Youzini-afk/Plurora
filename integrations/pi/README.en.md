# pi Reference Ledger

> 中文默认说明见 [`README.md`](./README.md).

This directory records how Plurora studies and absorbs [pi](https://github.com/earendil-works/pi) without turning pi into the kernel, protocol, or an official privileged path.

## Upstream snapshot

- Upstream repo: https://github.com/earendil-works/pi
- Upstream project: `pi` agent harness mono repo
- License: MIT
- Key packages:
  - `@earendil-works/pi-ai`
  - `@earendil-works/pi-agent-core`
  - `@earendil-works/pi-coding-agent`

## Absorption tiers

### Adapter-now

These can be absorbed into Ygg SDKs, templates, or reference packages now, but must be expressed through Ygg public protocol:

- pi-agent-core `AgentEvent` idea: run/turn/message/tool execution lifecycle.
- pi-agent-core `AgentTool` idea: label, argument preparation, execution mode, result/error/terminate.
- `beforeToolCall` / `afterToolCall` policy gates.
- parallel / sequential tool execution policy.
- steer / followUp queue concepts.
- pi-ai faux provider deterministic/scripted testing strategy.

### Package-internal optional

These may be used inside ordinary capability packages, never in kernel/v1/service/web shell:

- `pi-agent-core` agent loop.
- `pi-ai` faux provider.
- `pi-ai` stream/tool-call event shapes.

### Reference only

These are design references only:

- pi-coding-agent session tree / fork / compaction.
- pi-coding-agent resource loading / skills / extension organization.
- pi-coding-agent model resolver / provider display names.
- pi-ai provider lazy registration and provider onboarding checklist.
- pi TUI / web-ui display experience.

### Deferred

These are deferred until dedicated packages and host policy are ready:

- real provider calls;
- OAuth / API key login;
- provider discovery / model catalog;
- real model inference / streaming inference;
- multi-agent orchestration / director / planner graph.

### Rejected

These are explicitly out of Ygg agent infrastructure:

- `pi-coding-agent` as the Ygg product shell;
- default bash/read/write/edit tools;
- kernel agent/model/prompt/memory methods;
- private runtime access;
- official package priority;
- raw prompt/response/secret persistence.

## Ygg mapping

| pi-inspired idea | Ygg landing point |
|---|---|
| Agent run | ordinary package capability via `kernel.v1.capability.invoke/stream` |
| Abort/cancel | `kernel.v1.capability.cancel` |
| Tool | adapter view of Ygg capability |
| Tool execution | `kernel.v1.capability.invoke/stream` with explicit provider package |
| Tool gate | permission preview + before/after helper in SDK/package |
| Trace | package-owned events and stream frames |
| Proposal | `kernel.v1.proposal.*` |
| State | package-owned assets/projections/capabilities |
| UI | surface contributions consumed by Assist/Forge |

## Verification discipline

Every agent infrastructure update must prove:

- no new `kernel.v1.agent.*`, `kernel.v1.model.*`, `kernel.v1.prompt.*`, `kernel.v1.memory.*`, or `kernel.v1.turn.*`;
- official agent/reference packages have no priority;
- tool bridge rejects ambiguous providers or requires explicit providers;
- unauthorized tool calls fail and are audited;
- unapproved proposals cannot apply;
- streams cannot append after cancellation;
- raw secrets cannot enter proposals, audit, or trace.
