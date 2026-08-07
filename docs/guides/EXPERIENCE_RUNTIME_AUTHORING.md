# Experience Runtime 创作指南

> [English](./EXPERIENCE_RUNTIME_AUTHORING.en.md) · [中文](./EXPERIENCE_RUNTIME_AUTHORING.md)

本指南说明如何创建、检查、checkpoint、恢复和替换 Plurora 中的 experience-runtime 包。

## 概述

本指南使用当前 Contract V1 的普通 Package / Component 实现一种 Experience Runtime Product Profile：连续运行、暂停、恢复、checkpoint 和 fork。共享的体验生命周期可以由可选 Protocol / Profile 定义，具体体验状态由 Product 与 Component 拥有；它不进入宪法基底。

关键约束：
- 不新增 `platform.experience.*`、`platform.world.*`、`platform.turn.*`、`platform.chat.*` 或 `platform.memory.*`。
- Experience Package 是普通 Package，没有 substrate 特权。
- 当前 Experience 描述符、状态 projection、checkpoint 和恢复计划由对应 Component / Product 的 Package 分发，是产品工件而不是基底原语。
- 所有行为通过公开协议完成。

## 生成 experience-runtime 包

```bash
plurora init-package ./my-experience \
  --id example/my-experience \
  --entry subprocess \
  --language typescript \
  --template experience-runtime
```

生成结果包含：
- 4 个 surface：`experience_entry`、`play_renderer`、`forge_panel`、`assistant_action`
- 6 个能力：`describe-contract`、`create-checkpoint`、`inspect-checkpoint`、`draft-recovery`、`bind-agent-run`、`echo`
- 无网络声明，无 raw secret，无被禁止的内核命名空间

## Experience 描述符

Experience 描述符（`experience_runtime_descriptor`）是当前 Profile 的产品元数据，由对应 Package 分发，包含：

```typescript
const desc = createExperienceDescriptor({
  package_id: "example/my-experience",
  surfaces: {
    experience_entry: "example/my-experience/entry",
    play_renderer: "example/my-experience/play",
    forge_panel: "example/my-experience/forge",
    assistant_action: "example/my-experience/assist",
  },
  capabilities: {
    describe_contract: "example/my-experience/describe-contract",
    create_checkpoint: "example/my-experience/create-checkpoint",
    inspect_checkpoint: "example/my-experience/inspect-checkpoint",
    draft_recovery: "example/my-experience/draft-recovery",
    bind_agent_run: "example/my-experience/bind-agent-run",
  },
});
```

验证描述符：

```typescript
const errors = validateExperienceDescriptor(desc);
if (errors.length > 0) {
  // 描述符不符合契约
}
```

## State Projection

状态 projection 是体验当前状态的包级别快照：

```typescript
const projection = createStateProjection({
  package_id: "example/my-experience",
  session_id: "session-123",
  state: { health: 100, step_index: 5, location: "forest" },
  capability_id: "example/my-experience/describe-contract",
});
```

## Checkpoint

Checkpoint 是体验状态的持久化快照。它支持三种格式：

| 格式 | 说明 |
|------|------|
| `snapshot` | 完整状态快照 |
| `incremental` | 增量快照（基于前一个 checkpoint） |
| `delta` | 仅存储差异 |

```typescript
const checkpoint = createCheckpoint({
  package_id: "example/my-experience",
  session_id: "session-123",
  state_snapshot: { health: 100, step_index: 5, location: "forest" },
  asset_refs: ["asset:scene:forest", "asset:character:hero"],
  branch_ref: "branch:target:main",
  sequence: 3,
  capability_id: "example/my-experience/create-checkpoint",
});

// 检查 checkpoint
const inspection = inspectCheckpoint(checkpoint);
// inspection.valid === true/false
```

## Recovery

Recovery 是体验失败后的恢复计划。它支持五种策略：

| 策略 | 需要 checkpoint | 需要 approval |
|------|----------------|---------------|
| `restore_last_checkpoint` | 是 | 否 |
| `replay_from_checkpoint` | 是 | 是 |
| `restart_session` | 否 | 是 |
| `manual_intervention` | 否 | 是 |
| `discard_and_reset` | 否 | 是 |

