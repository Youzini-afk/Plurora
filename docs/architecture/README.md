# 架构

> [English](./README.en.md) · [中文](./README.md)

这里记录 Plurora 的长期分层、当前公开契约边界，以及 Host 控制平面的具体设计。长期架构区分宪法基底、协议公地、组件/内容、Host、发行版和产品，而不是把所有概念压成“内核或 Package”二选一。

## 平台形态与基底

先读 [`ARCHITECTURE.md`](ARCHITECTURE.md)。它是唯一持有完整分层图的文档。

- [`VISION.md`](VISION.md) — 长期开放性与技术方向，不再重复分层图
- [`CONSTITUTIONAL_SUBSTRATE.md`](CONSTITUTIONAL_SUBSTRATE.md) — 当前 runtime 过宽边界与迁回正确层的门槛
- [`CONSTITUTION_V2.md`](CONSTITUTION_V2.md) — 候选长期宪法；尚未采纳，不在默认阅读路径

## Package、Component、Protocol 与运行时

- [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.md) — Package Envelope、Component、Protocol、Content 和执行信任
- [`EXTENSION_POINTS.md`](EXTENSION_POINTS.md) — 当前扩展点 / hook 合同及长期协议归属
- [`EVENT_MODEL.md`](EVENT_MODEL.md) — journal、事件与不透明 payload 模型
- [`RUNTIME_LIFECYCLE.md`](RUNTIME_LIFECYCLE.md) — 当前 runtime / component 生命周期

## Host 控制平面

- [`HOST_DEVELOPMENT_CONTROL_PLANE.md`](HOST_DEVELOPMENT_CONTROL_PLANE.md) — 受控源码变更、验证、promotion 与恢复
- [`HOST_REMOTE_ACCESS.md`](HOST_REMOTE_ACCESS.md) — root / 设备身份、scope、HTTPS pairing 与应用路由暴露
- [`HOST_RESOURCE_AUTHORITY.md`](HOST_RESOURCE_AUTHORITY.md) — Work / Workspace / Installation / Run 等 Host 资源权威、认证上下文与审计
- [`REALIZATION_CONTROLLER.md`](REALIZATION_CONTROLLER.md) — desired/observed state、幂等 operation、安全激活与恢复
- [`TARGET_AGENT_PROTOCOL.md`](TARGET_AGENT_PROTOCOL.md) — remote target 身份、类型化操作、artifact/secret 与 tunnel 边界
- [`OPERATIONS_DATA_RELEASE.md`](OPERATIONS_DATA_RELEASE.md) — 数据迁移、备份、健康、诊断、升级和供应链

这些 Host 设计管理现实资源，但不进入宪法基底，也不拥有上层产品内容语义。

## 外部生态参考（非默认路径）

- [`PI_INTEGRATION.md`](PI_INTEGRATION.md) — pi agent 框架的组件 / 协议吸收边界；属于生态参考，不是平台核心阅读路径

## 当前合同归属

Contract V1 仍是当前公开合同。逐项判断某个方法、对象或事件属于基底、Host、协议还是 Shell Profile，请读 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md) 与 [`../spec/CONTRACT_REGISTRY.md`](../spec/CONTRACT_REGISTRY.md)。
