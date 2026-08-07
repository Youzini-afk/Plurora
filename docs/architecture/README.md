# 架构

> [English](./README.en.md) · [中文](./README.md)

这里记录 Plurora 的长期分层、当前 Contract V1 兼容边界，以及 Host 控制平面的具体设计。长期架构区分宪法基底、协议公地、组件/内容、Host、发行版和产品，而不是把所有概念压成“内核或 Package”二选一。

## 平台形态与基底

- [`VISION.md`](VISION.md) — 长期整体形态、开放性与技术方向
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — 分层模型、单向依赖、Host 正交关系与当前实现位置
- [`PLATFORM_KERNEL.md`](PLATFORM_KERNEL.md) — 宪法基底拥有的机制，以及当前 v1 kernel 的兼容职责
- [`CONSTITUTION_V2.md`](CONSTITUTION_V2.md) — 候选长期宪法、不变量与演化约束；尚未替代 Contract V1

## Package、Component、Protocol 与运行时

- [`CAPABILITY_PACKAGE.md`](CAPABILITY_PACKAGE.md) — Package Envelope、Component、Protocol、Content 和执行信任
- [`EXTENSION_POINTS.md`](EXTENSION_POINTS.md) — 当前扩展点 / hook 合同及长期协议归属
- [`EVENT_MODEL.md`](EVENT_MODEL.md) — journal、事件与不透明 payload 模型
- [`RUNTIME_LIFECYCLE.md`](RUNTIME_LIFECYCLE.md) — 当前 runtime / component 生命周期

## Host 控制平面

- [`HOST_DEVELOPMENT_CONTROL_PLANE.md`](HOST_DEVELOPMENT_CONTROL_PLANE.md) — 受控源码变更、验证、promotion 与恢复
- [`HOST_REMOTE_ACCESS.md`](HOST_REMOTE_ACCESS.md) — root / 设备身份、scope、HTTPS pairing 与应用路由暴露
- [`HOST_PROJECT_AUTHORITY.md`](HOST_PROJECT_AUTHORITY.md) — 当前 Project 资源权威、认证上下文、session binding 与审计
- [`DURABLE_DEPLOYMENT_CONTROLLER.md`](DURABLE_DEPLOYMENT_CONTROLLER.md) — desired/observed state、幂等 operation、安全激活与恢复
- [`TARGET_AGENT_PROTOCOL.md`](TARGET_AGENT_PROTOCOL.md) — remote target 身份、类型化操作、artifact/secret 与 tunnel 边界
- [`OPERATIONS_DATA_RELEASE.md`](OPERATIONS_DATA_RELEASE.md) — 数据迁移、备份、健康、诊断、升级和供应链

这些 Host 设计管理现实资源，但不进入宪法基底，也不拥有上层产品内容语义。

## 外部生态与吸收边界

- [`PI_INTEGRATION.md`](PI_INTEGRATION.md) — pi agent 框架的组件 / 协议吸收边界

## 当前合同归属

Contract V1 仍是当前公开合同。逐项判断某个方法、对象或事件属于基底、Host、协议还是 Shell Profile，请读 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md) 与 [`../spec/CONTRACT_REGISTRY.md`](../spec/CONTRACT_REGISTRY.md)。
