# Contract 分层矩阵（Candidate）

> [English](./CONTRACT_LAYERING_MATRIX.en.md) · [中文](./CONTRACT_LAYERING_MATRIX.md)

> 状态：Candidate owner 分类。现行 wire 契约见
> [`PUBLIC_CONTRACT.md`](PUBLIC_CONTRACT.md)；宪法原则见
> [`CONSTITUTION_V2.md`](../architecture/CONSTITUTION_V2.md)。

## 目的

公开契约横跨宪法基底、Host control、共享 Protocol 与 Shell Profile。该矩阵明确 owner，但不声称当前每个实现文件都已经完全符合长期理想边界。

它回答：

1. 每个公开身份当前由哪一层拥有；
2. 哪些职责仍然混合；
3. 哪些未来变更需要新的显式版本边界；
4. 哪些 Product choice 必须留在 substrate 之外。

## 层代码

| Code | Layer | 职责 |
|---|---|---|
| `S` | Constitutional Substrate | 身份、authority、journal、object、invocation、stream、receipt、causal lineage |
| `H` | Host Control Plane | 安装、process、target、port、proxy、secret、deployment、本地 diagnostics |
| `C` | Protocol Commons | 共享语义契约、change workflow、projection、extension contract |
| `P` | Shell / Product Profile | Surface contribution、layout、interaction mapping、Product default |
| `X` | Mixed boundary | 当前行为混合了多个 layer 的职责 |

`plurora/*` 是第一方 Package publisher namespace，不是 layer，也不是 privilege class。

## 当前事实基线

- 80 个精确公开 method ID 与 80 个 method schema。
- 59 个显式平台 event kind 与 59 个 payload schema。
- 37 个顶层 schema；共 176 个 schema。
- Contract Registry `0.1.0` 对每个 method 只暴露一个 wire ID，不提供 alias。
- Method ID 通过第一个 dot segment 声明 owner。
- 平台 event owner 由显式 registry 定义；59 个 kind 都要求 writer `plurora/runtime`。
- Package event 与 capability ID 仍位于精确 Package ID 的 slash namespace 下。
- 显式 contract 与 Protocol Commons negotiation 在 dispatch 前完成，并 fail closed。

## Method owner prefix

| Prefix | Count | 当前 owner | 边界说明 |
|---|---:|---:|---|
| `context.*` | 6 | `S` | 内容无关的 execution/journal scope 与 lineage |
| `journal.*` | 3 | `S` | append、replay 与 subscription 边界 |
| `capability.*` | 5 | `S` | discovery、invocation、streaming、cancellation |
| `authority.*` | 7 | `S` | handle、grant、revocation、decision |
| `object.*` | 3 | `S` / `H` | put/get 接近 substrate；全局 list 更接近 Host |
| `identity.*` | 1 | `S` | authenticated principal/context discovery |
| `host.*` | 40 | `H` / `X` | Host-local operation；effect 仍依赖 substrate authority 与 receipt |
| `protocol.*` | 3 | `C` | extension contract discovery 与 subscription |
| `change.*` | 6 | `C` | approval-gated Change protocol facade |
| `projection.*` | 4 | `C` | derived-view protocol operation |
| `shell.*` | 2 | `P` | 可替换 Shell Profile contribution registry |

## Host method 细分

| Host area | Count | 分类 |
|---|---:|---|
| Package lifecycle 与 audit | 8 | `X`：Host artifact/process 与 runtime component evidence 混合 |
| Project | 5 | `H`：当前发行版的 installation-instance model |
| target / exec / port / proxy | 17 | `H`，并依赖 `S` authority 与 receipt evidence |
| outbound | 6 | `X`：Host network adapter 与 `S` policy、secret、stream、receipt 混合 |
| surface bundle resolution | 1 | `X`：Host serving 与 Shell Profile interpretation 混合 |
| info / ping / diagnostics | 3 | `H` |

Mixed classification 不会创建私有 API；它只指出实现还可继续拆分的位置，而当前公开契约仍保持精确且可测试。

## 平台 event owner

| Group | Count | Owner |
|---|---:|---:|
| Context | 3 | `S` |
| Package 与 Project lifecycle | 13 | `H` / `X` |
| Capability 与 stream lifecycle | 10 | `S` |
| Authority | 3 | `S` |
| Object | 1 | `S` |
| Projection | 1 | `C` |
| Change proposal | 5 | `C` |
| Outbound / WebSocket | 8 | `H` / `X` |
| Exec / port / proxy / deployment | 14 | `H` |
| Runtime error | 1 | `S` |

Owner 是语义归属；持久化仍统一使用 `EventEnvelope` 与 EventStore 边界。

## 顶层 schema owner

- **Substrate：** event envelope、protocol context/response、capability descriptor 与 invocation、permission set、artifact/effect evidence。
- **Host：** Package Manifest envelope、Installation / Run / Exposure / Realization、本地执行 descriptor、Target Inventory 与 Host-facing record。
- **Protocol Commons：** protocol descriptor、Change primitive、便携 Work / Assembly / Port / State Slot 契约、composition lock、World Bundle 与 World Head。
- **Shell Profile：** 通过公开 schema 承载的 contribution 与 profile descriptor。

部分顶层 schema 为 transport 连接多个 layer；字段仍必须说明 owner，而不能把所有 layer 压成一个 ontology。

## 变更纪律

1. 当前精确 ID 是现行公开契约。
2. 稳定后，owner 调整不能成为原地重写身份的理由。
3. Breaking change 需要新的 contract/profile/version 边界、migration tooling、旧数据可读性与 conformance vector。
4. Compatibility 不得积累为隐藏 dispatch alias 或第一方 shortcut。
5. Product 与 Shell convenience 不得静默变成 substrate responsibility。

## 当前拆分优先级

- 分离 Package artifact/install state 与 active Component execution evidence；
- 让 Host network/process adapter 始终位于 substrate authority 与 EffectReceipt 契约之后；
- 完成 Protocol-owned projection 与 extension semantics，但不向 substrate 加入内容 ontology；
- 通过显式 profile 保持 Shell slot 与 Product organization 可替换；
- 第一方与第三方 participant 始终共享同一公开 transport 与 authority path。
