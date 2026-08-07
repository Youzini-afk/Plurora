# 能力句柄

> [English](./CAPABILITY_HANDLES.en.md) · [中文](./CAPABILITY_HANDLES.md)

能力句柄是内核 v1 的运行时权威模型。Manifest 中的字符串声明包**最多**可以要求什么；句柄表示内核在某一时刻实际授予它什么。

## 什么是句柄

句柄由内核铸造、不可伪造、带作用域、可撤销、可过期。包看到的是一个不透明 id 和最小必要 metadata；它不能凭空构造同等权威。

句柄适用于：

- 调用某个 capability provider；
- 读取或追加某个事件范围；
- 使用出站网络原语；
- 解析声明过的 `secret_ref`；
- 访问 host 暴露给包的其他 v1 权威。

## 为什么不是字符串权限

只用 manifest 字符串会把“声明”和“实际权威”混在一起。包可以声明 `capabilities.invoke`，但运行时仍需要知道：哪个 session、哪个 provider、哪个方法、哪个 host、什么时候过期、是否已撤销。

句柄模式是常见的 capability-security 模式，类似：

- WASI preview2 的 resource handles；
- Cloudflare Workers Durable Object / service binding；
- SES / object-capability 风格的 attenuated references；
- 浏览器中不可直接伪造的 platform handles。

Plurora 使用它不是为了复杂化 API，而是为了把最小权限、衰减、撤销、审计做成普通路径。

## 字符串声明 vs 运行时权威

| 层 | 含义 | 可变性 |
|---|---|---|
| Manifest capability / permission 字符串 | 权限上限与审核依据 | 包发布时固定 |
| Host policy | 本机允许的上限 | host 配置决定 |
| Runtime handle | 当前实际可用权威 | 可衰减、撤销、过期 |

调用时以内核句柄为准。Manifest 声明不足时不会铸造句柄；Host policy 禁止时也不会铸造句柄。

## 句柄字段

| 字段 | 说明 |
|---|---|
| `id` | 内核铸造的不可伪造标识。 |
| `cap_type` | 权威类型：invoke、event、outbound、secret、host 等。 |
| `cap_version` | 句柄语义版本。 |
| `scope` | 作用域：package、session、capability、provider、host、resource。 |
| `constraints` | 限制：方法、host、schema、次数、字节、deadline、metadata。 |
| `lease` | 过期时间、刷新策略或一次性策略。 |
| `provenance` | 来源：manifest 声明、host grant、衰减父句柄、审计原因。 |
| `parent` | 可选父句柄，用于衰减树和撤销传播。 |

## 生命周期

### 1. Mint at package load

路径 A 包加载时，内核读取 manifest、host policy 与 profile。满足条件的声明被转换成初始句柄。路径 B 包不会获得 v1 句柄。

### 2. Inject through bindings

句柄通过 bindings 注入：

- subprocess：`package.handshake` 的 `bindings` 字典；
- rust_inproc：`KernelEnv`；
- wasm：未来 WIT resource imports；
- remote：未来 SPIFFE + Biscuit token 兑换。

SDK 把这些句柄封装成 `kernelClient` 方法，包不需要手写协议字段。

### 3. Attenuate

包或 host 可以把父句柄变成更窄的子句柄：更短 lease、更小 session 范围、更少 method、更少 host、更低调用次数。子句柄永远不能比父句柄更强。

### 4. Use

调用 `kernel.v1.capability.invoke`、出站方法或事件方法时，运行时检查句柄 id、调用方、scope、constraints、lease 与 revoke 状态。失败时 fail closed 并写审计。

### 5. Revoke

`kernel.v1.cap.revoke(handle)` 让句柄立即失效。撤销可以只影响一个 handle，也可以按策略影响子树。卸载 package 时，内核撤销该包持有的 live handles。

### 6. Expire

带 lease 的句柄到期后不可再用。包需要重新通过 host grant、manifest reload 或显式刷新路径获取新句柄。

## 包如何使用

包作者通常不直接操作裸句柄，而是用 SDK：

```ts
const result = await kernelClient.invoke("provider/capability", input)
```