```typescript
const recoveryPlan = draftRecoveryPlan({
  package_id: "example/my-experience",
  session_id: "session-123",
  failure_kind: "state_corruption",
  last_checkpoint_ref: "checkpoint:session-123:3",
  capability_id: "example/my-experience/draft-recovery",
});
```

## Play Surface 订阅

Play surface 支持三种订阅类型：

| 类型 | 说明 |
|------|------|
| `state_change` | 状态变更通知 |
| `checkpoint` | Checkpoint 创建通知 |
| `lifecycle` | 生命周期事件通知 |

```typescript
const subscription = createPlaySurfaceSubscription({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/play",
  subscription_type: "state_change",
  capability_id: "example/my-experience/describe-contract",
});
```

## Forge/Assist 绑定

### Forge 绑定

Forge panel 绑定将 Forge surface 连接到体验会话，支持检查和提案：

```typescript
const forgeBinding = createForgeBinding({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/forge",
  inspect_capabilities: ["example/my-experience/describe-contract"],
  proposal_capabilities: ["example/my-experience/draft-recovery"],
  capability_id: "example/my-experience/bind-agent-run",
});
```

### Assist 绑定

Assist 绑定使用 `fork_then_approve` 策略，确保所有修改都通过提案：

```typescript
const assistBinding = createAssistBinding({
  package_id: "example/my-experience",
  session_id: "session-123",
  surface_id: "example/my-experience/assist",
  action_capabilities: ["example/my-experience/draft-recovery"],
  capability_id: "example/my-experience/bind-agent-run",
});
```

## Agent Run 绑定

Experience 可以通过 `bind_agent_run` 连接到 Agentic Forge：

- Agent run scoped 到分支
- Agent 在 scratch 分支探索
- Agent 产生候选结果和提案，不直接修改目标
- Experience 通过 Forge/Assist 检查、批准或拒绝

## 第三方替换

Experience-runtime Package 是普通 Package。任何满足相同 surface 与 capability 契约的第三方 Package 都可以替换它。替换规则：

- 同等 surface slot（experience_entry、play_renderer、forge_panel、assistant_action）
- 同等能力形状
- 无 publisher priority
- 通过 composition descriptor 声明替换

## 红线

以下行为被严格禁止：

1. 内核体验命名空间：不得在事件、提案、checkpoint 或任何输出中包含 `platform.experience.*`、`platform.world.*`、`platform.turn.*`、`platform.chat.*` 或 `platform.memory.*`。
2. Raw secrets：所有 secret 必须通过 `secret_ref` 引用。不得在 checkpoint、恢复计划或状态 projection 中包含 raw secret。
3. 直接修改目标分支：Agent 对 experience 的修改必须通过提案生命周期，不得直接修改目标分支。
4. 网络访问：Experience-runtime 包默认不出网。如需网络，必须声明 `permissions.network.declarations`。
5. 内核特权：Experience 包不享有任何内核特权。

## TypeScript SDK

`sdk/typescript/experience-runtime` 提供纯 TypeScript SDK。它无依赖，也不暴露私有运行时。

```typescript
import {
  createExperienceDescriptor,
  validateExperienceDescriptor,
  createStateProjection,
  createCheckpoint,
  inspectCheckpoint,
  draftRecoveryPlan,
  createPlaySurfaceSubscription,
  createForgeBinding,
  createAssistBinding,
  blockRawSecrets,
  hasKernelExperienceNamespace,
} from "../../sdk/typescript/experience-runtime/index.js";
```

## 参考实现

- `packages/plurora/experience-runtime-lab/` — 官方 experience-runtime lab 包
- `crates/plurora-runtime/src/inproc/experience_runtime_lab.rs` — 可重放的 in-process handler
- `sdk/typescript/experience-runtime/` — TypeScript SDK
- `docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md` — 本文档

## 延伸阅读

- `docs/CHARTER.md` — 不可变根本原则
- `docs/product/PLAY_CREATION_MODEL.md` — 可选游创 Product Profile 的产品立场与边界
- `docs/guides/AGENTIC_FORGE_PACKAGE_AUTHORING.md` — Agentic Forge 创作指南
- `docs/roadmap/NEXT_STEPS.md` — 路线图
