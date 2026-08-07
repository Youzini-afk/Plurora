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
文件、secret、网络、target、部署、备份和诊断。
```

## 我们想建设什么

Plurora 长期追求五件事：

- **开放：** 源码、协议、数据和扩展入口公开；用户能导出、迁移和删除数据；官方实现没有私有 API。
- **多样：** 不固定唯一应用形态、工作流、Shell、模型、组件或内容本体。
- **先进：** 采用真正增加自由度、安全性、性能和可移植性的技术，而不是为了新而新。
- **长久：** 稳定层小、协议可演化、旧数据可读取、错误抽象能迁移和退出。
- **好用：** 官方发行版安装后能直接工作，简单路径简单，复杂能力按需展开，失败可恢复。

完整原则见 [`docs/CHARTER.md`](docs/CHARTER.md)，长期形态见 [`docs/architecture/VISION.md`](docs/architecture/VISION.md)。

## 平台与官方产品

Plurora 不只是内核，也不等于官方 Web/Desktop。当前官方发行版使用 Home、Settings、Project frame、Console 和可贡献 Surface，提供本地 managed Host、远程 Host、安装、运行、创作、部署、权限和数据管理等体验。

这些是正在持续打磨的默认产品选择，而不是整个平台的永久本体：

- 第三方可以替换客户端、Shell、组件、协议、模型和 Host；
- Project 是当前官方 Host 的安装实例模型，不是所有产品必须采用的根对象；
- Home / Play / Forge / Assist 属于可选产品 Profile，不属于宪法基底；
- 官方组件和客户端只使用第三方也能使用的公开边界。

总体产品责任见 [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md)。游创是可选强观点 Profile，见 [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.md)。

## 当前状态

仓库处于 Foundation Alpha：公开 Contract V1、Rust Host/runtime、HTTP/RPC/SSE、Package 与 Component 生命周期、Web/PWA、Tauri Desktop、CLI、安装更新、项目管理、权限、对象与工件、模型接入、受控开发、target 与部署等基础已经形成较大可运行面。

当前实现仍带有 Contract V1 的历史聚合：`kernel.v1.*` 同时包含部分基底、Host、协议和 Shell 语义。新的架构方向不是推倒重写，而是在保持兼容和数据可读的前提下逐步明确所有权。

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

packages/official/     通过普通 Manifest 加载的第一方组件与实验能力
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

安装和管理 Package / Project：

```bash
plurora install github.com/user/plurora-package#v1.2.0
plurora list-installed
plurora project list
plurora project start <project-id>
plurora project stop <project-id>
plurora uninstall <package-id-or-project-id>
plurora update [<package-id>|--project-id <project-id>] [--check-only]
plurora lockfile --check
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
| 理解分层架构 | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) → [`docs/architecture/PLATFORM_KERNEL.md`](docs/architecture/PLATFORM_KERNEL.md) → [`docs/architecture/CAPABILITY_PACKAGE.md`](docs/architecture/CAPABILITY_PACKAGE.md) |
| 理解官方产品 | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md) → [`docs/design/PLATFORM_UI_DESIGN.md`](docs/design/PLATFORM_UI_DESIGN.md) |
| 接入公开协议 | [`docs/protocol/PROTOCOL_V0.md`](docs/protocol/PROTOCOL_V0.md) → [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.md) |
| 看长期合同分层 | [`docs/architecture/CONSTITUTION_V2.md`](docs/architecture/CONSTITUTION_V2.md) → [`docs/spec/CONTRACT_LAYERING_MATRIX.md`](docs/spec/CONTRACT_LAYERING_MATRIX.md) |
| 写第一个 Package / Component | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 安装 Package / Project | [`docs/guides/PACKAGE_INSTALLATION.md`](docs/guides/PACKAGE_INSTALLATION.md) → [`docs/guides/PROJECT_MODEL.md`](docs/guides/PROJECT_MODEL.md) |
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
