# Foreign Work、Rights 与不透明状态

> [English](./FOREIGN_WORK_RIGHTS.en.md) · [中文](./FOREIGN_WORK_RIGHTS.md)

本指南说明 Plurora 如何把外部 URI、本地可执行文件、受管理二进制、OCI image、远程服务与 entitlement adapter 纳入 Work / Installation / Run 模型，而不制造“闭源项目”特例或 DRM 特权路径。

## 身份与位置分离

`ForeignCapsuleDescriptor` 是可移植、内容寻址的 Work 内容。它只声明：

- 稳定的 capsule identity；
- 抽象 launch requirement；
- 可选的普通 `PortDescriptor`；
- 可选的 `StateSlotDescriptor`；
- Rights 与 Transparency artifact 引用。

它不能携带用户机器上的 executable 路径、working directory、URI、endpoint 或 image coordinate。验证器会递归检查 annotations，并以 `RawPath` 拒绝这些字段。实际坐标属于某个 Installation，通过 `plurora.foreign-launch-binding.v1` 保存到 Installation-scoped secret store；Installation 公开视图、事件、receipt 与诊断不会回显该值。

支持的 Host-local target 是：

- `external_uri`；
- `local_executable`；
- `managed_artifact`；
- `oci_image`；
- `remote_service`；
- `entitlement_adapter`。

portable requirement 固定 target kind 与 launch ID；Installation binding 必须精确匹配，不能借本地配置改变 Work 的入口语义。

## Rights 是声明，不是 authority

`RightsDeclaration` 对十类操作分别给出 `allowed`、`denied`、`requires_entitlement` 或 `unspecified`：install、execute、backup、export state、copy across Hosts、redistribute artifacts、modify、derive、modding、dedicated server。

Host 的当前策略按具体 effect 分开：

- Installation create/update 会拒绝明确的 `install: denied`；其余 install disposition 作为声明保留，execute 在 Run 边界再次判断；
- execute 的 `denied` 或 `unspecified` 会阻止 Run，`requires_entitlement` 只有通过普通 entitlement capability adapter 才能继续；
- backup、state export、跨 Host 复制与 dedicated server 都要求对应 Right 明确 `allowed`；
- 为保留既有无声明 Work 的安装/运行能力，完全缺少 Rights artifact 时，install 与 execute 视为允许；其他操作仍是 `unspecified`，不会自动执行。

Rights 不会铸造 capability、扩大 Host grant，也不是法律裁决。Library 将发布者/来源声明、引用的 evidence 与 Host 实际强制边界分栏展示。`TransparencyDeclaration` 同样把 source visibility、reproducible-build claim、SBOM/provenance/signature refs、telemetry disclosure 与 state portability 作为可审计事实，而不是信任捷径。

## Run 与 entitlement

Foreign launch 是普通 Work entrypoint。Run preflight 会读取 exact Work revision、launch requirement、Installation revision 与当前 Rights；缺 binding、target kind 不符、Rights 拒绝或 entitlement 失败都会返回结构化 gap/错误，不创建隐式部署。

entitlement 只是普通 `capability.invoke`：binding 指定 provider Package、capability 与可选版本，Host 只接受 adapter 返回的允许决定和可选 launch target。平台没有 `drm.*`、ownership database 或隐藏的第一方 API。adapter input、credential、本机坐标、child stderr 与命令细节不会进入 Debug、公开 Installation payload 或错误文本。

`plurora.foreign/dedicated-server` entrypoint 还要求 `dedicated_server: allowed`。Realization 计划若需要跨 Host 复制，则重新检查 `copy_across_hosts: allowed`；计划不会因为二进制已经存在而绕过 Rights。

## 协议 Port 与 Binding 边界

闭源与可组合性是两件事。ForeignCapsule 可以声明公开 save、health、mod、lobby 等普通 Port，兼容性仍由同一套 protocol/interface/version/profile/interaction/effect/transport 规则计算，不按 publisher 或 source visibility 提升优先级。

声明本身不会让 Host 猜测如何把一个任意进程接到 runtime handle。可执行 Binding 必须由普通 Component/Host adapter 提供明确 transport 与 provider endpoint；它走公开 Exposure/Powerbox/Binding 流程，没有 foreign-only bridge，也不会自动发现或自动选择 provider。开源但没有协议的程序同样可以只是 Capsule。

## Opaque state：backup 与 export 分开

`InstallationStateAction::Backup` 在 exact Installation revision 下快照当前 state tree，生成内容寻址的 `urn:plurora:installation-state-snapshot:v1` receipt。它不替换、不迁移、不清空当前 state；restore 仍是显式 `replace` update。

两项权利分别检查：

- 创建 snapshot 需要 `backup: allowed`；
- 通过 `object.get` 下载 snapshot 还需要当前 Work 的 `export_state: allowed`。

公开读取必须同时携带 exact Installation ID 与 Host journal 实际签发的 descriptor。仅知道 digest、伪造 descriptor 或拿其他 Installation 的 receipt 都不足以导出。snapshot 内路径必须是排序、唯一、相对且不含 traversal/platform prefix；state effect 保留现有 containment 与 symlink 防护。

## CLI

先查看当前 revision，再用稳定 idempotency key：

```bash
plurora installation backup <installation-id> \
  --expected-revision <revision> \
  --idempotency-key <stable-key>

plurora installation bind-foreign <installation-id> <launch-id> \
  --binding binding.json \
  --expected-revision <revision> \
  --idempotency-key <stable-key>
```

`bind-foreign` 先把精确 secret reference 加入 Installation policy，再通过普通 `plurora/secret-store-lab/put_installation_secret` capability 写入本地 binding；CLI 不打印 binding 内容。若调用结果不确定，应先重新读取 Installation revision 与 Run 状态，再决定是否用同一 key 重试。

## Web / PWA

Home 与 Installation Frame 只从公开 `host.installation.*` summary 读取 Rights/Transparency。Foreign Work 卡片显示来源、trust、声明、evidence 与 Host enforcement；本机 launch binding 通过受控输入写入，绝不从公开 summary 读回。Backup 与 Download 是两个独立 affordance；关闭页面不会停止 Run，也不会隐式备份、导出或启动。

## 可核验边界

Foreign Work conformance 覆盖：无协议 Capsule、portable coordinate 拒绝、闭源 Port 使用普通兼容性合同、保守 Rights、entitlement 脱敏且无 DRM method、dedicated entrypoint 与公开 backup schema。更深的 state/launch lifecycle、Realization Rights gate、Installation journal 与 Web 行为由对应 crate/service/TypeScript tests 覆盖。
