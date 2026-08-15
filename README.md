# Plurora

> [English](./README.en.md) · [中文](./README.md)

[![CI](https://github.com/Youzini-afk/Plurora/actions/workflows/ci.yml/badge.svg)](https://github.com/Youzini-afk/Plurora/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/License-AGPL--3.0--only-blue.svg)](./LICENSE)

**一个面向人、AI 与软件共同创造和运行的开放数字平台。**

如果你熟悉 Docker 是容器的运行时，可以把 Plurora 理解成**数字作品的运行时**：应用、工具、世界、游戏、agent、创作环境，以及尚未被命名的形态，都可以被创建、组合、运行、审视、修改、迁移和替换。平台提供公开合同和可信基底；官方 Web / Desktop 只是可替换的壳，不是整个平台。

仓库处于 Foundation Alpha：Host、公开合同、Web/PWA、Desktop、CLI 和可运行的创作示例已经形成较大可运行面。实现细节见 [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md)。

![Plurora](clients/web/public/icons/plurora.svg)

## 五分钟看到界面

需要 Rust 1.78+、Node.js 20+ 和 Git。从源码编译 Host 在 Windows 上还需要 Visual Studio Build Tools（C++ 桌面工作负载）。WebView2 只在构建 Desktop 时需要。

```bash
# 终端 1：启动 Host
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml

# 终端 2：启动 Web Shell
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

打开 [http://127.0.0.1:1420](http://127.0.0.1:1420)。完整步骤、依赖和故障排查见 [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.md)；构建 Desktop / Release 见 [`BUILDING.md`](BUILDING.md)。

## 几个词

| 词 | 含义 |
|---|---|
| **Work** | 一份可移植的数字作品：内容寻址，还不是某台机器上的运行实例。 |
| **Installation** | 某台 Host 采用一份 Work 的记录。装上不等于已经在跑。 |
| **Run** | 一次显式启动的运行。打开详情页不会自动开始或停止。 |
| **Powerbox** | 跨 Installation 连接能力时的显式选择界面，类似权限同意框。 |
| **Realization** | 把 Work 编译成计划，审批后再应用到 Docker / Agent 等 Target。计划本身没有副作用。 |
| **宪法基底** | 极小的通用机制层：身份、授权、对象、日志、调用。它不拥有聊天、游戏或某个官方界面。 |
| **Host** | 管安装、进程、文件、secret、网络、备份和诊断的控制面。 |
| **Shell** | Web、Desktop、CLI 等客户端。都可以被第三方替换。 |

因果链是 `Work → Installation → Run`，以及可选的 Powerbox 连接和 Realization 部署。这几步互不隐式触发。

## 我们想建设什么

- **开放：** 源码、协议、数据和扩展入口公开；用户能导出、迁移和删除数据；第一方没有私有 API。
- **多样：** 不固定唯一应用形态、工作流、Shell、模型或内容本体。
- **先进：** 采用真正增加自由度、安全性、性能和可移植性的技术。
- **长久：** 稳定层小、协议可演化、旧数据可读取、错误抽象能退出。
- **好用：** 官方发行版安装后能直接工作，简单路径简单，失败可恢复。

完整原则见 [`docs/CHARTER.md`](docs/CHARTER.md)。

## 平台与官方产品

Plurora 不只是内核，也不等于官方 Web/Desktop。当前官方发行版提供 Library、Settings、Installation frame、本地与远程 Host、Run、Powerbox、Realization、权限和数据管理。这些是可替换的默认产品选择。

- 第三方可以替换客户端、Shell、组件、协议、模型和 Host。
- Work、Installation 与 Run 不属于宪法基底。
- Home / Play / Forge / Assist 属于可选产品 Profile。
- 第一方组件和客户端只使用第三方也能使用的公开边界。
- Foreign Work 可以按 Rights / Transparency 披露后，在本机以不透明方式启动；它仍然走公开 Host 合同。

产品责任见 [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md)。游创是可选强观点 Profile，见 [`docs/product/PLAY_CREATION_MODEL.md`](docs/product/PLAY_CREATION_MODEL.md)。

## 仓库一览

```text
crates/                 Rust Host、runtime、service、CLI
clients/web/            官方 React 19 + Tailwind v4 + Vite Web Shell / PWA
clients/desktop/        Tauri 2.x wrapper + managed Host sidecar
packages/plurora/       通过普通 Manifest 加载的第一方组件
profiles/               Host 的组件与策略组合
examples/               示例、fixture 与第三方接入样例
sdk/                    TypeScript 与生成的 Rust contract SDK
docs/                   章程、架构、协议、产品、指南与状态
```

目录反映当前实现，不单凭 crate 名称决定永久架构归属。

## 常用命令

CLI 在未安装到 PATH 时，一律通过 Cargo 调用：

```bash
cargo run -p plurora-cli -- work check examples/works/modular-simulation --json
cargo run -p plurora-cli -- installation list
cargo run -p plurora-cli -- realization plan --help
cargo run -p plurora-cli -- play-create-demo
cargo run -p plurora-cli -- conformance
```

需要把 `plurora` 装进 PATH 时：

```bash
cargo install --path crates/plurora-cli
```

多数 `installation` / `realization` 命令需要先启动 Host。测试：

```bash
cargo test --workspace
npm run check --prefix clients/web
```

## 接下来读什么

完整索引见 [`docs/README.md`](docs/README.md)。每篇主要文档都有中英版本。

| 你想 | 先读 |
|---|---|
| 五分钟跑起来 | [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.md) |
| 写第一个 Package | [`docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md`](docs/guides/PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 理解平台目标 | [`docs/CHARTER.md`](docs/CHARTER.md) → [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) |
| 理解官方产品 | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md) |
| 接入公开协议 | [`docs/protocol/PUBLIC_PROTOCOL.md`](docs/protocol/PUBLIC_PROTOCOL.md) → [`docs/spec/PUBLIC_CONTRACT.md`](docs/spec/PUBLIC_CONTRACT.md) |
| 构建 Web / Desktop / Release | [`BUILDING.md`](BUILDING.md) |
| 参与贡献 | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| 看当前实现事实 | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md) |

聊天、世界模拟、Tavern、游戏引擎桥接、IDE、普通 Web 服务、agent runtime、文档工具和节点编辑器，都可以成为平台上的产品或组件。它们没有一项是平台唯一中心。YdlTavern 是独立接入项目，边界见 [`docs/tavern/TAVERN_COMPAT.md`](docs/tavern/TAVERN_COMPAT.md)。

## 许可证

Plurora 以 GNU Affero General Public License v3.0（仅此版本，`AGPL-3.0-only`）发布。白话要点：

- 本地自用、阅读源码、给自己改一版，都可以。
- 如果你把修改过的 Host 或第一方代码通过网络提供给别人使用，需要公开对应源码。
- 第三方 Package 应声明自己的许可证，不必仅仅因为跑在 Plurora 上就变成 AGPL。
- 向本仓库第一方代码提交的贡献默认按同一许可证进入。

完整文本见 [`LICENSE`](LICENSE)，边界见 [`docs/LICENSING.md`](docs/LICENSING.md)。这不是法律意见。
