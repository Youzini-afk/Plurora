# Conformance Kit

> [English](./CONFORMANCE_KIT.en.md) · [中文](./CONFORMANCE_KIT.md)

Conformance kit 用来让第三方能力包在交给 host、市场、CI 或用户之前，自行验证是否遵守内核 v1 契约。它检查的是平台边界，不检查包的内容语义。

## 目的

Plurora 要求官方包和第三方包走同一份契约。Conformance kit 提供可重复的本地验证：manifest 是否能解析、entry 是否可运行、bindings 是否正确、能力声明是否一致、权限是否最小、审计是否可见、fixture 调用是否稳定。

通过 kit 不代表包“质量好”或“内容正确”；它只说明该包按 v1 合同参与平台。

## 基本用法

```bash
plurora conformance package --contract v1 --path <package>
```

常用选项：

```bash
plurora conformance package --contract v1 --path <package> --format json
plurora conformance package --contract v1 --path <package> --static-only
```

- `--format json`：输出机器可读报告，适合 CI。
- `--static-only`：只做 manifest/schema/声明检查，不启动包。
- `--contract v1`：验证路径 A 包；路径 B 包也用该参数，但会按 `entry.contract: "none"` 跳过不适用项。

## 报告状态

| 状态 | 含义 |
|---|---|
| PASS | 检查通过。 |
| FAIL | 检查失败，合规百分比下降，CI 通常应失败。 |
| SKIP | 检查不适用于该包，例如路径 B 的 capability binding。 |
| WARNING | 不违反契约，但有风险或可改进，例如声明过宽。 |

## 8 个验收检查

### 1. Manifest parse

解析 `manifest.yaml` 或等价描述，校验 id、version、entry、capabilities、permissions、surface contributions、schemas、hooks 和 extension points 的基本形状。失败通常表示包根路径不对、YAML 无效、必填字段缺失或 schema 不匹配。

### 2. Contract mode

检查 `entry.contract`。`"v1"` 表示路径 A：包接受能力句柄、权限强制与审计。`"none"` 表示路径 B：包自包含运行，不接收 v1 bindings。缺省值按路径 A 处理，便于新包安全默认。

### 3. Entry support

确认 entry kind 被当前 host 支持。`subprocess` 与 `rust_inproc` 是当前主要执行路径；`wasm` 与 `remote` 是一等 manifest 形式但执行留待后续。若 host policy 不允许某 entry，检查失败。

### 4. Bindings / handshake

路径 A subprocess 包必须完成 JSON-RPC stdio handshake，并声明自己接收的 bindings。Rust in-process 包必须能通过 `ComponentEnv` 初始化。路径 B 包跳过 v1 binding 检查，但仍需要证明可启动且不会要求 kernel capability。

### 5. Capability declarations

检查 `provides` 与 `consumes`。Provider 必须有稳定 id、version、input/output schema、streaming 标志和 side-effect 描述。Consumer 声明必须能映射到句柄上限。歧义 provider、缺失 schema 或非法 namespace 会失败。

### 6. Permission declarations

检查 `events.append/read`、`capabilities.invoke`、`permissions.network`、`permissions.secret_refs` 等声明是否与包行为一致。未声明即使用会失败；声明过宽可能给 WARNING；路径 B 包不允许通过这些声明获取 v1 权威。

### 7. Audit visibility

验证包生命周期、能力调用、出站请求、permission denial 或 Path B contract mode 能被 host 审计看见。路径 A 包应产生 declared vs used 报告；路径 B 包的事件应包含 `contract_mode: "none"`，方便 operator 区分。

### 8. Fixture invocation

对非流式 capability 使用 deterministic fixture 输入调用，验证 schema、权限、输出形状和无 raw secret 泄漏。流式 capability 使用轻量 lifecycle smoke。`--static-only` 会跳过本项。

## 合规百分比

报告会计算适用检查中的 PASS 比例。SKIP 不计入分母。WARNING 不算失败，但应在发布前审查。

示例：路径 A 包 8 项全 PASS → 100%。路径 B 包跳过 bindings/capability/permission fixture 中不适用部分，只要自包含路径通过，也可以得到 100%。

## CI 集成

推荐在包仓库中加入：

