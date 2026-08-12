# Powerbox、Exposure 与跨 Installation Binding

> [English](./POWERBOX_BINDING.en.md) · [中文](./POWERBOX_BINDING.md)

本指南描述跨 Installation Powerbox。它把 provider 的公开 Exposure 与 consumer 的 import Port 连接起来，但不把候选、偏好或 UI 变成执行权。Powerbox 不会隐式 plan、apply Realization 或 rebind。

## 权威与隔离

- Host journal 是 Exposure、Binding、Installation 与 Realization 的 durable authority；公开 relay 只返回经筛选的记录，private intent/handle 留在 Host 内部。
- Exposure 状态为 `active → closing → revoked/expired`；Binding 决策为 `selected/active → closing → terminal`。关闭先于 terminal commit，重复 close/revoke/expiry 通过幂等 journal 结果收敛。
- Host restart、owner takeover、retry 或 effect 结果不确定时，不猜测成功；记录 `outcome_unknown`/`recovery_required` 或等价的 terminal 诊断，并通过新记录恢复。
- Exposure 只允许 provider Installation、Run 与 exact export Port；Binding 只允许 consumer Installation、import Port、Exposure，并可选 exact consumer Run。资源 selector 不因省略 ID 自动扩大。
- `Port` 的复合资源 ID 是 `<installation-id>/<port-id>`；它不是路径，也不是 capability handle。grant basis 只保存哈希后的授权依据，不保存 credential。
- 每个外部 effect（创建/撤销 Exposure、选择/撤销 Binding、注入或停止）前都重新验证 grant、祖先 delegation、lease、owner 与资源匹配。

## 公开方法与事件

这些公开方法全部属于 Host、状态为 `implemented`，请求/结果均有 typed schema：

```text
host.exposure.list
host.exposure.create
host.exposure.revoke
host.binding.list
host.binding.candidates
host.binding.select
host.binding.revoke
```

对应的事件为：

```text
host/exposure.created
host/exposure.revoked
host/exposure.expired
host/binding.selected
host/binding.revoked
host/binding.expired
```

完整 payload 见 [`../spec/v1/EVENT_KIND_REGISTRY.md`](../spec/v1/EVENT_KIND_REGISTRY.md)，请求/结果见 [`../spec/v1/schemas/methods/`](../spec/v1/schemas/methods/)。这些事件只能由 `plurora/runtime` 写入；Package 不能伪造它们。

## Exposure 与 Binding 数据

`host.exposure.create` 必须提供 provider Installation、Run、export Port、exact audience、expiry、expected revisions 与 `idempotency_key`。Audience 是显式资源选择器，不能用 publisher 或默认设备身份代替。

`host.binding.candidates` 是 effect-free 查询，必须提供 consumer Installation、import Port、`phase` 与 expected revision；可选 `preferences` 只是排序提示。结果包含：

- exact Exposure、audience 与 effective expiry；
- consumer/provider 两端的 `PortDescriptor`；
- `PortContract` 的 protocol、interface、version 与 profiles；interaction model、effect class、transport、multiplicity、availability 与 latest binding phase；
- provider Work/Installation source 与 identity；
- provider Component 的 version、entry kind、artifact/behavior digest、trust class、claim status、enforced boundaries 与 protocol implementations；
- `candidate_digest`、stale/unknown gap 与 next step。

候选按稳定规则排序，不按 publisher；0 个候选或多个候选都不自动选择。当前 provider candidate 返回上限是 256，超限返回结构化 `work_too_complex`，而不是静默截断。未知字段或 interaction 不猜测、不降级。

`host.binding.select` 必须回传选中的 `candidate_digest`、Exposure 与 provider/consumer revisions；选择后 Host 生成 opaque Binding ID。`host.binding.revoke` 只能针对 exact consumer/Port/Exposure/Binding 与 expected revision。

## Powerbox disclosure 与用户选择

官方 Web chooser 先显示明确的 binding phase（authoring、installation、launch 或 runtime），再显示 exact Exposure、audience、expiry 与 provider 来源。两端 PortContract 必须同时展示 protocol、interface、version、profiles、interaction、effects、transport、multiplicity 与 availability。

Provider disclosure 分开呈现：Work/Installation 的 source、Component 的 trust class、claim 与 enforced boundary、artifact/behavior digest、protocol implementation evidence，以及可能过期的 `stale` 状态。第一方与第三方遵守同一规则；稳定顺序不使用 publisher priority。

Preference hint 只能影响展示顺序，不是 authority；不能把候选写入 active Binding，也不能隐式 apply Realization、rebind 或扩大 scope。未知值保持可见并要求用户或显式 policy 决定。

## Runtime pin 与注入

- Host 只向选中的 Component 注入最小 runtime handle；handle、credential、grant basis 与 private intent 不出现在 UI、公开事件或 receipt。
- 每个 Binding pin 记录 consumer/provider Installation revision、Work/AssemblyLock、root Port、完整 `node_path` 与 leaf Port、Component artifact/behavior/trust，以及 capability version。
- Launch phase 不带 consumer Run pin；Runtime phase 必须同时 pin `RunId`、`run_revision` 与 `context_id`。同一 Component 的多个 Port 可以共享一次 activation，但不同 component 或不同 `node_path` 必须隔离。
- Unary 与 stream 都遵循同一 authority、deadline、cancel、backpressure 与 receipt 语义；stream 在建立 barrier 后才交付帧。
- provider stop、Exposure revoke/expiry、authority revoke、版本漂移或 pin digest 不匹配会取消 Binding。Host 不自动改绑；consumer 按 `availability`（required、degraded_without、optional）处理。
- 不跨 Installation 共享 state、secret 或 activation context。Subprocess invocation token 与 background work 必须显式声明、单独授予和可撤销；不能从 UI 或普通 event 推导。

## CLI、Web 与 Surface 边界

CLI 使用公开命令：

```bash
plurora exposure list|create|revoke
plurora binding candidates|list|select|revoke
```

Home/Library 负责发现与入口；Installation frame 提供 chooser 和 exact Run/Installation context。PWA/mobile 使用同一 Host API，Host cache 按 Host/Installation 隔离；关闭 tab、iframe 或 PWA 连接不会停止 Run，也不会 revoke Exposure/Binding。

Surface 只有显式 allowlist 的公开 bridge；没有 Powerbox private bridge、root credential 或隐藏的 first-party route。Surface 不能直接选择 provider、读取 handle、apply Realization 或创建 rebind。

## 验证与边界

6 个 Powerbox conformance case 覆盖 method owner/status/typed DTO、action-before-effect、76-event registry 中对应 payload、无 private authority、effect-free candidates/no auto-select，以及 launch/runtime exact Run pin：

```text
powerbox.public_method_identity_owner_typed_dto
powerbox.public_method_actions_no_effect_on_denial
powerbox.public_event_identity_payload
powerbox.public_wire_no_private_authority
powerbox.candidates_effect_free_no_auto_select
powerbox.launch_runtime_exact_run_pin
```

当前公开合同为 99 methods、76 events、39 top-level、214 schemas；Realization 已实现且不改变 Powerbox 的 least-authority 边界。

相关文档：[`RUN_LIBRARY.md`](RUN_LIBRARY.md)、[`REALIZATION.md`](REALIZATION.md)、[`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md)、[`../architecture/HOST_RESOURCE_AUTHORITY.md`](../architecture/HOST_RESOURCE_AUTHORITY.md)、[`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md)。
