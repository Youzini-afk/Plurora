# 真实模型调用端到端

> [English](./REAL_MODEL_END_TO_END.en.md) · [中文](./REAL_MODEL_END_TO_END.md)

真实模型调用仍走普通 Package capability、Host secret resolver 与 outbound executor。Installation 只提供 Host-local policy 和 secret scope；它不会让 Work、Package 或第一方代码绕过 authority、审计或脱敏。

## 调用链

```text
Work / Assembly
  └─ Component import Port → model provider capability
        ↓ resolver + AssemblyLock
Installation
  ├─ active WorkRevision / AssemblyLock
  ├─ InstallationSecretPolicy
  └─ secret_ref:installation:*
        ↓ Host-verified Installation context
capability.invoke / capability.stream
        ↓ manifest + handle + schema + effect checks
provider adapter
        ↓ host.outbound.execute / stream / websocket
Host executor resolves secret at the last moment
        ↓ HTTPS / WSS
terminal EffectReceipt + redacted audit
```

Web Installation detail 不会自动创建 Run。真实调用必须在显式 `host.run.start` 成功后的 Run context 中进行；Run 只激活已安装、已验证且唯一匹配的本地实现。缺失、歧义、unsupported backend 或需要机器资源时，start 返回结构化 gap，不隐式 build/deploy。关闭 tab 不会停止 Run；显式 `host.run.stop` 只停止该 Run。

## 配置 secret

平台共享值：

```text
secret_ref:store:OPENAI_API_KEY
```

Installation-local 值：

```text
secret_ref:installation:OPENAI_API_KEY
```

Installation record 必须允许完整引用：

```yaml
secret_policy:
  allow_platform_fallback: false
  allowed_secret_refs:
    - secret_ref:installation:OPENAI_API_KEY
```

`InstallationStoreSecretResolver` 先检查 exact allowlist，再读取 `~/.plurora/installations/<installation-id>/secrets.dat`。本地值缺失时，只在 `allow_platform_fallback` 为 true 时解析对应的 `secret_ref:store:*`。

## Provider Package

Provider 是普通 Package：

- manifest 声明 capability、network host、method、purpose 与所需 `secret_ref`；
- Component export Port 声明 protocol/interface/version/profile、interaction、transport 与 effect class；
- 第一方 publisher 不获得 routing priority；多个兼容 provider 会报歧义；
- raw key 不进入 Package input/output schema。

Adapter 构造请求 shape，但不能直接联网。实际网络 effect 只能通过 `host.outbound.*`，并在执行前重新检查当前 authority 与 policy。

## 执行与审计

1. `capability.invoke` 或 stream 调用验证 caller handle、schema 与 provider binding。
2. Host 从已验证 Installation context 建立 secret scope。
3. outbound executor 解析 secret，并在最后一刻注入 header。
4. audit 只记录 Package / capability / destination / method / purpose / secret ref / redaction state。
5. success、denied、error、cancelled 或 timeout 都产生可区分 terminal evidence。

事件、日志、stream frame、proposal 与 receipt 不包含 raw request body、provider response、prompt 或 secret value。

## 默认网络边界

- HTTP 只允许 HTTPS，WebSocket 只允许 WSS；
- redirect fail closed；
- manifest 未声明 destination / method / purpose 时拒绝；
- live executor 默认关闭，普通 conformance 使用 fake executor；
- secret resolution 本身不触发网络。

## 常见失败

| 诊断 | 原因 | 处理 |
|---|---|---|
| no active installation scope | 调用没有 Host 验证的 Installation context | 通过绑定 Installation 的 Host 路径调用 |
| reference is not allowed | ref 不在 `allowed_secret_refs` | 更新 Installation policy |
| installation entry absent | 本地值缺失且 fallback 关闭 | 写入 Installation store 或明确启用 fallback |
| outbound denied | manifest/handle/policy 不允许 destination | 修正声明并重新审批 |
| binding ambiguous | 多个 provider 同等兼容 | 在 Work/Assembly 中显式绑定 provider |
| run gap | Run start 缺少本地匹配实现、绑定或可满足 target | 展示 `reason_code` 与 `next_step`，修复 Installation/本地 Package 后重试 `host.run.start` |

Secret resolver 细节见 [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md)；Work 与 Installation 边界见 [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md)。
