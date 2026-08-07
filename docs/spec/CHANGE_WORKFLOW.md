# Change workflow 与 Proposal adapter

> [English](./CHANGE_WORKFLOW.en.md) · [中文](./CHANGE_WORKFLOW.md)

状态：Experimental，schema version 1。

通用 `Intent → ChangeSet → PolicyDecision → Commit` 数据链描述改变的目标、操作、授权判断与实际提交结果，不向 kernel 引入世界、对话、模型或内容领域语义。

## Schemas 与 type URI

| Primitive | Type URI / schema |
| --- | --- |
| `Intent` | `urn:yggdrasil:intent:v1` / [`intent.schema.json`](v1/schemas/intent.schema.json) |
| `ChangeSet` | `urn:yggdrasil:change-set:v1` / [`change-set.schema.json`](v1/schemas/change-set.schema.json) |
| `PolicyDecision` | `urn:yggdrasil:policy-decision:v1` / [`policy-decision.schema.json`](v1/schemas/policy-decision.schema.json) |
| `ChangeCommit` | `urn:yggdrasil:change-commit:v1` / [`commit.schema.json`](v1/schemas/commit.schema.json) |

`ChangeOperation` 保持开放的 `op`、`target`、`input_refs` 和 `payload` 结构；kernel 不维护封闭的领域操作枚举。

## v1 Proposal adapter

现有 `kernel.v1.proposal.*` 方法和事件保持不变。创建 Proposal 时 adapter 添加：

- `intent`：创建者、目标 session/branch 与 goal；
- `change_set`：映射旧 `operations`、`required_permissions`、`expected_effects`；
- `policy_decision`：初始为 `requires_approval`；
- `commit`：apply 终态后生成；
- `receipt`：拒绝、失败、部分完成或成功的 terminal evidence。

Approve 需要 `change.proposal.approve`，reject 需要 `change.proposal.reject`。HostAdmin/HostDev 内建满足这些检查，其他 principal 必须持有按 proposal 限定的显式 grant。Approve 将 decision 更新为 `allowed`；reject 更新为 `denied` 并生成 `change.policy` receipt。

## Apply lifecycle

Apply 使用以下顺序：

1. 在 proposal registry 中以原子写锁把 `approved` 占位成 `applying`，阻止并发重复 apply。
2. 对执行 principal 重新检查 `change.proposal.apply` 和旧字段中的每一项 `required_permissions`，再对全部操作做无副作用 preflight，包括操作名、必要 target 和 payload 形状。
3. 顺序执行 operation；每个 operation 生成 `change.operation` receipt。
4. 全部成功时写 `ChangeCommit(status=committed)` 和 `change.commit/succeeded` receipt。
5. 尚未执行任何操作即失败时写 `failed` commit/receipt。
6. 已有操作成功后失败时写 `partial` commit/receipt，并保留已完成 operation receipts。

`ChangeCommit.result_refs` 指向 final receipt 的 recorded output objects；`operation_receipts` 保留实际完成/失败的 operation evidence。错误正文不会复制进 receipt，protocol result 只保留稳定 failure code 与 SHA-256 message fingerprint。终态写入使用 expected-status compare-and-set，公开 failure transition 不能覆盖 `applying` 或任何终态 proposal。

## Compatibility boundary

- v1 Proposal payload fields仍可读写；新增字段均为 additive/optional。
- 旧 `asset.put` 与 `projection.rebuild` 继续作为 adapter operation，而不是永久 substrate enum。
- Proposal 不等同于 intent；approval 不等同于 commit；receipt 只证明实际发生的 effect。
- Apply 不承诺跨任意外部系统的全局 ACID transaction。Preflight 防止可预测的半提交；不可逆执行中途失败必须显式记录 `partial`。

## Security

Proposal 创建继续阻断 raw secret，并要求使用 `secret_ref`。Intent、ChangeSet、policy、approval、operation output 和 final result 进入 receipt 前会写入独立对象并经过 secret redaction；receipt envelope 仅保留 descriptors、状态和必要摘要。
