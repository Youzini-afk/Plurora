# 外部项目操作平面指南

> [English](./EXTERNAL_PROJECT_OPERATING_PLANE.en.md) · [中文](./EXTERNAL_PROJECT_OPERATING_PLANE.md)

External Project Operating Plane 说明 Plurora 不必只接入已经适配清单和能力契约的项目。未适配的 git、npm、本地目录或 archive 项目可以先作为 external project 被平台理解、评估风险、规划、展示和包装。只有稳定的 adapter 或 wrapper 才进入普通 Ygg 包和能力体系。

安全边界从一个事实出发：install/run 等于执行不可信代码，workflow 与 secret 泄漏都是真实风险。因此未适配项目先进入计划、策略、proposal 和审计边界；只有用户批准且被类型化 executor 约束的操作才产生副作用。

## 四类对象

| 对象 | 含义 | 是否进入 capability registry |
|---|---|---:|
| Ygg Package | 已适配的能力提供者，有清单、能力、权限、surface 和检查。 | 是 |
| External Project | 未适配项目引用，例如 git/npm/local/archive。默认不可信。 | 否 |
| Managed Workspace | External Project 的受控实例、计划或 fixture，包含 source ref、workspace state、entrypoint、patch proposal 和 audit ref。它不是内核对象。 | 否 |
| Adapter / Wrapper Package | 把外部项目的稳定操作包装成普通 Ygg 包和能力。 | 是 |

这让平台避免退回“所有项目必须先写插件”的旧模式。外部项目可以保持原样。Plurora 围绕它做 intake、workspace plan、风险摘要、项目聚合 UI、patch proposal 和 adapter preview。

## 已实现包

### `official/install-lab` 的 external intake

`plurora install` 现在先检测项目类型，再决定是否解析包清单。没有 `project.yaml` / package manifest 的本地目录和 git source 不再因为“缺少 manifest”而提前失败，而是调用 `official/install-lab/prepare_external_intake` 生成一个零包、可审计的 `external_workspace` 安装计划。

当前支持两种明确所有权：

- `managed`（默认）：本地目录或 git tree 复制/获取到 `<data>/workspaces/external/<project_id>/<content_digest>`。安装计划记录内容 digest，重复安装同一来源和内容是幂等的；卸载只会归档/删除这个 host-owned 根，不会触碰用户源目录。
- `linked_local`（CLI `--link-local`）：workspace 直接指向 canonical 本地源目录，descriptor 明确标记为用户拥有。它是可变引用，不伪造 content digest；卸载永远只移除 Ygg 项目记录，不删除或归档源目录。

managed local copy 会保留 `.gitignore` 等源码元数据，但跳过 VCS 目录、`node_modules`、`target`、虚拟环境和常见语言缓存；materialized tree 默认最多 25,000 个文件、25,000 个目录和 256 MiB，直接 capability 调用也不能把硬上限抬过 100,000 个文件、100,000 个目录或 1 GiB。绝对、悬空或逃逸 workspace root 的 symlink 会被拒绝；托管存储的每一级祖先都必须是 canonical data root 下的真实目录。HTTPS git tree 接受同一套有界 materialization、hash、大小和 symlink 校验；submodule entry 等不支持的 tree mode 会明确失败。临时 bare fetch 现在会在 gix 读取时累计 Git pack 字节，超过默认 512 MiB 即 fail-closed 中断；Host 部署 workspace clone 因其 1 GiB tree 上限而显式使用 2 GiB 硬上限。transport 仍是如实标注的 full bare fetch，而不是伪称 shallow clone，但已不再无界。内联凭据、query 参数与 fragment 都会被拒绝，认证只能由 Host 带外提供，绝不嵌入 descriptor。

项目 ID 由安全 slug 加 96-bit source identity hash 构成，因此同名但不同路径/URL 的来源不会碰撞。descriptor 还记录 `source_kind`、`workspace_ownership` 和可用时的 `source_digest`。相同 ID 若已有不兼容 descriptor 会 fail-closed；并发 materialization 只会复用 digest 完全一致的胜者。

这一步只 materialize 源码和写项目 descriptor，不运行 install/build/test/script，不把 external project 注册成 capability provider。`--wrap-as-adapter` 也不再生成一个并不存在的假 manifest；真实 adapter authoring 留给带 ChangeSet 审批的后续开发流程。

### `official/project-intake-lab`

普通官方包，无内核特权。提供以下能力：

