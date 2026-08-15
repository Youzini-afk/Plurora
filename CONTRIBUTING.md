# 参与 Plurora

> [English](./CONTRIBUTING.en.md) · [中文](./CONTRIBUTING.md)

感谢你愿意一起建设这个平台。这份文档说明如何在本仓库做一次可审查的贡献。平台原则见 [`docs/CHARTER.md`](docs/CHARTER.md)，写作规范见 [`docs/STYLE.md`](docs/STYLE.md)。

## 先跑起来

按 [`docs/guides/GETTING_STARTED.md`](docs/guides/GETTING_STARTED.md) 启动 Host 和 Web Shell。构建 Desktop 或 Release 见 [`BUILDING.md`](BUILDING.md)。

## 改动落在哪一层

先确认你改的是哪一类事实，再动手：

| 你在改 | 先读 | 不要顺便改 |
|---|---|---|
| 平台原则 | [`docs/CHARTER.md`](docs/CHARTER.md) | 官方 UI 或某个 Profile |
| 分层与所有权 | [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) | 一次性路线图 |
| 公开方法 / 事件 | [`docs/spec/PUBLIC_CONTRACT.md`](docs/spec/PUBLIC_CONTRACT.md) | 生成物本身 |
| 官方产品体验 | [`docs/product/PLATFORM_PRODUCT_MODEL.md`](docs/product/PLATFORM_PRODUCT_MODEL.md) | 宪法基底 |
| 当前实现状态 | [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md) | 把状态写成永久承诺 |

一次 PR 尽量只动一个产品或子系统边界。

## 几条红线

- 第一方 Package、Shell 和客户端没有私有 API、隐藏权限或旁路。
- 不要手改 `docs/spec/v1/schemas/`、`sdk/openapi.yaml`、`sdk/rust/plurora-contract-sdk/` 或 `sdk/typescript/contract-sdk/`。改 registry / 类型源头，再运行 `scripts/regen-sdks.sh`。
- linked-local 源码永不删除；保持路径包含和 symlink 防护。
- 不要把 raw secret、token 或用户绝对路径写进文档或测试。
- 不要改 YdlTavern、市场 / 支付 / DRM 或无关产品。
- 中英文档在同一提交内保持事实同步。

## 合同怎么改

1. 改 `crates/plurora-runtime/src/protocol.rs` 的 `PlatformMethod`，或 `crates/plurora-core/src/event.rs` 的事件常量，以及对应领域类型。
2. 如需调整导出映射，改 `crates/plurora-cli/src/schema_export/`。
3. 运行 `scripts/regen-sdks.sh`。
4. 把生成的 schema 与 SDK 和源头改动一起提交。
5. 行为回归优先加 `crates/plurora-cli/src/conformance/` 里的具名 case，而不是只加散落单测。

## 文档怎么写

- 中文默认 `xxx.md`，英文 `xxx.en.md`，顶部使用标准语言切换。
- 写给读者：它是什么、怎么用、边界是什么。不要写开发日志或阶段编号。
- 精确统计数字只写在 [`docs/ALPHA_STATUS.md`](docs/ALPHA_STATUS.md) 和被检查脚本锚定的 spec 文件里。
- 提交前运行 `python scripts/check-docs.py` 和 `python scripts/check-identity.py`。

## 建议的本地检查

按改动范围选择，不必每次全跑：

```bash
cargo fmt --check
python scripts/check-docs.py
python scripts/check-identity.py
cargo test -p <affected-crate>
npm run check --prefix clients/web
cargo run -p plurora-cli -- conformance --case <pattern>
git diff --check
```

完整 workspace 测试、全量 conformance、Docker、Desktop sidecar 和 Host 运维验收通常属于 CI。

## Pull request

1. 从最新 `main` 开分支。
2. 保持提交可审查；不要改写已发布历史。
3. PR 说明写清改了什么、为什么、跑了哪些检查、还有什么环境限制。
4. 文档与代码的事实变化放在同一 PR。
5. 安全问题不要开公开 Issue，见 [`SECURITY.md`](SECURITY.md)。

寻找起步任务时，优先看标了 `good first issue` 的 Issue。不确定归哪一层时，先开讨论而不是先改基底。