SDK 从 bindings 中选择合适 handle，把它放进 protocol context。若没有 handle，调用失败而不是退回匿名 host 权限。

低层协议仍允许显式传 handle id，供非 TypeScript SDK、测试和其他语言绑定使用。

## Subprocess bindings

Subprocess 包启动后先握手。握手消息包含包身份、contract mode、SDK capability、可用 bindings。示意：

```json
{
  "contract": "v1",
  "bindings": {
    "invoke": [{ "id": "cap_...", "scope": { "package_id": "demo/echo" } }],
    "outbound": [],
    "events": []
  }
}
```

stdout 保留给 JSON-RPC 帧；stderr 可被内核捕获为 package log。

## Rust in-process bindings

Rust in-process 包通过 `KernelEnv` 获得句柄。Host catalog 负责把 manifest entry 与 in-process provider trait 绑定。未在 catalog 中注册的 in-process provider 会被拒绝加载。

## 出站与 secret

网络和 secret 也使用同一模型：

- manifest 声明 `permissions.network` 和 `permissions.secret_refs`；
- host policy 决定是否允许；
- 内核铸造 outbound / secret handle；
- 调用时检查 host、method、scheme、secret_ref 声明；
- 审计只写引用和脱敏状态，不写 raw secret。

## Effect audit 如何消费句柄

`kernel.v1.audit.package` 与 `plurora audit --package <id>` 把三类数据合并：

1. declared：manifest 中声明的 capability、permission、network、secret_refs；
2. granted：内核实际铸造、衰减、撤销、过期的 handles；
3. used：capability invocation、outbound、event read/write、secret resolution 的审计事件。

报告会标记：

- declared but unused；
- used but undeclared；
- granted but never used；
- revoked/expired handle use attempt；
- wider-than-needed declarations；
- Path B 的 `contract_mode: "none"`。

## 设计原则

- 默认最小权限。
- 不能靠包名获得特权。
- 字符串声明不等于权威。
- 权威必须可观察、可撤销、可过期。
- 所有拒绝都应 fail closed。
- 审计记录不能包含 raw secret 或内容语义。

## 常见问题

### 句柄会不会破坏跨语言 SDK？

不会。句柄是 JSON 可表示的不透明 id + metadata。任何语言都能保存并传回。

### 包能不能把句柄传给另一个包？

默认不能。句柄绑定调用方 package。未来 Powerbox 可以做显式转授，但必须生成新的 provenance 与审计链。

### 路径 B 包有没有句柄？

没有 v1 capability bindings。它可以作为自包含进程运行，但不能通过 manifest 声明获得内核能力。

### 撤销是否影响正在运行的调用？

新的调用必须失败。进行中的流式调用会按取消/终止策略处理，并发出对应生命周期事件。

## 操作员检查清单

- 检查 package manifest 中声明的权限是否都对应必要功能。
- 运行 `plurora audit --package <id>` 查看 declared vs used。
- 对长期运行包定期检查 live handles 数量。
- 撤销不再需要的 handles，而不是等待 package unload。
- 对网络与 secret handles 设置较短 lease。
- 对高风险调用使用 attenuated child handle。
- 在 CI 中运行 conformance kit，防止新增未声明使用。

## 包作者检查清单

- 只声明真实需要的 capability 和 permission。
- 不在配置文件中保存 handle id 作为长期凭据。
- 不把 handle 传给另一个包或用户脚本。
- 调用失败时把 permission error 暴露成可理解诊断。
- 对可选功能接受缺少 handle 的情况。
- 使用 SDK bindings，不手写 spoofed package id。

## 最小示例流程

1. Manifest 声明消费 `example/echo.invoke`。
2. Host policy 允许该 provider。
3. Package load 时内核铸造 invoke handle。
4. Subprocess handshake 收到 bindings。
5. SDK 用该 handle 调用 provider。
6. 内核写 `capability.invoked` 与 `capability.completed`。
7. `plurora audit` 显示 declared、granted、used 三者一致。

## 参考

- [`../spec/KERNEL_V1_CONTRACT.md`](../spec/KERNEL_V1_CONTRACT.md)
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.md)
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.md)