- `describe_intake_contract`
- `inspect_external_project_ref`
- `detect_project_stack_from_metadata`
- `draft_workspace_plan`
- `draft_security_risk_summary`
- `list_candidate_entrypoints`
- `draft_adapter_plan`
- `generate_adapter_manifest_preview`
- `generate_subprocess_wrapper_preview`
- `generate_adapter_fixture_preview`
- `check_adapter_readiness`

能力边界：

- 只做静态 intake、基于元数据的技术栈检测、风险摘要和 workspace/adapter planning。
- 不 clone、不 install、不 run、不联网、不读本地文件系统。
- 阻断 raw secret、path traversal、home path 和敏感绝对本地路径。
- 检测 npm 生命周期脚本：`preinstall`、`install`、`postinstall`、`prepare`、`prepublish` 等会标记为 `executes_code` / `requires_approval`。
- Adapter preview 必须使用普通 third-party package id。不允许 `official/`，也不允许 path traversal 或 unsafe chars。
- 能力 id 必须属于 adapter package namespace。
- 只生成清单、wrapper、fixture 和 readiness preview。不写文件、不执行、不发布。

### `official/workspace-lab`

普通官方包，无内核特权。提供以下能力：

- `describe_workspace_contract`
- `draft_workspace_creation`
- `explain_required_permissions`
- `request_workspace_action`
- `summarize_workspace_audit`
- `create_fixture_workspace`
- `inspect_workspace`
- `read_workspace_metadata`
- `plan_workspace_run`
- `record_fixture_process_result`
- `discover_workspace_entrypoints`
- `draft_workspace_patch`

能力边界：

- Action taxonomy 覆盖 `clone_project`、`read_metadata`、`install_dependencies`、`run_command`、`run_tests`、`stop_process`、`read_logs`、`discover_entrypoints`、`write_patch`、`deploy_plan`。
- 每个 action 标注 `risk_level`、`requires_approval`、`executes_code`、`network_required`、`filesystem_write_required`。
- `request_workspace_action` 默认拒绝。policy/action 不匹配时 fail-closed。
- Fixture workspace 能力证明 workspace descriptor、entrypoint、run plan、fixture result 和 patch proposal 的形状。它不创建目录、不启动进程、不读取文件。
- Patch 只生成提案形状，`file_write_performed=false`。

### Host 开发控制平面

规划包与真实变更执行现在是两条不同的权限路径。`official/workspace-lab` 继续只生成确定性的计划和 patch proposal；获批的源码变更由 access-token 保护的 `/host/v1/projects/:project_id/changes` API 接收，并沿 `Intent -> ChangeSet -> PolicyDecision -> ChangeCommit -> EffectReceipt` 留下 durable 因果链。审批和执行是两个请求，批准对象包含精确 operations、验证方式、所需 authority 和预期效果，批准后不能替换内容。

首版只支持有界 `file_write` / `file_delete`，先复制到 Host-owned scratch，再做静态验证或受限 Dockerfile build。Docker 默认无网络，不支持任意 host command、Nixpacks scratch build、build secret 或 host mount。完整设计见 [`../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md`](../architecture/HOST_DEVELOPMENT_CONTROL_PLANE.md)。

所有权决定结果如何交付：

- `managed_external`：验证通过后创建新的不可变 content-digest tree，并原子更新 descriptor；旧 tree 不会被原地修改。
- `native_managed`：只返回 verified bundle，不自动原地写回。
- `linked_local`：拒绝进入该流程；必须先导入 managed 副本，Host 永不自动修改用户目录。

已提交且使用 `docker_build` 验证的 `managed_external` ChangeSet 还可以进入 verified deployment 事务。验证阶段提交不可变 build-context artifact 并删除验证镜像；preview 重新校验 descriptor、tree 与 artifact provenance，再由用户选择的 `local` 或 Agent target 通过类型化 operation 重建 candidate，且 preview 始终保持 Host 认证。第二次部署审批绑定精确 candidate/evidence；activation 通过健康检查并提交 durable `VerifiedActivate` revision 后才清理上一修订。recover / rollback 从 durable context 在记录的 target 上重建，不读取 live workspace，也不重新抓取源码。

development execute/recover 会在内存操作中携带认证身份，但绝不携带其凭据。它会在 Docker 与 managed-workspace 效应前刷新 Host access journal，并在阻塞验证结束后再次复核。撤销无法中断已经进入 executor 的单个效应，但会阻止所有后续用户效应，并把 durable recovery 状态留给显式获授权的操作者对账。

