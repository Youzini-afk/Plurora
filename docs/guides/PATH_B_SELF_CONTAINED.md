# 路径 B：自包含包

> [English](./PATH_B_SELF_CONTAINED.en.md) · [中文](./PATH_B_SELF_CONTAINED.md)

路径 B 是内核 v1 的 opt-out 模式。包通过 `entry.contract: "none"` 声明自己不参与 v1 capability / permission enforcement。内核仍托管它、记录生命周期、暴露 operator 可见性，但不为它注入能力句柄。

## 什么是路径 B

路径 B 包是自包含应用或工具。它可以作为 Plurora host 管理的进程运行，但它不通过 manifest 权限获得平台权威，不使用 `capability.invoke`，也不依赖 v1 bindings。

这不是“低级”路径。它是给迁移期、兼容层、既有工具和原型保留的一等参与方式。

## 什么时候使用

- 移植已有应用，先让它在 host 生命周期里跑起来。
- 运行不需要 Plurora 能力的第三方工具。
- 原型阶段还没决定能力边界。
- 只需要被 host 启停和观测，不需要平台权限。
- 自带网络、存储或 UI sandbox，且接受不由内核 v1 强制。

## 什么时候不要使用

不要在以下情况选择路径 B：

- 包需要调用其他 capability provider。
- 包需要通过 manifest 声明获取网络访问。
- 包需要通过 `secret_ref` 让 host 注入 secret。
- 包需要 events.read / events.append 权限。
- 包需要 declared vs used authority audit。
- 包要作为可复用平台能力被其他包调用。

这些场景应该使用路径 A：`entry.contract: "v1"`。

## Manifest 形状

最小形状：

```yaml
id: example/self-contained
version: 0.1.0
entry:
  kind: subprocess
  contract: "none"
  command: ["./run-example"]
```

路径 B 可以保留描述性 metadata、surface descriptor 或 host 启动所需字段，但权限声明不会产生 v1 能力句柄。

## 会发生什么变化

| 行为 | 路径 A | 路径 B |
|---|---|---|
| Manifest 权限强制 | 是 | 否 |
| Capability handles | 注入 | 不注入 |
| Reverse kernel calls | 受句柄约束 | 不可用或按 host 策略拒绝 |
| Secret resolution | `secret_ref` + host resolver | 不提供 v1 secret binding |
| Network outbound | `platform.outbound.*` + audit | 不由 v1 outbound 管理 |
| Lifecycle events | 发出 | 发出 |
| Package logs | 可捕获 | 可捕获 |
| Conformance kit | 权限与调用检查 | 自包含与可观察检查 |

## 生命周期与审计可见性

路径 B 仍会有 package lifecycle events：loading、starting、ready、stopping、stopped、unloaded、degraded、log。事件 payload 应包含或可派生：

```json
{ "contract_mode": "none" }
```

这样 operator 可以清楚地区分：该包由 host 管理，但不受 v1 capability enforcement 保护。

## 安全含义

路径 B 是显式信任边界。内核不会声称拦截其所有副作用。Host 仍可以使用 OS sandbox、容器、profile policy、文件系统权限、网络隔离或用户提示来约束它，但这些不是 v1 capability contract。

如果你需要可审计、可撤销、最小权限的能力，请使用路径 A。

## Conformance 行为

`plurora conformance package --contract v1 --path <package>` 会识别 `entry.contract: "none"`：

- manifest parse：PASS/FAIL；
- contract mode：PASS，标记 Path B；
- entry support：PASS/FAIL；
- bindings、capability、permission：SKIP 或 WARNING；
- lifecycle visibility：PASS/FAIL；
- fixture invocation：只检查自包含 smoke。

路径 B 包可得到 100% 合规，因为不适用项不计入分母。

## 路径 A vs 路径 B

| 维度 | 路径 A (`v1`) | 路径 B (`none`) |
|---|---|---|
| 主要用途 | 平台能力包 | 自包含应用/迁移工具 |
| 权威来源 | 内核句柄 | 包自身/host 外部策略 |
| Manifest 权限 | 强制执行 | 描述性，不授予 v1 权威 |
| SDK | 生成 SDK + bindings | 可不用 SDK |
| 审计 | declared vs used | lifecycle + mode 标记 |
| 适合第三方复用 | 是 | 通常否 |
| 最小权限 | 内核强制 | host/OS 自行处理 |

## 迁移建议

可以先用路径 B 让既有工具跑起来，再逐步迁移到路径 A：

1. 收集实际副作用和调用需求。
2. 把网络、secret、capability 调用写进 manifest。
3. 接入 bindings 与 SDK。
4. 运行 conformance kit。
5. 切换 `entry.contract` 到 `"v1"`。

## Operator 提示

- 把路径 B 视为外部进程信任边界，而不是 v1 sandbox。
- 在 package list 或 dashboard 中显示 `contract_mode: none`。
- 对路径 B 包使用更明确的 profile 审批。
- 若路径 B 包需要网络或文件系统，优先用 OS/container policy 管理。
- 定期评估它是否应迁移到路径 A。

## 包作者提示

- README 中说明为什么选择路径 B。
- 不要在 manifest 中暗示 v1 权限会被强制。
- 若将来需要 secret 或 outbound，提前设计迁移路径。
- 保持 stdout/stderr 约定，避免破坏 host lifecycle。
- 提供健康检查或最小 smoke，帮助 conformance kit 判断可启动。

## 常见误解

路径 B 不是“不安全模式”的同义词。它只是说明安全边界不由内核 v1 capability contract 提供。一个路径 B 包仍可由 OS sandbox、容器、只读文件系统、网络隔离和人工审批保护。

路径 B 也不是绕过审计。Host 仍应记录 lifecycle、logs、exit status 和 contract mode；只是不会有 declared vs used authority 报告。

## 参考

- [`../spec/PUBLIC_CONTRACT.md`](../spec/PUBLIC_CONTRACT.md)
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.md)
- [`CONFORMANCE_KIT.md`](CONFORMANCE_KIT.md)
