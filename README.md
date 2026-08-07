# Yggdrasil

> [English](./README.en.md) · [中文](./README.md)

**一个面向 AI 原生世界、游戏、故事与游玩的可扩展创作平台。**

它由三层构成：一个稳定、克制、对内容无意见的内核；一个开放的能力包生态；以及 Home 上可安装、可启动、可停止的项目。平台中每一个有意义的概念——角色、提示词、模型、agent、世界、规则、记忆——都来自能力包，不是内核；项目是宿主运行时概念。

```text
┌──────────────────────────────────────────────┐
│  Web shell · CLI · 第三方客户端                 │   走公开协议
├──────────────────────────────────────────────┤
│  公开协议   ·   /rpc + SSE                    │
├──────────────────────────────────────────────┤
│  项目（Home 卡片：YdlTavern / ...）             │   可安装/启动/停止
├──────────────────────────────────────────────┤
│  能力包（官方包 = 第三方包）                       │   清单驱动
├──────────────────────────────────────────────┤
│  内核：会话 · 事件 · 权限 · ...                 │   对内容无意见
└──────────────────────────────────────────────┘
```

## 为什么做这个

今天大多数 AI 原生创作工具把使用者切成两半：消费成品体验的玩家，和构建体验的开发者。**Yggdrasil 拒绝这种切分。**

玩家可以审视当前会话、让 assistant 修改、fork 它、替换其中某个能力包，再把改动反馈出去。创作者面对同一份公开协议、同样的能力包、同样的 surface。底座在两个方向上完全相同。

完整的产品立场见 [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.md)。

## 重心所在

- 内核只承载能力包，不干别的。
- 所有有意义的概念都由能力包提供。
- 官方包没有任何特权——同一份清单，同一套机制，同一道权限闸门。
- 创作者可以随意组合、替换、或自己写新的能力包。

平台的职责是让激进的 AI 原生创作成为可能，不是给某条「官方路径」开特权。

## 当前状态

平台底座已经搭好：安装/更新链路、surface bundle freshness 防护、项目控制台诊断与更新入口都已落地。Contract V1 是公开平台规范，见 [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.md)。下一阶段不再继续摊大表面积，而是用真实可玩体验来牵引剩下的工作。

详细状态、能力清单、partial 与 deferred 项见 [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md)。下一步方向见 [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.md)。

## 仓库一览

```text
crates/                Rust 内核与运行时
  ygg-core/              内核类型与契约（对内容无意见）
  ygg-runtime/           运行时主机：会话、事件、能力包、能力、钩子、
                         surface、提案、资产、分支、projection
  ygg-service/           公开协议层（HTTP /rpc、事件 SSE 订阅）
  ygg-cli/               host 模式、清单工具、能力包脚手架、conformance

clients/web/           React 19 + Tailwind v4 + Vite 平台 Web shell
clients/desktop/       Tauri 2.x 桌面 wrapper（嵌入 web shell）

packages/official/     通过普通清单加载的官方基础能力包
profiles/              host profile，批量自动加载能力包
examples/              示例清单与 fixture 包

sdk/typescript/        子进程能力包脚手架与领域 SDK
sdk/rust/              生成的 Rust kernel SDK
docs/                  架构、协议、规范、路线图、产品文档
integrations/          上游项目调研记录（pi、TavernHeadless、pretext、TDB...）
```

## 快速上手

启动 host：

```bash
cargo run -p ygg-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

构建或检查 Web shell：

```bash
npm run check --prefix clients/web
npm run build --prefix clients/web
```

跑完整 conformance 套件：

```bash
cargo test --workspace
cargo run -p ygg-cli -- conformance
```

安装和管理能力包：

```bash
yg install github.com/user/yggdrasil-package#v1.2.0
yg list-installed
yg project list
yg project start <project-id>
yg project stop <project-id>
yg uninstall <package-id-or-project-id>
yg update [<package-id>|--project-id <project-id>] [--check-only]
yg lockfile --check
```

只用公开协议跑通空白游创循环：

```bash
cargo run -p ygg-cli -- play-create-demo
```

更多命令（清单、能力包、composition、host 模式、第三方创作循环、模板）见 [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md)。

## 文档导航

每篇开发文档都有英文与简体中文两版，文件顶部的双语 blockquote 可在两种语言间切换。[`docs/`](docs/README.md) 按主题分组：架构、协议、规范、产品、能力包创作、性能、路线图、tavern 兼容。

按目的的最短读路径：

| 你想 | 先读 |
|---|---|
| 理解平台立场 | [`docs/CHARTER.md`](docs/CHARTER.md) → [`docs/architecture/VISION.md`](docs/architecture/VISION.md) → [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.md) |
| 理解架构 | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) → [`docs/architecture/PLATFORM_KERNEL.md`](docs/architecture/PLATFORM_KERNEL.md) → [`docs/architecture/CAPABILITY_PACKAGE.md`](docs/architecture/CAPABILITY_PACKAGE.md) |
| 接入公开协议 | [`docs/protocol/PROTOCOL_V0.md`](docs/protocol/PROTOCOL_V0.md) → [`docs/spec/KERNEL_V1_CONTRACT.md`](docs/spec/KERNEL_V1_CONTRACT.md) |
| 写第一个能力包 | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 安装能力包/项目 | [`docs/guides/PACKAGE_INSTALLATION.md`](docs/guides/PACKAGE_INSTALLATION.md) → [`docs/guides/PROJECT_MODEL.md`](docs/guides/PROJECT_MODEL.md) |
| 管理 API key / secret | [`docs/guides/SECRET_MANAGEMENT.md`](docs/guides/SECRET_MANAGEMENT.md) |
| 跑真实模型端到端调用 | [`docs/guides/REAL_MODEL_END_TO_END.md`](docs/guides/REAL_MODEL_END_TO_END.md) |
| 写 agent / 模型 / 体验包 | [`docs/guides/AGENT_PACKAGE_AUTHORING.md`](docs/guides/AGENT_PACKAGE_AUTHORING.md)、[`docs/guides/MODEL_PROVIDER_INTEGRATION.md`](docs/guides/MODEL_PROVIDER_INTEGRATION.md)、[`docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md`](docs/guides/EXPERIENCE_RUNTIME_AUTHORING.md) |
| 挂载第三方 Web surface | [`docs/guides/SURFACE_HOSTING.md`](docs/guides/SURFACE_HOSTING.md) |
| 看当前状态 | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md) |
| 看下一步 | [`docs/roadmap/NEXT_STEPS.md`](docs/roadmap/NEXT_STEPS.md) |
| 写文档 | [`docs/STYLE.md`](docs/STYLE.md) |

## 延后事项

下面这些方向有价值，但不属于内核——它们都将以普通能力包的形态到来：

- 兼容 SillyTavern 资源与扩展的接入项目 YdlTavern——独立仓库，跑在 Yggdrasil 之上（[`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.md)）。
- 生产级长期自治 agent、多 agent 协作、生产级记忆系统、世界模拟、导演。
- 外部游戏引擎接入（UE5、Godot、Unity、Web 端）。
- 完整 Studio、ComfyUI 风格节点编辑器、市场。
- 最终视觉设计。

## 协议

Yggdrasil 以 GNU Affero General Public License v3.0（仅此版本，`AGPL-3.0-only`）发布，详见 [`LICENSE`](LICENSE)；第一方代码与第三方内容的边界见 [`docs/LICENSING.md`](docs/LICENSING.md)。
