# Modular Simulation Work Kit

> [English](./MODULAR_SIMULATION.en.md) · [中文](./MODULAR_SIMULATION.md)

`modular-simulation` 是第一个面向继续创作而不是一次性演示的 Work kit。它选择 5×5 殖民地经营这个规则驱动、状态丰富的题材，证明 Work、Assembly、Port、portable state、Surface、Run、Powerbox、Realization 与替换可以组成一条完整路径，而不引入通用 3D 引擎或第一方私有接口。

## 组成

| 部分 | 实现 | 边界 |
|---|---|---|
| 模拟 Component | `plurora/modular-simulation` Rust `rust_inproc` Package | 状态全部作为输入/输出传递；没有进程全局游戏状态、隐式网络或 secret |
| 输入 Port | Assembly export `input` → `apply_input` | `build`、`assign`、`advance` 是确定性 reducer 操作 |
| portable save | Assembly export `save` → `export_save`，Installation-scoped `save` StateSlot | canonical JSON + SHA-256；backup required；schema 为 `modular-simulation.save.v1` |
| 可选 AI | Runtime import `ai-advisor`，以及独立 `example/ai-advisor` provider Work | 只通过显式 Powerbox Binding 调用；provider 已加载但没有 Binding 时仍返回 `binding_unavailable`，不会环境发现 |
| Web Surface | `plurora/modular-simulation-renderer` 静态 `surface_bundle` | `play_renderer` 与 `asset_editor`；只通过 Surface bridge 调用 allowlist 内的 capability |
| 本地 Run | `host.run.*` + `AssemblyRuntimeDriver` | exact AssemblyLock/Component pin、Run session 与 Package lease；关闭页面不停止 Run |
| 远端 server fork | `example/modular-simulation-server` Work | `OperationalIntent` 只触发 effect-free Realization plan；没有隐式 build/apply |
| 社区替换 | `community/modular-simulation` | 与第一方实现走相同 manifest、Component、Port 和 provider 路由；两个 provider 同时存在时必须显式选择 |

这里的“隔离”首先是 Component、状态和权限边界：参考 Rust 实现当前使用 Host catalog 中的 `rust_inproc` 后端，因此不是 OS 进程沙箱。它不因第一方身份获得额外 authority；需要进程隔离的发行版可以在保持相同 Component/Port/state 合同的前提下换成 `subprocess` 实现。

## 运行与检查

```bash
plurora work check examples/works/modular-simulation --json
plurora work check examples/works/modular-simulation-community --json
plurora work check examples/works/modular-simulation-server --json
plurora work check examples/works/modular-simulation-ai-advisor --json

plurora work promote-component examples/works/modular-simulation \
  --node simulation \
  --assembly-id example/modular-simulation-core \
  --json

plurora conformance --tag phase8
```

Forge profiles 自动加载模拟、renderer 与本地 advisor Package。要使用 advisor，仍需把 `example/modular-simulation-ai-advisor` 作为普通 Work 安装/运行，创建其 Exposure，再为 consumer 的 `ai-advisor` Port 显式选择 Binding。安装基础 Work 后，Library 的 Play 操作仍先执行 Run preflight；本地 artifact、exact Component pin 或必需 Binding 缺失时会显示结构化 gap，不会触发部署。`request_ai_move` 即使看到已加载的兼容 capability，也只能使用为当前 Run/Port 选择并重新验证的 Binding。

## 状态、fork 与替换

模拟状态是一个版本化的 portable document，不藏在 Package 单例里：

- `create_state` 创建 v1 状态；
- `apply_input` 返回下一 revision，不修改隐藏状态；
- `export_save` 对 canonical bytes 计算稳定摘要；
- `migrate_save` 将 v0 显式转换为 v1，并对已有 v1 幂等；
- `inspect_state` 只返回结构化统计；
- `server_tick` 使用同一 reducer，供 server entrypoint 使用。

`examples/works/modular-simulation-community` 用普通第三方 Package 替换模拟节点，同时保留兼容 Port 与 save schema。替换 durable state 时仍遵守 StateSlot 规则：schema/owner/portability 不兼容时，必须提供 migration Port 或由用户明确批准 reset。`examples/works/modular-simulation-server` 是独立 WorkRevision fork；它的 remote-server 意图不修改基础 Work，也不自动创建 Realization。

## 提升为可复用组件

`plurora work promote-component` 接收一个或多个根 Assembly node，确定性计算：

- 选中子图内部保留的 binding；
- 被切断的外部 import/export；
- 已有 root exposure；
- 由选中节点拥有的 StateSlot；
- 新 nested Assembly 与结构化 diagnostics。

输出是内容寻址 `AssemblyPromotionCandidate` 和 nested `AssemblyRevision`。命令明确报告 `persisted:false`、`published:false`：它不写 ObjectStore、不改 Work、不安装 Package，也不替 Agent 发布。创作者审查 candidate 后，才可以在独立的受权变更中保存或发布它。

## 目录

- 基础 Work：`examples/works/modular-simulation/`
- server fork：`examples/works/modular-simulation-server/`
- 社区替换：`examples/works/modular-simulation-community/`
- 可选 advisor provider：`examples/works/modular-simulation-ai-advisor/`
- Rust Package：`packages/plurora/modular-simulation/`
- 静态 renderer：`packages/plurora/modular-simulation-renderer/`
- 社区 Package：`examples/packages/community-modular-simulation/`
- advisor Package：`examples/packages/modular-simulation-ai-advisor/`

这些目录是可检查的 source input；运行时身份仍由生成的 WorkRevision、AssemblyRevision、AssemblyLock、Package envelope 与 digest 决定，路径本身不是 authority。
