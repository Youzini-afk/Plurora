# 宪法基底与当前 Runtime 边界

> [English](./CONSTITUTIONAL_SUBSTRATE.en.md) · [中文](./CONSTITUTIONAL_SUBSTRATE.md)

本文区分两件事：

1. Plurora 长期应保持极小的 **Constitutional Substrate**；
2. 当前由 `plurora-core` 与 `plurora-runtime` 承载的更宽实现边界。

二者目前并不完全重合。`plurora-core` / `plurora-runtime` 为了提供可运行平台，还承载了一部分 Host、Protocol 与 Shell 职责。长期目标是让职责回到正确层，同时保持显式版本边界、迁移能力与数据可读性。

## 拥有与不拥有

完整的“基底拥有 / 不拥有”清单以 [`ARCHITECTURE.md`](ARCHITECTURE.md) 的分层职责为准。本文只补充当前实现与那张清单之间的缝。

一句话：基底拥有身份、authority、内容寻址对象、journal/因果、invoke/stream/cancel、CAS/幂等、effect receipt 和协议协商。它不拥有 Library、聊天、agent、世界、Docker、某个官方界面或任何第一方 Package ID。

## 当前 Contract V1 kernel 承载的兼容职责

当前 v1 公开合同仍包含：

- session 与 event；
- package 生命周期与 capability routing；
- extension point 与 hook；
- asset、projection、proposal；
- Project；
- Host info、target、exec、port、proxy 与 outbound；
- Surface contribution；
- permission 与 audit。

这些方法继续是支持中的公开接口。它们的长期所有者不同：

| 当前概念 | 长期归属 |
|---|---|
| principal、authority、object、journal、invoke、stream、receipt | Constitutional Substrate |
| package 获取、Project、target、exec、port、proxy、secret、部署 | Host Control Plane |
| projection、change、领域共享状态机 | Protocol Commons |
| Surface slot、Home / Forge / Assist 映射 | Shell / Product Profile |
| chat、agent、memory、world 等语义 | 具体协议、组件或产品 |

逐项分类见 [`../spec/CONTRACT_LAYERING_MATRIX.md`](../spec/CONTRACT_LAYERING_MATRIX.md)。

## 当前实现中应继续保持的边界

### 公开协议唯一

HTTP、stdio、同进程调用、未来 WASM imports 和远程边界必须保留相同的身份、authority、错误和 effect 语义。内部调用不能获得第三方无法使用的能力。

### 第一方实现无特权

官方组件和第三方组件通过同一注册、binding、调用和审计机制工作。包名不能成为权限判断或路由优先级。

### 内容语义不进入核心机制

内核和 Host 不解释角色、消息、模型、agent、世界、文档或游戏规则。它们可以保存不透明数据和可验证引用，但语义由协议与组件解释。

### 权威由句柄和调用上下文表达

裸字符串权限、调用者提供的 `session_id`、路径或 target ID 都不能单独构成授权依据。实际授权必须绑定认证 principal、资源 selector、条件和生命周期。

### 事件日志不是所有数据的唯一存储

Journal 保存需要长期排序、审计或因果的事实。大对象、媒体、快照和模型输出进入对象存储；当前运行状态和 Host 操作可以使用专门的 durable control-plane projection。不能为了“事件即真相”的口号把所有字节复制进日志。

### 历史回放不重新触发效果

读取历史使用已记录输出和 receipt。再次调用模型、网络、工具或进程必须创建新的 invocation 和因果分支。

## 执行与信任

统一调用合同不等于统一信任保证。至少区分：

| Trust class | 典型保证 |
|---|---|
| `sandboxed_component` | 显式 imports、资源限制、较强可移植性 |
| `isolated_process` | 进程故障隔离；OS 级文件/网络强制需要 Host 提供可检查的 enforcement 声明 |
| `remote_boundary` | 远程身份、网络故障、租户和服务策略显式化 |
| `trusted_native` | Host 级信任与性能逃生口，不适合不可信动态代码 |
| `static_resource` | 不执行代码，只提供内容或 Surface |
| `foreign_capsule` | 可托管，但不承诺平台协议、组合或可移植保证 |

Rust in-process、subprocess、WASM 和 remote 可以服务相同协议，但不得被文档描述为“只有打包形式不同”。

## Transport

当前实现支持：

- in-process Rust 调用；
- HTTP `/rpc`；
- SSE 事件订阅；
- Host stdio JSON-RPC；
- Host-owned HTTP / WebSocket outbound 与反向代理。

未来 transport 可以增加，但不能改变上层合同的身份、authority 和 terminal semantics。

## 新机制进入基底的门槛

新增基底职责前，必须说明：

1. 为什么普通协议、组件或 Host 无法安全实现；
2. 它是否与特定产品、UI、工作流或内容本体无关；
3. 它是否增加用户自由和可替换性，而不是锁定当前第一方实现；
4. 它的错误、取消、资源限制、审计和迁移语义；
5. 它如何与现有 v1 数据和客户端兼容。

“多个功能都方便使用它”不足以进入基底。默认放在能够拥有其语义和生命周期的最高层。

## 稳定性承诺

当前 Contract V1 保持可用并通过兼容层演化；候选 v2 只有在明确采纳后才成为稳定宪法。基底可以增长，但增长速度必须慢于上层产品和协议，且每一次增长都应缩小未来锁定，而不是扩大它。
