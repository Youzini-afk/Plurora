# pi Reference Ledger

> 中文默认说明见 [`README.md`](./README.md).

This directory records how Plurora studies and absorbs [pi](https://github.com/earendil-works/pi) without turning pi into the kernel, protocol, or a first-party privileged path.

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

These can be absorbed into Plurora SDKs, templates, or reference packages now, but must be expressed through Plurora public contract:

- pi-agent-core `AgentEvent` idea: run/turn/message/tool execution lifecycle.
- pi-agent-core `AgentTool` idea: label, argument preparation, execution mode, result/error/terminate.
- `beforeToolCall` / `afterToolCall` policy gates.
- parallel / sequential tool execution policy.
- steer / followUp queue concepts.
- pi-ai faux provider deterministic/scripted testing strategy.

### Package-internal optional

These may be used inside ordinary capability packages, never in constitutional substrate, Host service, or Web shell:

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

These are explicitly out of Plurora agent infrastructure:

- `pi-coding-agent` as the Plurora product shell;
- default bash/read/write/edit tools;
- kernel agent/model/prompt/memory methods;
- private runtime access;
- first-party Package priority;
- raw prompt/response/secret persistence.

## Plurora mapping

| pi-inspired idea | Plurora landing point |
|---|---|
| Agent run | ordinary package capability via `capability.invoke/stream` |
| Abort/cancel | `capability.cancel` |
| Tool | adapter view of Plurora capability |
| Tool execution | `capability.invoke/stream` with explicit provider package |
| Tool gate | permission preview + before/after helper in SDK/package |
| Trace | package-owned events and stream frames |
| Proposal | `platform.proposal.*` |
| State | package-owned assets/projections/capabilities |
| UI | surface contributions consumed by Assist/Forge |

## Verification discipline

Every agent infrastructure update must prove:

- no new `platform.agent.*`, `platform.model.*`, `platform.prompt.*`, `platform.memory.*`, or `platform.turn.*`;
- first-party agent/reference Packages have no priority;
- tool bridge rejects ambiguous providers or requires explicit providers;
- unauthorized tool calls fail and are audited;
- unapproved proposals cannot apply;
- streams cannot append after cancellation;
- raw secrets cannot enter proposals, audit, or trace.
