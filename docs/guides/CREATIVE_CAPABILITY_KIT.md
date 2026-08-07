# Creative Capability Kit

> [English](./CREATIVE_CAPABILITY_KIT.en.md) · [中文](./CREATIVE_CAPABILITY_KIT.md)

Creative Capability Kit 把成熟的无界面创作和 RP 流程整理成通用能力包。这些包遵守 Plurora 的公开协议和清单约束。

TavernHeadless 提供边界案例参考。但这些官方包不是 `tavern-*` wrapper：

- `official/persona-lab` 处理类似 persona 的结构化 profile。
- `official/knowledge-lab` 处理结构化知识集合和匹配追踪。
- `official/context-lab` 处理有边界的上下文块组装和预算诊断。
- `official/text-transform-lab` 处理可重放的文本转换预览和管线说明。

## 规则

- 内核不知道 persona、knowledge、prompt、worldbook、chat、character 或 model-call 概念。
- 这些包都是普通清单、能力和 surface 包。
- 兼容输入格式只是 adapter 和 fixture，不是 Plurora 的 canonical ontology。
- 变更必须表达为明确的资产、projection 或提案计划，不能隐藏为包状态写入。
- 输出应包含来源和诊断。

## Reference tracking

`integrations/tavern-headless/` 记录已研查的 TavernHeadless commit、能力映射和紧凑 fixture。TavernHeadless 更新时，用它作为 review ledger。

决策词汇：

- `adapted`：已泛化为 Plurora package。
- `adapter_only`：适合 import/export，但不是 canonical。
- `deferred`：有价值，但暂不属于本 kit。
- `rejected`：明确不继承。

## 典型流程

1. 用 `official/persona-lab/import_profile` 导入类似 profile 的载荷。
2. 用 `official/knowledge-lab/import_collection` 导入知识集合。
3. 用 `official/knowledge-lab/match_entries` 匹配知识条目。
4. 用 `official/context-lab/assemble_preview` 组装通用上下文块。
5. 用 `official/text-transform-lab/apply_preview` 预览可重放的转换。
6. 如果需要持久化，通过公开协议创建需审批的提案，写入资产或重建 projection。

这条流程刻意停留在包层。第三方包只要暴露兼容能力和 surface，就可以替换任意官方 lab。
