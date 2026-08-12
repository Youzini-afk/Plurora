# Host 资源权威

> [English](./HOST_RESOURCE_AUTHORITY.en.md) · [中文](./HOST_RESOURCE_AUTHORITY.md)

状态：**Candidate 实现**。Host Access 把 root、设备、CLI、Web/PWA、Desktop 与未来 Agent 的认证身份，统一衰减为 action scope 与结构化 resource selector。它保护 Host-local Work / Workspace / Installation / Run / Target / Exposure / Binding / Realization，但不会把这些对象提升为宪法基底概念。

## 不变量

- HTTP、Cookie、Bearer、stdio 与 in-process transport 使用同一认证与授权语义。
- 请求 body 不能覆盖认证 principal、grant、delegation chain 或已验证资源。
- 精确公开 method 先由 Contract Registry 解析；未知 method 没有 fallback。
- 列表在服务端按可见资源过滤，不能先返回全集再由客户端隐藏。
- session id、URL 参数、display name 与本地路径都不是 capability。
- allow/deny decision 可关联 principal、grant、resource 与后续 receipt，但不记录凭据或 raw secret。

## Action scope

当前 action wire 值：

```text
observe
installation.manage
run
binding.manage
exposure.manage
realization.plan
realization.apply
develop.propose
develop.approve
develop.execute
access_manage
```

底层 target/exec/port/proxy adapter 不是普通设备的 Realization 权限；它们只对 HostAdmin/HostDev 或显式 `access_manage` 管理边界开放。Managed lifecycle 只通过 `realization.plan` / `realization.apply`。

Exposure/Binding 与 Realization 方法使用这些 action：

| Action | Methods | Exact resources |
|---|---|---|
| `observe` | `host.exposure.list`, `host.binding.list`, `host.binding.candidates` | 可见的 provider/consumer Installation、Run、Exposure、Binding 与 Port |
| `exposure.manage` | `host.exposure.create`, `host.exposure.revoke` | provider Installation + Run + export Port；revoke 另含 exact Exposure |
| `binding.manage` | `host.binding.select`, `host.binding.revoke` | consumer Installation + import Port + Exposure；revoke 另含 exact Binding；Runtime 可带 exact Run pin |
| `realization.plan` | `host.realization.plan` | exact Installation + Target；只生成并持久化内容寻址 plan，不产生 target effect |
| `observe` | `host.realization.list`, `host.realization.get` | list 按可见 Installation/Target/Realization 过滤；get 要求 exact Installation + Realization |
| `realization.apply` | `host.realization.apply`, `stop`, `rollback`, `reconcile` | exact Installation + Target + current Realization；rollback 另要求 exact historic Realization |

Port 的复合资源 ID 使用 `<installation-id>/<port-id>`。它只是 resource selector 的稳定拼接形式，不是 filesystem path、credential 或 handle。

## Resource selector

资源种类是：

```text
work
workspace
installation
run
target
exposure
binding
realization
```

Selector 线路形状：

```json
{"kind":"installation","id":"018f2b74-..."}
{"kind":"installation","id":null}
```

`id: null` 是显式 wildcard；省略 `id` 会拒绝，不能因字段缺失获得全局可见性。子 grant 的 actions、resources、期限和 delegation depth 必须都是父权威的子集。Exposure audience 也必须显式列出 exact selector；candidate relay 只返回 caller 可见且 audience 匹配的 provider。

## 调用上下文

认证后 transport 构造不可由请求覆盖的上下文：

```text
AuthenticatedCallContext
  principal_ref
  credential_kind
  grant_ref?
  delegation_chain[]
  authority_refs[]
  transport
  audience_host_id
  issued_at / expires_at?
  correlation_id / parent_invocation_id?
```

Host 在 dispatch 前从已解析参数与服务端 projection 提取资源：

```text
HostOperationContext
  authenticated_call
  action
  resources[]
  installation_ref?
  workspace_ref?
  run_ref?
  target_ref?
  operation_ref?
  policy_decision_ref
```

