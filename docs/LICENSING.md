# 许可证边界

> [English](./LICENSING.en.md) · [中文](./LICENSING.md)

Plurora 仓库中的第一方源代码以 GNU Affero General Public License v3.0 的仅此版本授权，SPDX 标识为 `AGPL-3.0-only`。完整法律文本见仓库根目录的 [`LICENSE`](../LICENSE)。

## 第一方代码

下面这些部分统一使用 `AGPL-3.0-only`：

- Rust workspace、CLI、Host、service 与运行时；
- Web shell 与 Desktop wrapper；
- 仓库内维护的 Rust / TypeScript SDK；
- `packages/plurora/` 下的第一方 capability Package；
- 仓库内维护的构建、验证和发布脚本。

除非另有书面约定，向这些第一方部分提交的贡献按同一许可证进入仓库。

## 不被重新授权的内容

仓库也包含用于兼容、测试或集成的内容，它们可能保留自己的许可证：

- `examples/` 中明确声明其他许可证的示例或 fixture；
- `integrations/` 中记录的上游项目及其许可证元数据；
- lockfile 中列出的第三方依赖。

这些记录不改变 Plurora 第一方代码的许可证，也不把第三方内容重新授权为 AGPL。能力包清单中的 `license` 字段描述该能力包自身；第三方能力包应声明自己的真实许可证。

## 分发与网络使用

AGPL 对修改、分发以及通过网络向用户提供修改版本时的源代码提供义务有具体规定。实际使用和分发应以 [`LICENSE`](../LICENSE) 的完整文本为准；本文只说明仓库的许可证边界，不替代法律意见。

## 一致性检查

CI 运行：

```bash
bash scripts/check-license-metadata.sh
```

该检查确保第一方 Cargo/npm 元数据、lockfile 根包记录和第一方 capability Package 清单都保持为 `AGPL-3.0-only`，防止再次出现许可证漂移。
