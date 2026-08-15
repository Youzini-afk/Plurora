# 开始使用 Plurora

> [English](./GETTING_STARTED.en.md) · [中文](./GETTING_STARTED.md)

这份指南属于官方发行版的试用路径：从源码启动本地 Host 和 Web Shell，检查一份示例 Work，再决定下一步读什么。它不把官方界面写成平台宪法。

## 你需要什么

- Rust 1.78 或更新（`rustc --version`）
- Node.js 20 或更新（`node --version`）
- Git
- 从源码编译 Host 所需的 C/C++ 工具链：Windows 上是 Visual Studio Build Tools 2022（C++ 桌面开发工作负载）

WebView2 和 Linux WebKit 开发包只在构建 Desktop 时需要，见 [`../../BUILDING.md`](../../BUILDING.md)。本机若缺少 `cl.exe`，`cargo run -p plurora-cli` 会在编译 `ring` / SQLite 时失败。

下面的命令都在仓库根目录执行。在把 `plurora` 装进 PATH 之前，一律写成 `cargo run -p plurora-cli -- <subcommand>`。

## 1. 启动 Host

```bash
cargo run -p plurora-cli -- host serve \
  --http 127.0.0.1:8787 \
  --profile profiles/forge-alpha.yaml
```

第一次编译会花几分钟。成功后终端会保持运行，并监听 `127.0.0.1:8787`。这个 profile 会按普通 Manifest 加载第一方 Package，不会给它们特权。

默认数据目录是 `~/.plurora/`（可用 `PLURORA_DATA_DIR` 覆盖）。不要把里面的路径写进面向他人的文档或 Issue。

## 2. 启动 Web Shell

另开一个终端：

```bash
npm ci --prefix clients/web
npm run dev --prefix clients/web
```

打开 [http://127.0.0.1:1420](http://127.0.0.1:1420)。你应看到官方 Library（Home）。打开某个 Installation 详情不会自动启动 Run。

如果页面连不上 Host：确认第一个终端仍在运行，且浏览器访问的是本机 `127.0.0.1`，不是另一个端口上的旧进程。

## 3. 检查示例 Work

Host 不必为这一步运行。`work check` 只读取本地 source，不安装、不启动：

```bash
cargo run -p plurora-cli -- work check examples/works/modular-simulation --json
```

成功时会看到 Work / Assembly 诊断，而不是错误。这份 kit 是一个 5×5 殖民地模拟：可移植存档、静态 renderer、可选 AI Port。它证明第三方可以用同一条公开路径替换组件。更深的说明见 [`MODULAR_SIMULATION.md`](MODULAR_SIMULATION.md)。

空白游创回路（同样走公开合同）：

```bash
cargo run -p plurora-cli -- play-create-demo
```

## 4. 可选：把 CLI 装到 PATH

```bash
cargo install --path crates/plurora-cli
plurora work check examples/works/modular-simulation --json
```

`installation` 和 `realization` 子命令需要已运行的 Host。创建 Installation 不会自动开始 Run，计划 Realization 也不会自动 apply。

## 接下来

| 你想 | 去 |
|---|---|
| 写第一个 Package | [`PACKAGE_AUTHORING_WALKTHROUGH.md`](PACKAGE_AUTHORING_WALKTHROUGH.md) |
| 理解 Installation / Run | [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md) → [`RUN_LIBRARY.md`](RUN_LIBRARY.md) |
| 管理 API key | [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md) |
| 构建 Desktop 或 Release | [`../../BUILDING.md`](../../BUILDING.md) |
| 理解平台为什么这样分层 | [`../CHARTER.md`](../CHARTER.md) → [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md) |
| 参与贡献 | [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) |

## 常见问题

- **`npm ci` 失败：** 确认 Node 20+，删除 `clients/web/node_modules` 后重试。
- **Host 编译失败：** 确认 Rust 1.78+。Windows 上若提示找不到 `cl.exe`，安装 Visual Studio Build Tools 的 C++ 桌面工作负载。
- **命令写成了 `plurora ...` 却找不到：** 改回 `cargo run -p plurora-cli -- ...`，或先 `cargo install --path crates/plurora-cli`。
- **想用真实模型：** 默认 profile 不出网。复制 `profiles/forge-with-live-models.example.yaml` 并收紧 `allowed_hosts`，见 [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md)。