```bash
plurora conformance package --contract v1 --path . --format json > conformance.json
```

PR gate 应在以下情况失败：

- 出现 FAIL；
- 合规百分比低于项目阈值；
- 新增 WARNING 未被显式接受；
- JSON 报告显示权限声明变宽但没有审查记录。

对 monorepo，可对每个 package manifest 循环运行。对路径 B 包，仍建议运行 kit，确保 host 可观察它的生命周期。

## 解释常见失败

- `manifest_not_found`：`--path` 指错，或包没有 manifest。
- `entry_not_supported`：host policy 不接受该 entry。
- `handshake_failed`：subprocess 未输出正确 JSON-RPC handshake，或 stdout 被日志污染。
- `binding_missing`：包声明需要能力，但未收到 handle。
- `schema_invalid`：输入/输出 schema 不符合 v1 schema subset。
- `permission_denied`：fixture 调用尝试了未声明权限。
- `raw_secret_detected`：payload、metadata 或输出包含明显 raw secret。

## 自定义检查

后续会把 kit 提取为可嵌入 library。当前建议把自定义检查作为 CI 中的独立步骤运行，并把 conformance JSON 作为输入。例如：要求所有网络声明必须带 `purpose`，或要求 package id 匹配组织前缀。

自定义检查不应替代官方 kit。它们只能收紧项目规则，不能放宽 v1 契约。

## 与 SDK 的关系

生成 SDK 使用同一份 `docs/spec/v1/schemas/`。如果 SDK 生成成功但 conformance 失败，说明包实现或 manifest 与契约不一致；如果 conformance 通过但 SDK 类型缺失，说明 SDK 生成产物需要刷新。

## JSON 报告形状

`--format json` 输出稳定字段，便于 CI 和 dashboards 消费：

```json
{
  "package_id": "example/echo",
  "contract": "v1",
  "compliance_percent": 100,
  "checks": [
    { "id": "manifest.parse", "status": "PASS" },
    { "id": "bindings.handshake", "status": "PASS" }
  ]
}
```

字段可以 additive 扩展。CI 应忽略未知字段，并只依赖 `status`、`id`、`compliance_percent` 等稳定字段。

## 静态检查 vs 动态检查

静态检查不启动包，适合快速 PR lint：manifest parse、contract mode、entry support、capability declaration、permission declaration。动态检查会启动包并验证 handshake、bindings、fixture invocation、audit visibility。

建议 PR 上先跑静态检查，main branch 或 release gate 跑完整检查。

## 发布建议

发布包前建议保存 conformance JSON 作为 artifact。Artifact 应记录 kit 版本、schema hash、package manifest hash 与运行时间。这样后续出现兼容问题时，可以判断是包变了、schema 变了，还是 host 行为变了。

## Monorepo 示例

```bash
for manifest in packages/*/manifest.yaml; do
  dir=$(dirname "$manifest")
  plurora conformance package --contract v1 --path "$dir" --format json
done
```

失败时不要合并多个包的报告；逐包保留，便于找到具体权限或 schema 问题。

## 与 effect audit 的关系

Conformance kit 证明包在测试场景中遵守契约。Effect audit 证明运行中的包实际使用了哪些权威。两者互补：CI 依赖 kit，operator 依赖 audit，release review 应同时看两者。

## 版本兼容

Kit 版本跟随 v1 schema additive 演进。新增检查默认应以 WARNING 或 SKIP 进入，避免无预警破坏现有包；当检查成为 release gate 时，应在 roadmap 或 changelog 中明确说明。

## 路径 A 与路径 B

| 项目 | 路径 A (`v1`) | 路径 B (`none`) |
|---|---|---|
| Manifest 权限声明 | 生效并强制 | 不用于获得 v1 权威 |
| Bindings | 必须注入 | 跳过 |
| Capability invoke | 通过句柄 | 不适用 |
| Audit | declared vs used | lifecycle + `contract_mode: none` |
| Kit 目标 | 权限正确且可调用 | 自包含且可观察 |

## 参考

- [`../spec/KERNEL_V1_CONTRACT.md`](../spec/KERNEL_V1_CONTRACT.md)
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.md)
- [`PATH_B_SELF_CONTAINED.md`](PATH_B_SELF_CONTAINED.md)
