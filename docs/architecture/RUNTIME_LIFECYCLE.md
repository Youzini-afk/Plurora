# Runtime 生命周期

> [English](./RUNTIME_LIFECYCLE.en.md) · [中文](./RUNTIME_LIFECYCLE.md)

本文记录当前公开契约 runtime 协调的 Package、Context、Proposal 与 Capability Invocation 生命周期。它们是现行可执行契约，不表示所有 Plurora 产品都必须用同一组名词组织领域。

## Package 生命周期

```text
discovered  Manifest 对 Host 可见
loading     Manifest 已校验，执行边界准备中
starting    entry 启动并注册声明
ready       接受已支持调用
loaded      Package 注册已提交
degraded    可达但能力下降或失败
stopping    已发起关闭
stopped     执行资源已释放
unloaded    Package 已从 live registry 移除
```

状态转换会由 writer `plurora/runtime` 发出平台拥有的 `host/package.*` 事件。Subscriber 通过公开 journal 观察，不存在第一方私有生命周期通道。

## Context 生命周期

Context 是带 label、active Package set 与 authority scope 的事件流。Runtime 不为其赋予内容语义。

```text
requested   收到 context.open
open        持久化 context/opened
forking     收到带 parent 与 sequence 的 context.fork
forked      记录 child lineage 并持久化 context/forked
closing     收到 context.close
closed      持久化 context/closed；拒绝后续 append
```

Runtime 拥有身份、顺序、lineage 与 authority 边界。Protocol 与 Component 从 journal event、object 与 projection 派生领域状态。

## Proposal 生命周期

`change.proposal.*` facade 协调需要审批的通用变更：

```text
created     记录 proposal；change/proposal.created
approved    记录 review decision；change/proposal.approved
rejected    记录 review decision；change/proposal.rejected
applied     提交已批准 operation；change/proposal.applied
failed      校验或应用失败；change/proposal.failed
```

Apply 会重新检查 authority 与 terminal state。Proposal payload 的领域含义仍在宪法基底之外。

## Capability invocation 生命周期

```text
requested        收到 capability.invoke 或 capability.stream
authorizing      校验 caller handle、scope、permission 与 input
intercepting     dispatch capability/before_invoke
routed           显式或无歧义地选择 provider
running          provider 执行；可能发出 stream frame
completed        记录 capability/completed 或 stream-ended terminal
failed           记录 capability/failed 或 stream-error terminal
cancelled        记录 cancellation terminal
timed out        记录 timeout terminal
```

Invocation input/output 是 provider-owned JSON，并按声明 schema 校验。Runtime 记录 authority 与执行证据，但不解释领域内容。

## Deadline 与 cancellation

长时间执行受 Manifest sandbox policy 与 Host policy 限制。Cancel 与 timeout 会阻止后续 stream chunk，并产生不同 terminal state。“重新生成”等领域动作仍属于 Package capability，不是平台生命周期 primitive。

## 重启与 replay

Host 重启时：

1. 打开 durable store 与 profile；
2. 重新发现 Manifest，autoload Package 重新进入生命周期；
3. 按 authority 立即开放 journal 读取；
4. Component 通过公开 replay 重建自己的 projection；
5. 中断的 Host operation 使用各自领域的 durable recovery 规则。

Runtime 不会在公开 event、object、receipt 与 registered store 之外重建隐藏内容状态。

## 刻意省略

该生命周期不定义 turn、message、prompt、model orchestration、memory update、agent task 或 world tick。Protocol、Component 与 Product 可以独立定义这些生命周期。
