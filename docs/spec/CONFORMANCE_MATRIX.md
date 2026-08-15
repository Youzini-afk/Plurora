# Conformance 矩阵

> [English](./CONFORMANCE_MATRIX.en.md) · [中文](./CONFORMANCE_MATRIX.md)

Conformance 套件是章程的可执行守卫：它同时证明正向行为和拒绝行为。完整具名用例清单以 CLI 为准，不要在本文手写一份会漂移的总数或逐条状态表。

当前实现快照见 [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md)。建设方向见 [`../roadmap/NEXT_STEPS.md`](../roadmap/NEXT_STEPS.md)。

## 怎么跑

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
cargo run -p plurora-cli -- conformance --list
cargo run -p plurora-cli -- conformance --case sharing_lab
cargo run -p plurora-cli -- conformance --tag sharing
cargo run -p plurora-cli -- conformance --fail-fast
cargo run -p plurora-cli -- conformance --slowest 3
```

过滤、计时和诊断见 [`../performance/PERFORMANCE_AND_CODE_HEALTH.md`](../performance/PERFORMANCE_AND_CODE_HEALTH.md)。

第三方 Package 的本地验收见 [`../guides/CONFORMANCE_KIT.md`](../guides/CONFORMANCE_KIT.md) 与 `plurora conformance package`。

## 域覆盖

`--list` 输出的 tag 按域分组。新增用例时选已有 tag，避免再发明平行分类：

| 域 | 典型 tag | 守卫什么 |
|---|---|---|
| Substrate | `substrate` `event` `protocol` `permission` `hook` | 身份、journal、公开方法、权限、钩子 |
| Host 生命周期 | `host` `assembly` `secret` | Installation、Work/Lock、secret_ref |
| Run / Powerbox / Realization | `runtime` `surface` | Run、Exposure/Binding、Realization、Surface |
| Package 执行 | `package` `capability` `subprocess` `first_party` | in-process / subprocess、第一方无特权 |
| 出站与流 | `network` `outbound` `stream` | fail-closed 网络、审计、流式生命周期 |
| 创作与体验 | `agentic` `experience` `memory` `sharing` | agent、体验、记忆、分享 |
| 存储与来源 | `storage` `asset` `projection` `source_intake` `workspace_lab` `retrieval` | 对象、投影、外部来源 |
| 替换证明 | `replacement` | 第三方可替换第一方，无 publisher priority |
| 可选联网 | `live` | 仅在显式 opt-in 时跑的真实出站 |

影响架构判断的覆盖应写进 architecture / spec 正文，而不是把 CLI 列表再抄一遍。
