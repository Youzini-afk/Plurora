# Agentic Forge Package Authoring Guide

> [English](./AGENTIC_FORGE_PACKAGE_AUTHORING.en.md) · [中文](./AGENTIC_FORGE_PACKAGE_AUTHORING.md)

This guide describes the current Contract V1 Agentic Forge Product Profile: how to build, run, and replace an agent runtime through ordinary Packages and Components. It requires no substrate or first-party-implementation privilege.

## What Agentic Forge Is

Agentic Forge is an optional agent-runtime Profile. The current implementation is provided by ordinary Component Packages: it observes branches, maintains plan graphs, calls scoped capability tools, explores scratch branches, and produces candidates. When a change should reach the target branch, it asks the user through an inspectable proposal. Shared meaning belongs to the Profile and concrete run state to the Component or Product; Agent does not enter the constitutional substrate.

## What Agentic Forge Is Not

- Not a chat shell, coding-agent clone, or API gateway
- Not a kernel `agent.*` namespace or protocol method
- Not a privileged runtime with ambient authority
- Not a model provider, prompt manager, or memory store
- Not an always-on autonomous agent or cost optimizer

## Package-Owned Runs

Every agent run is owned by a specific package (`owner_package`). The run lifecycle is:

```
created → prepared → running → paused → waiting_for_approval
                  ↘ completed   ↗
                  ↘ failed      ↗
                  ↘ cancelled   ↗
                  ↘ archived
```

The package controls the run. The kernel does not have `platform.agent.run` or similar methods.

## Plan Graph

Each run maintains a plan graph with:

- nodes of explicit kinds: `observe`, `infer`, `tool_call`, `inspect`, `branch_op`, `compare`, `propose`, `wait`
- edges connecting nodes
- `status`, `revision`, `input_refs`, `output_refs`
- `approval_policy` and `retry_policy`
- `deterministic_mode` flag

Plan graphs are deterministic. No network is performed. Plans are exported, inspected, and replayed without side effects.

## Scratch Branch / Candidate / Promote

Agents explore on scratch branches. They never modify the target branch directly.

1. `create_candidate` produces a branch-aware candidate artifact with `scratch_branch_ref`, `target_branch_ref`, `changed_asset_refs`, `confidence`, `uncertainty`, and `status`.
2. `compare_candidate` produces a diff summary (scratch vs target). If `target_revision` does not match `current_target_revision`, `stale=true`.
3. `draft_promote_proposal` produces a proposal draft with package-owned `asset.put` operations and `requires_user_approval=true`. It never mutates the target directly. If the target is stale, promotion is blocked.
4. `archive_candidate` sets the candidate to `archived` without modifying the target.

## Tool Bridge Scoped Grants

The capability tool bridge (`plurora/capability-tool-bridge-lab`) provides:

- `explain_tool_call`: scoped grant summary with branch-aware context. `no_execution=true`, `no_ambient_authority=true`.
- `record_tool_observation`: accepts untrusted tool output (`untrusted=true`). Large output gets an `asset_ref` recommendation. Raw secrets are blocked.
- `summarize_tool_risk`: lists risk categories and their mitigations.
- `replay_tool_plan`: deterministic fingerprint replay. Mismatches are flagged, never silently passed.
- `plan_toolchain`: multi-step plan. Each step must have `provider_package_id`. Nested delegation without `explicit_delegation=true` is blocked. Target branch writes without a promote grant are blocked.

## Inference Fallback

Inference-backed agent runs support 4 provider kinds:

| Provider | Behavior |
|----------|----------|
| `deterministic` | Default. Produces `candidate_seed` or `proposal_seed` based on objective. No network. |
| `recorded` | Replays recorded output. Fingerprint mismatch is flagged. |
| `cloud_adapter_plan` | Returns `needs_host_policy`. No network performed. |
| `local_fake` | Fake local inference. `inference_performed=true`, but no network. |

Inference output is validated against an allowlist: `candidate_seed`, `proposal_seed`, `observation`, `needs_repair`. Forbidden actions such as `privilege_escalation`, `auto_promote`, and `secret_request` are rejected.

## Failure Taxonomy

Inference failures return explicit kinds and recovery hints:

`rate_limit` · `quota` · `timeout` · `auth` · `network_denied` · `invalid_output` · `malformed_output` · `replay_mismatch` · `policy_reject`

## Third-Party Replacement

The first-party `plurora/agentic-forge-lab` is an ordinary Package. It has no kernel privilege and no routing priority. Third-party packages can replace it:

1. Create a package with equivalent capability shapes (e.g., `thirdparty/agentic-forge`)
2. Create a composition that declares `replacement_candidates: [plurora/agentic-forge-lab]`
3. Both packages produce package-owned shapes: candidates, proposals, plan graphs, working state
4. Neither package may directly mutate target branches or perform network

See `examples/packages/thirdparty-agentic-forge/` and `examples/compositions/agentic-forge-replacement/`.

## Secret Safety

- Raw-secret-like content is blocked with `redaction_state=unsafe_blocked`
- No raw secret echo in any output
- Use `secret_ref` references instead of embedding secrets
- Inference output cannot request secrets (`secret_request` is a forbidden action)

## Budget and Deadline

- `describe_contract` declares `run_constraints` with budget/deadline support
- `start_run` accepts optional `max_steps` and `deadline_ms`
- `cancel_run` produces consistent `cancelled` state with trace events including reason
- Missing budget is diagnosed, not silently ignored

## Forge Workspace Public Protocol

The forge workspace surfaces (`forge_panel`, `assistant_action`, `home_card`) are public protocol. Any Package can contribute to these surface slots — first-party or third-party. The runtime does not prefer first-party implementations.

## TypeScript SDK

`sdk/typescript/agentic-forge/` provides helpers for:

- Run lifecycle states, plan graph, working state, candidates
- Compare, promote, archive operations
- Inference node, replay, validation, failure taxonomy
- Tool bridge: risk categories, tool call context, toolchain steps
- Secret safety: `blockRawSecrets`, `looksLikeRawSecret`, `hasKernelAgentNamespace`

Self-tests run locally and require no network.

## Non-Goals

- Always-on autonomous agents
- Provider router or cost optimizer
- Multi-model tournament
- Shell/fs/git default tools
- Automatic permission escalation
- Direct target branch mutation
- Chat/coding-agent primary identity
