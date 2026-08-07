# Memory Package Authoring Guide

> [English](./MEMORY_PACKAGE_AUTHORING.en.md) · [中文](./MEMORY_PACKAGE_AUTHORING.md)

本指南说明如何在当前 Contract V1 中，用普通 Package / Component 创作、替换和消费长期记忆与知识。Memory 的共享语义可以由可选 Protocol 定义，具体记录和检索实现由 Component / Product 拥有；它不是宪法基底服务。

## 核心原则

1. Memory 不进入基底。记忆记录、检索、更新、纠正和删改计划由采用的 Protocol、Component 与 Product 管理；当前 V1 通过普通 Package 分发实现，而不是 `platform.memory.*`。
2. 无官方优先级。`official/memory-lab` 是一种实现。第三方包如 `thirdparty/memory-lab` 完全可互换。
3. 变更要走提案。记忆更新、纠正和遗忘/删改只产出提案草案或计划。它们永不直接修改可信状态或删除记录。消费者必须审批后才能应用。
4. 参考实现本地可重放。它不要求网络、嵌入 API 或模型推理。第三方包可通过自身出站和网络权限添加此类能力。
5. 分支感知。记忆记录按分支作用域。检索和视图可按分支引用过滤。
6. 阻断 raw secret。所有能力输入都会扫描原始秘密。原始 API key、token 和 password 被拒绝并返回 `redaction_state: unsafe_blocked`。请使用 `secret_ref` 引用。
7. 无禁止命名空间。记忆包不得引用 `platform.memory.*`、`platform.experience.*`、`platform.world.*`、`platform.scene.*`、`platform.turn.*`、`platform.chat.*`、`platform.agent.*`、`platform.model.*`、`platform.prompt.*` 或 `platform.director.*`。

## 记忆实验室能力契约

`official/memory-lab` 提供 9 项能力：

| 能力 | 用途 | 输出形态 |
|---|---|---|
| `describe_memory_contract` | 描述包契约 | `memory_lab_contract` |
| `record_memory` | 记录一条记忆 | `memory_record` |
| `retrieve_memory` | 检索匹配的记忆 | `retrieval_result` |
| `trace_retrieval` | 展示检索如何匹配 | `retrieval_trace` |
| `draft_memory_update` | 草拟提案门控的更新 | `memory_update_draft` |
| `apply_memory_correction` | 产出纠正形态 | `memory_correction` |
| `draft_forget_redaction` | 草拟删改计划 | `memory_redaction_plan` |
| `branch_memory_view` | 按分支查看记忆 | `memory_branch_view` |
| `explain_memory_provenance` | 解释记录来源 | `memory_provenance` |

### Surface

- `forge_panel`：检查记忆记录、追踪、草案、纠正、删改计划和来源。
- `assistant_action`：草拟需审批的更新、纠正或删改计划。
- `home_card`：记录和检索记忆。

## 记忆记录

`memory_record` 包含：

- `record_id`：由 key + content address 派生的确定性 ID。
- `record_kind`：`fact`、`preference`、`observation`、`correction`、`summary`、`context` 之一。
- `key`：记录的查找键。
- `content`：记录内容（任意值）。
- `content_address`：稳定内容寻址哈希（FNV-1a 64-bit）。
- `branch_ref`：此记录所属的分支。
- `disclosure`：AI 生成 / 实时生成 / 未指定元数据。
- `source_refs`：协议可见源引用。
- `knowledge_refs`：可选的 knowledge-lab 条目交叉引用。

## 检索

`retrieve_memory` 使用确定性关键词匹配（大小写不敏感子串）。支持分支感知过滤：指定 `branch_ref` 时，仅考虑该分支的记录。

`trace_retrieval` 产出详细追踪，展示检索算法的每一步。

## 提案门控更新

`draft_memory_update` 产出 `memory_update_draft`：

- `update_kind`：`add_record`、`modify_record`、`correct_record`、`forget_record`、`merge_records`。
- `requires_user_approval`：始终为 `true`。
- `plan_only`：始终为 `true`，无直接状态变更。
- `content_address`：草案的稳定哈希。

消费者必须通过提案生命周期审批并应用草案。

## 纠正

`apply_memory_correction` 产出 `memory_correction` 形态：

- `original_record_ref`：被纠正记录的引用。
- `corrected_content`：纠正后的内容。
- `requires_user_approval`：始终为 `true`。
- `content_address`：稳定哈希。

## 遗忘 / 删改

`draft_forget_redaction` 产出 `memory_redaction_plan`：

- `target_record_refs`：删改目标记录。
- `redaction_scope`：`record_only` 或更广。
- `status`：`draft`（需要审批后才变为 `applied`）。
- `plan_only`：始终为 `true`，无直接删除。
- `requires_user_approval`：始终为 `true`。

删改计划是一份提案。实际删除/删改仅在明确用户审批后发生。

## 分支感知视图

`branch_memory_view` 支持范围：

- `current_branch`：仅指定分支的记录。
- `all_branches`：所有分支的记录。
- `specified_branch`：与 current_branch 相同（显式）。
- `branch_diff`：按分支分组记录以便比较。

## 来源

`explain_memory_provenance` 产出链，每步包含：

- `step`：`record_created`、`record_retrieved`、`update_drafted`、`correction_applied`、`redaction_planned`、`branch_viewed`、`provenance_traced`。
- `ref`：协议可见引用。
- `content_address`：稳定内容寻址哈希。
- `description`：人类可读解释。

## 跨包集成

`official/playable-creation-board` 在其 `request_change` 输出中包含可选 `memory_refs`：

- `memory_package_id`：使用的记忆包（默认 `official/memory-lab`）。
- `retrieve_context_plan`：描述如何为变更规划检索记忆上下文（可选，board 无此亦可运行）。
- `knowledge_refs`：可选的 knowledge-lab 条目交叉引用。

这是可选交叉引用。board 不依赖 memory-lab 才能运行。

## 第三方替换

`thirdparty/memory-lab` 证明 `official/memory-lab` 无特殊优先级：

- 相同 9 项能力、3 个 surface。
- 相同输出形态（`memory_record`、`retrieval_trace`、`update_draft`、`correction`、`redaction_plan`、`branch_view`、`provenance`）。
- 通过 `examples/compositions/memory-lab-replacement/composition.yaml` 加载。
- 官方 `memory-lab` 列为 `replacement_candidate`，不是默认 provider。

## 这不是什么

- 不是 RAG 产品。参考实现使用可重放的关键词匹配，不是向量搜索或嵌入 API。
- 不是聊天记忆系统。没有对话回合、消息或提示词语义。
- 不是内核记忆。不存在 `platform.memory.*` 方法或命名空间。
- 不是唯一方式。第三方包可通过普通包能力提供不同的检索算法、存储后端或嵌入匹配。
