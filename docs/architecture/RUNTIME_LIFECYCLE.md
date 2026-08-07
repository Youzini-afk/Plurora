# 运行时生命周期

> [English](./RUNTIME_LIFECYCLE.en.md) · [中文](./RUNTIME_LIFECYCLE.md)

本文记录 Contract V1 当前 runtime 协调的 Package、Session、Proposal 与 Capability Invocation 生命周期。它们是现行兼容合同，不代表宪法基底永久只认识或必须认识这些名词。

这些生命周期不描述 turn、chat、prompt 或其他领域操作。领域语义由采用的 Protocol、Component 或 Product 拥有；当前 V1 常以 Package writer 和事件命名空间承载它们。

## 包生命周期

```text
discovered  manifest 对 host 可见
loading     manifest 已验证，sandbox 已准备，ABI 已检查
starting    entry point 已启动，内核握手完成，capability 和 hook 已注册
ready       接受调用和 dispatch
degraded    可达但报告能力降低（heartbeat 迟缓、部分功能不可用）
stopping    已发送优雅关闭信号
stopped     资源已释放
unloaded    在 host 中不再活跃
```

每次转换都会发出一个 `kernel/v1/package.*` 事件。订阅方通过公开协议做出反应，例如可观测性工具或其他能力包。内核不为包状态暴露私有钩子。

## 会话生命周期

一个会话是带标签的事件流，附带一组包集和一个权限作用域。内核不赋予它其他含义。

```text
requested   open() 已收到，principal 和 labels 已提供
opening     context/before_open 已 dispatch（sync，可 veto）
open        context/opened 已发出
            event 日志接受已授权写入者的 append
            capability invocation 正在针对活跃 package set 进行 dispatch
forking     fork() 已收到，携带 parent session 和 forked-from 序号
forked      context/forked 已发出；子 session 继承 parent 直至所选序号
closing     context/before_close 已 dispatch（sync，可 veto）
closed      context/closed 已发出；日志已冻结，不再接受 append
```

runtime 不拥有“当前 turn”、“活跃 actor”或 Session 的任何内容级状态。需要这些概念的协议或组件从事件、对象和自己的 projection 中推导。

## 提案生命周期

内核负责协调通用的、需要审批的变更提案。该生命周期与内容无关。它只知道可以执行的操作，例如 `asset.put` 和 `projection.rebuild`。

```text
created     proposal 在发起 principal 下记录；change/proposal.created 已发出
approved    审批者决定已记录；change/proposal.approved 已发出
rejected    审批者决定已记录；change/proposal.rejected 已发出
applied     已批准的 proposal 已对内核执行；change/proposal.applied 已发出
failed     执行或验证失败；change/proposal.failed 已发出
```

能力包或 assistant 身份不能直接应用提案。提案必须先到达 `approved` 状态。内核不会发明特定领域的提案语义；多步事务和能力包侧补偿等更丰富的操作属于上层能力包。

## 能力调用生命周期

```text
requested        invoke(id, version, input) 已收到
authorizing      capability/before_invoke 已 dispatch（sync，可 veto）
routed           按 id+version+session package set 选择 provider
running          provider 正在执行；streaming chunk 可能正在流动
completed        capability/completed 已发出，附带 output（或 stream 结束）
failed           capability/failed 已发出，附带结构化 error
cancelled        取消已被 provider 确认；failed/completed 事件记录结果
```

内核将调用记录为内核事件。`input` 和 `output` 的内容对内核不透明，只按 provider 声明的 schema 验证。

## 取消与超时

每个长时间运行的操作都有 deadline，例如能力调用、钩子分发和包启动。deadline 由清单中的沙箱策略和 host 策略推导而来。超过 deadline 会触发取消，内核会记录结果。

内核不会为内容发明自己的取消语义。没有“重新生成”，也没有“停止生成”。这类操作是能力包的能力。

## Replay 与引导

当 host 重启时：

1. 清单被重新发现。
2. 包经历 `loading` 和 `starting`。
3. 已存储的会话立即可用于只读 replay。
4. 会话只有在所需包到达 `ready` 后才恢复写操作。

需要从事件日志重建内部状态的能力包，通过 `events.read` 和 replay 流来完成。内核不提供其他恢复机制。

## 错误

内核只在自己的边界上对错误分类：transport、manifest、schema、permission、capacity、lifecycle、ambiguous-route。能力包错误作为不透明的结构化失败，通过能力调用传递，并记录在 `capability/failed` 下。

## 本生命周期未描述的内容

- 没有 turn，没有 message，没有 prompt 循环。
- 没有模型调用编排。
- 没有记忆更新流。
- 没有 agent 任务。
- 没有 world tick。

以上概念都适合存在于能力包内部。它们都不是内核生命周期。
