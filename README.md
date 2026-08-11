# Plurora

> [English](./README.en.md) · [中文](./README.md)

**一个面向人、AI 与软件共同创造和运行的开放数字平台。**

Plurora 让应用、工具、服务、世界、游戏、agent、创作环境和未来尚未被命名的数字形态，能够被创建、组合、运行、审视、修改、迁移和替换。平台提供可信的共同基础，但不规定上层最终应该长成什么样。

```text
┌──────────────────────────────────────────────────────────┐
│ Products / Experiences / Services                        │
├──────────────────────────────────────────────────────────┤
│ Distributions / Shells / Clients                         │
├──────────────────────────────────────────────────────────┤
│ Protocol Commons / Components / Content                  │
├──────────────────────────────────────────────────────────┤
│ Constitutional Substrate                                 │
└──────────────────────────────────────────────────────────┘

Host Control Plane / Runtime Fabric 横跨各层，管理安装、执行、
文件、secret、网络、target、Realization、备份和诊断。
```

## 我们想建设什么

Plurora 长期追求五件事：

- **开放：** 源码、协议、数据和扩展入口公开；用户能导出、迁移和删除数据；第一方实现没有私有 API。
- **多样：** 不固定唯一应用形态、工作流、Shell、模型、组件或内容本体。
- **先进：** 采用真正增加自由度、安全性、性能和可移植性的技术，而不是为了新而新。
- **长久：** 稳定层小、协议可演化、旧数据可读取、错误抽象能迁移和退出。
- **好用：** 官方发行版安装后能直接工作，简单路径简单，复杂能力按需展开，失败可恢复。

完整原则见 [`docs/CHARTER.md`](docs/CHARTER.md)，长期形态见 [`docs/architecture/VISION.md`](docs/architecture/VISION.md)。

## 平台与官方产品

Plurora 不只是内核，也不等于官方 Web/Desktop。当前官方发行版使用 Library、Settings、Installation frame、Console 和可贡献 Surface，提供本地 managed Host、远程 Host、安装、Run、Powerbox、Realization、权限和数据管理等体验。

这些是正在持续打磨的默认产品选择，而不是整个平台的永久本体：

- 第三方可以替换客户端、Shell、组件、协议、模型和 Host；
- Work、Installation 与 Run 分别表达逻辑作品、某台 Host 上的安装实例和一次运行；它们不属于宪法基底；
- Home / Play / Forge / Assist 属于可选产品 Profile，不属于宪法基底；
- 第一方组件和客户端只使用第三方也能使用的公开边界。

总体产品责任见 [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md)。游创是可选强观点 Profile，见 [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.md)。

## 当前状态

仓库处于 Foundation Alpha：公开 Contract V1、Rust Host/runtime、HTTP/RPC/SSE、Package 与 Component 生命周期、Web/PWA、Tauri Desktop、CLI、Work 打包、Installation 更新、Run 生命周期、权限、对象与工件、模型接入、受控开发、target 与 Realization 基础已经形成较大可运行面。

当前 Contract V1 通过 99 个精确的 owner-based method ID、76 个平台事件和 39 个顶层 schema（共 214 个 schema）分别表达 Substrate、Host、Protocol 与 Shell 职责。Exposure/Binding/Powerbox 与 managed Realization 均已实现；实现仍会在该公开边界之后继续拆分，但不会创建第一方私有路径或并行 wire identity。

具体已实现、partial 和 deferred 状态见 [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md)。当前建设方向见 [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.md)。

## 仓库一览

```text
crates/
  plurora-core/            当前核心类型、schema、身份、事件与合同对象
  plurora-runtime/         runtime、组件执行、协议调度与部分 Host 能力
  plurora-service/         HTTP / RPC / SSE 与 Host service 边界
  plurora-cli/             CLI、Host、脚手架、contract 与 conformance 工具

clients/web/           官方 React 19 + Tailwind v4 + Vite Web Shell / PWA
clients/desktop/       Tauri 2.x wrapper + managed Host sidecar

packages/plurora/     通过普通 Manifest 加载的第一方组件与实验能力
profiles/              发行版 / Host 的组件与策略组合
examples/              示例、fixture 与第三方接入样例

sdk/typescript/        TypeScript SDK 与子进程组件工具
sdk/rust/              生成的 Rust contract SDK
docs/                  章程、架构、协议、产品、指南、状态与路线图
integrations/          外部项目和生态接入调研
```