## Web 聚合入口

`clients/web/src/projects/external-projects.ts` 通过公开协议和能力调用聚合 `project-intake-lab` 与 `workspace-lab` 的计划输出。

- Home/Play 显示 External Project Operating Plane rail。
- Forge 显示 External Projects / Managed Workspaces panel。
- Assistant drawer 显示 inspect / draft patch / generate adapter plan 的轻量入口。
- 项目控制台的 Development 区域和 `plurora host access ... changes` CLI 通过公开 Host API 草拟、审阅、批准、执行、导出和恢复 ChangeSet；控制台还负责 verified private preview、独立部署审批、activation 与中断对账。两个客户端都不直接读写 workspace。
- UI 不读 SQLite、runtime internals、本地项目目录或进程状态。

## 安全红线

- 不新增 `platform.project.*`、`platform.workspace.*`、`platform.git.*`、`platform.npm.*`、`platform.deploy.*`、`platform.ide.*`。
- External Project 不是包；Managed Workspace 不是内核对象；Adapter/Wrapper 才是包。
- 未适配项目不得直接注册为 capability provider。
- 危险动作必须先计划，再通过策略、提案、审批、审计和脱敏边界。
- 默认不执行 `npm install`、`pip install`、`cargo build`、`make` 或任意 project script。
- 不继承宿主 `.env`、SSH key、browser profile、home directory 或 raw secrets。
- Agent 和普通包只能草拟计划、提案和 patch；真实效果必须由已认证 Host 的策略、审批、scratch、验证与审计链完成。
- Web shell 只走公开协议。

## 示例

`examples/packages/external-project-adapter-preview/manifest.yaml` 是 adapter preview fixture。它使用 `thirdparty/example-adapter` namespace，通过普通包清单证明外部项目 adapter 应走同一条包路径。它不是发布物，不自动写入用户项目，也不自动执行外部命令。

可检查：

```bash
cargo run -p plurora-cli -- package check packages/official/project-intake-lab/manifest.yaml
cargo run -p plurora-cli -- package check packages/official/workspace-lab/manifest.yaml
cargo run -p plurora-cli -- package check examples/packages/external-project-adapter-preview/manifest.yaml
cargo run -p plurora-cli -- conformance --tag project_intake
cargo run -p plurora-cli -- conformance --tag workspace_lab
```

## 真实项目持续验收

GitHub CI 的 [`External project Host operations acceptance`](../../.github/workflows/ci.yml) 是黑盒发布门槛，而不是产品专属 demo。它通过 CLI `install --workspace-only` 接入固定到 commit `6c7a360ddb4a0d75be06044bf8a914f260ff10c7` 的 [`mdn/beginner-html-site-styled`](https://github.com/mdn/beginner-html-site-styled/tree/6c7a360ddb4a0d75be06044bf8a914f260ff10c7)，启动普通 SQLite/autoload Host 后只使用带认证的公开 RPC/HTTP contract。

[`scripts/host-operations-acceptance.py`](../../scripts/host-operations-acceptance.py) 对真实项目创建两个独立 verified revision，并对结构不同的 [`Python 标准库 HTTP fixture`](../../examples/host-operations/python-service/README.md) 创建第三个 revision。门槛覆盖 network-none Docker 验证、private preview、生产 route、容器删除与 readiness 降级、显式 recover、Host crash、durable lease 接管、SQLite/runtime 投影恢复和 rollback。直接 Docker 调用只用于精确故障注入、观察和清理，不用于完成平台操作。

## 后续方向

external intake、受控源码 ChangeSet、verified local/Agent deployment 与真实项目故障恢复已形成第一条 Host 闭环。下一步不是加入任意命令执行，而是在相同边界上继续收紧和扩展：

- artifact 的细粒度读取权限、加密/保留策略、reachability GC 与 journal snapshot compaction。
- 更多显式 verifier 与沙箱后端；每一种都必须声明网络、secret、资源和效果，不能退化成通用 shell runner。
- native verified bundle 的人工/工具化应用，以及更深入的 project graph、dependency risk analysis 和受控 adapter / deployment descriptor 引导式创作。
- target-edge ingress 与应用身份单独设计；任意网络代理和通用远程 shell 仍明确不做。

这些仍应作为普通包和 Host executor 底座推进，不应进入内核 product ontology。
