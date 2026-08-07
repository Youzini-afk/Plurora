# Agentic Forge 包编写指南

> [English](./AGENTIC_FORGE_PACKAGE_AUTHORING.en.md) · [中文](./AGENTIC_FORGE_PACKAGE_AUTHORING.md)

本指南描述当前 Contract V1 的 Agentic Forge Product Profile：如何用普通 Package / Component 构建、运行和替换 agent runtime。它不需要基底或官方实现特权。

## Agentic Forge 是什么

Agentic Forge 是一套可选 agent runtime Profile。当前实现由普通 Component Package 提供：观察分支、维护计划图、调用有范围的能力工具，在 scratch 分支探索并产出候选结果。需要进入目标分支时，它通过可审查的 proposal 请求用户提升。共享语义属于该 Profile，具体 run state 由 Component / Product 拥有；Agent 不进入宪法基底。

## Agentic Forge 不是什么

- 不是 chat shell、coding-agent 克隆或 API gateway
- 不是 kernel `agent.*` 命名空间或协议方法
- 不是拥有 ambient authority 的特权 runtime
- 不是 model provider、prompt manager 或 memory store
- 不是 always-on autonomous agent 或 cost optimizer

## Package-Owned Runs

每个 agent run 都由一个特定包拥有（`owner_package`）。运行生命周期如下：

```
created → prepared → running → paused → waiting_for_approval
                  ↘ completed   ↗
                  ↘ failed      ↗
                  ↘ cancelled   ↗
                  ↘ archived
```

包控制运行。内核没有 `platform.agent.run` 或类似方法。

## Plan Graph

每个运行维护一个计划图：

- 节点有显式 kind：`observe`、`infer`、`tool_call`、`inspect`、`branch_op`、`compare`、`propose`、`wait`
- 边连接节点
- `status`、`revision`、`input_refs`、`output_refs`
- `approval_policy` 和 `retry_policy`
- `deterministic_mode` 标记

计划图是确定性的。它不执行网络。计划可以导出、审查和重放，且没有副作用。

## Scratch Branch / Candidate / Promote

Agent 在 scratch 分支上探索。它永远不直接修改目标分支。

1. `create_candidate` 产生分支感知的候选资产，含 `scratch_branch_ref`、`target_branch_ref`、`changed_asset_refs`、`confidence`、`uncertainty`、`status`。
2. `compare_candidate` 产生差异摘要（scratch vs target）。如果 `target_revision` 不匹配 `current_target_revision`，则 `stale=true`。
3. `draft_promote_proposal` 产生提案草案（包拥有的 `asset.put` 操作），含 `requires_user_approval=true`。它永远不直接修改目标。如果目标已过期，提升会被阻断。
4. `archive_candidate` 将候选结果设置为 `archived`，不修改目标。

## Tool Bridge Scoped Grants

Capability tool bridge（`official/capability-tool-bridge-lab`）提供：

- `explain_tool_call`：有范围的授权摘要，包含分支上下文。`no_execution=true`、`no_ambient_authority=true`。
- `record_tool_observation`：接受不可信工具输出（`untrusted=true`）。大输出会给出 `asset_ref` 建议。Raw secret 会被阻断。
- `summarize_tool_risk`：列出风险类别和对应缓解方式。
- `replay_tool_plan`：按确定性指纹重放。不匹配时标记，绝不静默通过。
- `plan_toolchain`：多步计划。每步必须有 `provider_package_id`。嵌套 delegation 无 `explicit_delegation=true` 时会被阻断。无 promote grant 时也不能写入目标分支。

## Inference Fallback

带推理的 agent run 支持四种 provider：

| Provider | 行为 |
|----------|------|
| `deterministic` | 默认。根据 objective 产生 `candidate_seed` 或 `proposal_seed`。无网络。 |
| `recorded` | 重放 recorded output。指纹不匹配时标记。 |
| `cloud_adapter_plan` | 返回 `needs_host_policy`。不执行网络。 |
| `local_fake` | Fake local inference。`inference_performed=true`，但无网络。 |

推理输出按 allowlist 验证：`candidate_seed`、`proposal_seed`、`observation`、`needs_repair`。被禁止的动作会被拒绝，例如 `privilege_escalation`、`auto_promote` 和 `secret_request`。

## Failure Taxonomy

推理失败会返回明确的 kind 和恢复提示：

`rate_limit` · `quota` · `timeout` · `auth` · `network_denied` · `invalid_output` · `malformed_output` · `replay_mismatch` · `policy_reject`

## Third-Party Replacement

Official agentic-forge-lab 是普通包。它没有内核特权，也没有路由优先级。第三方包可以替换它：

1. 创建具有等价能力形状的包（如 `thirdparty/agentic-forge`）
2. 创建声明 `replacement_candidates: [official/agentic-forge-lab]` 的 composition
3. 两个包都产生包拥有的形状：候选结果、提案、计划图和工作状态
4. 两个包都不能直接修改目标分支或执行网络

参见 `examples/packages/thirdparty-agentic-forge/` 和 `examples/compositions/agentic-forge-replacement/`。

## Secret 安全

- 类似 raw secret 的内容会被阻断，返回 `redaction_state=unsafe_blocked`
- 任何输出不回显 raw secret
- 使用 `secret_ref` 引用代替嵌入 secret
- 推理输出不能请求 secret（`secret_request` 是 forbidden action）

## Budget 和 Deadline

- `describe_contract` 声明 `run_constraints`，含 budget/deadline 支持
- `start_run` 接受可选 `max_steps` 和 `deadline_ms`
- `cancel_run` 产生一致的 `cancelled` 状态，并在追踪事件中包含原因
- 缺少 budget 被诊断，不静默忽略

## Forge Workspace 公共协议

Forge workspace surface（`forge_panel`、`assistant_action`、`home_card`）属于公共协议。任何包都可以贡献到这些 surface slot，包括 official 和 third-party。Runtime 不偏好 official 包。

## TypeScript SDK

`sdk/typescript/agentic-forge/` 提供以下 helper：

- 运行生命周期、计划图、工作状态和候选结果
- Compare、promote、archive 操作
- 推理节点、重放、验证和失败分类
- 工具桥：风险类别、工具调用上下文和工具链步骤
- Secret 安全：`blockRawSecrets`、`looksLikeRawSecret`、`hasKernelAgentNamespace`

自测全部本地运行，不需要网络。

## 非目标

- Always-on autonomous agents
- Provider router 或 cost optimizer
- Multi-model tournament
- Shell/fs/git 默认工具
- 自动权限提升
- 直接 target branch mutation
- Chat/coding-agent 作为主要身份