代码目录反映当前实现，不单凭 crate 名称决定永久架构归属。

## 快速上手

启动 Host：

```bash
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

构建或检查 Web Shell：

```bash
npm run check --prefix clients/web
npm run build --prefix clients/web
```

运行测试与 conformance：

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
```

查看 Work / Installation 命令，并通过公开 Host 契约管理 Installation：

```bash
plurora work --help
plurora installation list
plurora installation info <installation-id>
plurora installation create --help
plurora installation update --help
plurora installation remove --help
plurora realization plan --help
plurora realization apply --help
```

通过公开协议运行空白游创示例：

```bash
cargo run -p plurora-cli -- play-create-demo
```

更多命令和组件创作流程见 [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md)。

## 文档导航

每篇主要文档都有英文与简体中文版本，顶部可切换语言。完整索引见 [`docs/README.md`](docs/README.md)。

| 你想 | 先读 |
|---|---|
| 理解平台目标 | [`docs/CHARTER.md`](docs/CHARTER.md) → [`docs/architecture/VISION.md`](docs/architecture/VISION.md) |
| 理解分层架构 | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) → [`docs/architecture/CONSTITUTIONAL_SUBSTRATE.md`](docs/architecture/CONSTITUTIONAL_SUBSTRATE.md) → [`docs/architecture/CAPABILITY_PACKAGE.md`](docs/architecture/CAPABILITY_PACKAGE.md) |
| 理解官方产品 | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md) → [`docs/design/PLATFORM_UI_DESIGN.md`](docs/design/PLATFORM_UI_DESIGN.md) |
| 接入公开协议 | [`docs/protocol/PUBLIC_PROTOCOL.md`](docs/protocol/PUBLIC_PROTOCOL.md) → [`docs/spec/PUBLIC_CONTRACT.md`](docs/spec/PUBLIC_CONTRACT.md) |
| 看长期合同分层 | [`docs/architecture/CONSTITUTION_V2.md`](docs/architecture/CONSTITUTION_V2.md) → [`docs/spec/CONTRACT_LAYERING_MATRIX.md`](docs/spec/CONTRACT_LAYERING_MATRIX.md) |
| 写第一个 Package / Component | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 打包 Work / 创建 Installation | [`docs/guides/PACKAGE_INSTALLATION.md`](docs/guides/PACKAGE_INSTALLATION.md) → [`docs/guides/INSTALLATION_MODEL.md`](docs/guides/INSTALLATION_MODEL.md) |
| 使用 Library / 启动和停止 Run | [`docs/guides/RUN_LIBRARY.md`](docs/guides/RUN_LIBRARY.md) |
| 使用 Powerbox / 连接跨 Installation Port | [`docs/guides/POWERBOX_BINDING.md`](docs/guides/POWERBOX_BINDING.md) |
| 把 Work 编译并应用到 Target | [`docs/guides/REALIZATION.md`](docs/guides/REALIZATION.md) |
| 管理 API key / secret | [`docs/guides/SECRET_MANAGEMENT.md`](docs/guides/SECRET_MANAGEMENT.md) |
| 写 agent / 模型 / 体验组件 | [`docs/guides/AGENT_PACKAGE_AUTHORING.md`](docs/guides/AGENT_PACKAGE_AUTHORING.md)、[`docs/guides/MODEL_PROVIDER_INTEGRATION.md`](docs/guides/MODEL_PROVIDER_INTEGRATION.md)、[`docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md`](docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md) |
| 挂载第三方 Web Surface | [`docs/guides/SURFACE_HOSTING.md`](docs/guides/SURFACE_HOSTING.md) |
| 看当前状态 | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md) |
| 看建设方向 | [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.md) |
| 写文档 | [`docs/STYLE.md`](docs/STYLE.md) |

## 可以在 Plurora 上发展的形态

聊天、世界模拟、Tavern、游戏引擎桥接、IDE、普通 Web 服务、agent runtime、文档工具、节点编辑器和市场，都可以成为平台上的产品、协议、组件或发行版。它们没有一项是平台唯一中心，也没有一项因为“官方”而获得隐藏权威。

YdlTavern 是一个独立接入项目，边界见 [`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.md)。

## 许可证

Plurora 以 GNU Affero General Public License v3.0（仅此版本，`AGPL-3.0-only`）发布，详见 [`LICENSE`](LICENSE)；第一方代码、外部依赖和第三方内容的边界见 [`docs/LICENSING.md`](docs/LICENSING.md)。