运行时只消费这份验证后的上下文，或由它铸造的衰减 handle。Host journal 保存 durable authority；公开 relay 只发送过滤后的 typed projection，private intent、grant basis（仅哈希）与 handle 不出现在 wire、UI 或事件中。

## 固定授权顺序

1. transport 验证凭据并建立认证上下文；
2. Contract Registry 解析精确 method；
3. resource extractor 从参数与服务端 projection 得到资源；
4. 校验对象归属与跨引用一致性；
5. policy engine 计算 action × resources × authority；
6. 写入脱敏 policy decision；
7. 通过后才进入 runtime 或产生外部效应。

资源未知、归属冲突、selector 缺失、grant 过期或祖先已撤销都 fail closed。

## Installation 与 Run

- `host.installation.list|get` 要求 `observe`；list 只返回 caller 可见的 Installation。
- `host.installation.create` 要求 `installation.manage` 和请求 `work_id` 对应的精确 Work；canonical CAS 中 `WorkRevision.work_id` 必须与请求一致。`update|remove` 要求精确 Installation。
- Runtime 为 create、所有 update state action（包括 `preserve`）与 remove 铸造不可 wire 构造的 Host-only sidecar。服务在等待 apply lock 后及每个 durable/effect 边界前同步 HostAccess journal，并重新检查 grant active/expiry/delegation/action/精确资源；请求字段本身不授予权限。
- Installation-local secret scope 来自 Host 验证的 Installation context。
- Run start 要求 `run` 与精确 Installation。RunId 由 Host 在 preflight 后生成；后续 get/stop 必须同时携带 I/R，registry 先证明 Run 是该精确 Installation 的 child，才从父 selector 派生本次 exact child authority。该规则不跨 Installation，也不是第一方私有 bypass；Host restart 会把 active Run 标为 `interrupted`。
- package surface 只能拿到短期、方法 allowlist 的衰减 handle，不能获得 root/device credential。
- Binding runtime 只向选中的 Component 注入最小 handle；provider stop、Exposure revoke/expiry、owner revoke 或版本漂移会在每个 effect barrier 重新校验并使 Binding 失效。
- Realization plan 固定 Installation revision、Target inventory 与 plan digest；apply/stop/rollback/reconcile 在每个 durable/effect 边界前重新验证当前 grant、exact resources、owner lease 与 revision。Plan 和 approval 不是 authority，private effect checkpoint 与 receipt 也不进入 public wire。

## 审计

敏感调用至少关联：principal、credential kind、grant id、delegation-chain digest、canonical method、action、resource refs、allow/deny reason、correlation/causation，以及后续 receipt 或 terminal failure。凭据原文、Cookie、secret 值和完整请求 payload 永不进入 journal。

## 威胁与防线

| 失败模式 | 防线 |
|---|---|
| Installation A 的 grant 操作 B | 精确 selector 与服务端 projection 交叉验证 |
| 省略 selector id 获得 wildcard | wire 必须显式 `id: null` |
| transport 自建别名绕过 policy | 所有 transport 共用解析后的 `PlatformMethod` 与 policy table |
| 设备身份折叠成无约束 HostDev | 保留 authenticated principal 与 grant envelope |
| 列表或事件流泄露其他资源 | 服务端过滤并固定 subscription scope |
| grant 撤销后继续创建新副作用 | 每个新 effect 前重新水化 grant / ancestor 状态 |
| iframe 偷取 Host token | 只暴露短期 handle、方法 allowlist 与 bundle-root lease |

## 完成门槛

- exact-resource 设备对其他资源的 get/update/remove/secret/develop/effect 全部拒绝；
- 伪造 context、未知 method、direct transport 与重放 grant 都不能绕过 policy；
- revoke、expiry、delegation attenuation 与批量撤销有并发覆盖；
- 审计可从用户动作连接到 policy decision 与 effect receipt，且不泄露凭据。
