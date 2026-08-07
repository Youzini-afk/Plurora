# Model Connectivity Kit

> [English](./MODEL_CONNECTIVITY_KIT.en.md) · [中文](./MODEL_CONNECTIVITY_KIT.md)

Model Connectivity Kit 是 Plurora 的模型连接基础设施。它准备 provider profile 与 route plan，但不把数据发送给外部 provider。

## 包含的能力包

- `plurora/model-connector-lab`
  - `describe_families`
  - `validate_profile`
  - `mask_secret`
  - `discovery_plan`
  - `compat_report`
- `plurora/model-routing-lab`
  - `define_binding`
  - `resolve_binding`
  - `preview_routes`
  - `params_normalize`
  - `compat_report`

## 安全边界

当前能力刻意不出网，也不做推理：

- discovery 是计划，不是 live provider query；
- validation 是结构检查，不是 credential verification；
- 接受 `secret_ref`，拒绝 raw secrets；
- route resolution 从显式 binding 中选择，不创建隐藏的 global route；
- params normalization 保持 provider-specific options namespaced；
- 相关输出包含来源，以及 `network_performed: false` 或 `inference_performed: false`。

## 典型流程

1. 用 `plurora/model-connector-lab/describe_families` 描述 provider family。
2. 用 `plurora/model-connector-lab/validate_profile` 验证脱敏安全的 profile。
3. 用 `plurora/model-connector-lab/discovery_plan` 生成模型发现计划。
4. 用 `plurora/model-routing-lab/define_binding` 定义 consumer-slot binding。
5. 用 `plurora/model-routing-lab/resolve_binding` 解析可重放 route。
6. 用 `plurora/model-routing-lab/params_normalize` 归一化 generation-like 参数。

持久化仍应通过公开资产和提案协议操作。model labs 不获得特殊写权限。

## TavernHeadless 参考

`integrations/tavern-headless/model-connectivity-map.yaml` 跟踪从 TavernHeadless 研查的 provider、profile 和 instance 行为。这个 map 只是 reference ledger。Plurora 不创建 `tavern-*` model packages。

## Deferred inference

真实模型调用属于未来能力包族，可能是 `plurora/model-inference-lab`。在此之前，Plurora 需要先定义 secret 解析、网络权限、请求/响应审计、流式/取消策略、用量统计、provider 错误和脱敏规则。
