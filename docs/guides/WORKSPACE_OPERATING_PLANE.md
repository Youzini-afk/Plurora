# Workspace 操作平面

> [English](./WORKSPACE_OPERATING_PLANE.en.md) · [中文](./WORKSPACE_OPERATING_PLANE.md)

Workspace 是某台 Host 上用于检查、创作或构建的可变源码位置。它与可移植 Work identity、Host-local Installation 和后续 Run 分离；源码仓库存在并不代表它已经成为可运行组件。

## 所有权

- `managed`：Host 把受控副本放在 `<data>/workspaces/<workspace-id>/source/`。
- `linked_local`：Workspace 记录指向用户已有目录的绑定；Host 没有删除或改写该源的权威。

linked-local source 永远不会因 Installation update/remove、失败清理或 recovery 被删除。需要写入时，先导入 managed 副本并通过独立 ChangeSet 审批。

## Intake

没有显式 Work source 的普通仓库先经过静态 inspection：技术栈、候选入口、构建图、风险摘要与 adapter plan。它不会因包含源码就自动 install dependencies、run scripts、build、test 或联网。

Install Lab 可以把外部 URI、本地 executable、OCI 或远程服务归一化为 Foreign Capsule WorkRevision；具体 URL、可执行路径或 credential 只进入 Host-local acquisition / binding，不进入 portable artifact。

## 文件安全

managed materialization 沿用已有的真实风险边界：

- canonical root containment；
- 每级祖先与末端拒绝 symlink/reparse escape；
- 25,000 文件/目录与 256 MiB copy 预算；
- HTTPS Git 拒绝内联 credential、query 与 fragment；
- 不支持的 tree mode 明确失败；
- staging 与原子 promotion，失败不覆盖现有 Workspace。

这些限制防止无界复制、路径逃逸与凭据进入 descriptor，不是面向产品功能的任意配额。

## 计划与效果分离

普通 Package 与 agent 可以产生 inspection、workspace plan、patch proposal、adapter preview 和 verifier plan；这些输出不授予执行 authority。

真实文件 effect 进入 Host development control plane：

```text
Intent → ChangeSet → PolicyDecision → ChangeCommit → EffectReceipt
```

调用按唯一 subject 路由：`workspace/<workspace-id>` 或 `installation/<installation-id>`。`develop.propose`、`develop.approve` 与 `develop.execute` 是不同 scope；批准对象绑定精确 operations、验证方式、所需 authority 与预期 effect，批准后不能替换内容。

首版真实执行只接受有界、类型化 file write/delete 与显式 verifier。Docker 默认无网络；没有 ambient shell、Host mount、build secret 或第一方 bypass。

## 与 Installation 的关系

Workspace 可以生成新的 WorkRevision / AssemblyLock candidate，但不能直接修改 Installation active pointer。操作者仍需调用 `host.installation.update`，提交 expected revision 与显式 state action。更新失败保留旧 Work/Lock pointer。

## Web 与 CLI

Web 与 CLI 只通过 public protocol / Host API 读取 projection、草拟和审批 ChangeSet；它们不直接读 SQLite、Host filesystem 或 executor 内部状态。

Work 打包与 Installation 采用见 [`PACKAGE_INSTALLATION.md`](PACKAGE_INSTALLATION.md)；完整受控变更边界见 [`../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md`](../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md)。
