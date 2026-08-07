# Target Agent 协议

> [English](./TARGET_AGENT_PROTOCOL.en.md) · [中文](./TARGET_AGENT_PROTOCOL.md)

状态：**Candidate 实施合同**。Target Agent 是 Host Control Plane 的远程执行适配器，不是 remote package、通用 SSH shell、第二个 Host 或被部署应用的身份系统。身份、类型化 operation、artifact、Docker deployment 和 authenticated reverse tunnel 已形成同一受控边界；自动调度与 target-edge ingress 不在当前合同内。

## 三种远程边界

| 边界 | 主体 | 目的 | 凭据/信任 |
|---|---|---|---|
| 远程 Host 客户端 | 人的设备、Web/PWA、CLI | 控制一个 Host | root 或 project-scoped device grant |
| Remote Target Agent | 受管执行节点 | 执行部署/验证操作并报告事实 | 独立 target identity + operation authority |
| Remote package entry | 能力提供者服务 | 响应 package invoke/stream | package/workload identity + attenuated capability |

三者不能共享 bearer token、生命周期或隐式权限。尤其不能用 Host device token 当 agent credential，也不能把 target exec 注册成 package invoke。

## 角色

- **Controller：** Host 内的 desired-state、operation、lease、policy 和 audit 所有者。
- **Agent：** target 本地执行类型化操作、持久保存 operation ledger、观测本地实体。
- **Target driver：** Controller 侧 transport-neutral 接缝；local driver 与 remote driver 通过相同语义 conformance。
- **Artifact service：** 以 digest 提供有授权、可校验的内容传输。
- **Ingress/tunnel adapter：** 把 Host route 连接到 agent loopback service，不把任意远程 IP 伪装成本地 lease。

一个 Agent 不是 Host：它不拥有 project catalog、grant registry、用户 session、package marketplace 或最终 deployment intent。

## Target 记录与生命周期

```text
ExecutionTarget
  id
  display_name
  reachability: local | direct | reverse_tunnel
  identity_ref
  protocol_versions[]
  declared_capabilities[]
  effective_capabilities[]
  labels{}
  status
  last_seen_at?
  enrolled_at / revoked_at?
```

状态至少包括：`Enrolling`、`Available`、`Degraded`、`Offline`、`Draining`、`Incompatible`、`Revoked`。

`declared_capabilities` 是 agent 声明；`effective_capabilities` 是声明、Host policy 和已验证探测的交集。调用者提交一个 JSON target 不能直接令它成为 Available。

## Enrollment 与身份

1. root 或拥有 target-manage 权威的主体创建短期、单次 enrollment challenge；
2. Agent 生成自己的密钥并提交 challenge、public key、版本和能力；
3. Host 验证 challenge，铸造/登记有 Host audience 的 target identity；
4. 后续 session 使用 mTLS 或等价的双向认证，证书/密钥支持轮换；
5. revoke 后拒绝新 session 和新 operation；正在运行 workload 的处理由 drain/revoke policy 决定。

Host journal 只保存 public identity、credential digest/serial、状态和审计引用，不保存 agent private key。第一版可以使用 Host 管理的 CA；协议语义应允许后续接入 SPIFFE，而不把 SPIFFE 部署要求写入核心模型。

### 已实现的 identity/observation 与 typed-worker 切片

`target-agent.v1` 的身份与观测控制面：

| 调用方 | 路由 | 权威与作用 |
|---|---|---|
| Host 客户端 | `POST /host/v1/targets/{target_id}/enrollments` | `deploy` scope + target selector；创建最长 15 分钟的单次 challenge |
| Agent | `POST /target-agent/v1/enroll` | 消费 challenge，协商版本/能力并一次性接收 Host 生成的 bootstrap target credential |
| Agent | `POST /target-agent/v1/heartbeat` | 独立 `YggTarget` credential；刷新 observation 与 45 秒 liveness |
| Host 客户端 | `GET /host/v1/targets/{target_id}/observe` | `observe` scope + target selector；读取声明、有效能力、epoch 与观测摘要 |
| Host 客户端 | `POST /host/v1/targets/{target_id}/revoke` | `deploy` scope + target selector；撤销身份并同时推进 lease/policy epoch |

Enrollment token 和 agent credential 只以带 domain separation 的 SHA-256 digest 进入 `host_control_target_agents` journal；challenge 单次消费，重启后非 revoked target 先回到 `Offline`，旧凭据与旧 epoch 不能恢复为可用状态。`kernel.v1.target.register/unregister` 保留兼容方法名但 fail closed，调用方 JSON 不能绕过该流程制造 `Available` target。

Typed-worker 控制面暴露以下路由；它们不提供通用命令：

| 调用方 | 路由 | 权威与作用 |
|---|---|---|
| Host 客户端 | `POST/GET /host/v1/targets/{target_id}/operations` | `deploy`/`observe` scope，同时约束 target 与 project selector；创建或列出类型化 operation |
| Host 客户端 | `GET /host/v1/targets/{target_id}/operations/{operation_id}` | 读取 project-scoped durable operation/receipt |
| Agent | `GET /target-agent/v1/operations/next` | 仅向 live、epoch 匹配的 target 返回其未终结 operation |
| Agent | `POST /target-agent/v1/operations/{operation_id}/progress` | 持久记录 accepted/running；首个随机 `execution_id` 获得唯一执行权 |
| Agent | `POST /target-agent/v1/operations/{operation_id}/receipt` | 只接受与 authority、execution owner 和 request digest 一致的终态回执 |
| Agent | `GET /target-agent/v1/operations/{operation_id}/artifacts/{digest}` | 只流式读取该 accepted/running operation 明确授权的 digest |

Host 的 `host_control_target_operations` journal 与 Agent 的 SQLite ledger 都使用 expected-sequence CAS。Agent 在回复 accepted 前持久化 request/authority digest，在提交回执前持久化 terminal receipt；同一 data directory 有进程锁，复制 credential 到另一 ledger 也不能夺取已经绑定的 `execution_id`。当前可执行类型是 `artifact.materialize/release`、`health.probe`、声明式 `verifier.run(artifact_integrity)` 与 `deployment.apply/observe/drain/stop`；未知类型没有 shell fallback。下载先进入 digest 派生的 partial 文件，完整 SHA-256/size 校验后才原子落入本地 CAS，失败 partial 会删除。

Revoke 对新工作和新的 accepted/running 转换 fail closed。线性化边界是 Host 已持久确认 `Running`：该边界前观察到 revoke/offline/stale epoch 就不执行；边界后的当前幂等 step 可完成或重放 receipt，但不能取得新 operation。Revoke 不伪装成对 target-local effect 的原子回滚；`deployment.drain` 执行有界优雅停止并保留容器，`deployment.stop` 删除容器，只有显式 `force_remove` 才允许强制删除。

Operation authority 绑定 target/operation/step/project/effect/artifact/lease epoch/policy epoch/expiry/nonce/request digest。远程 Agent authority 用 enrollment credential 的 domain-separated digest 作为 epoch-scoped HMAC key，并由 Agent 以一次性获得的 credential 独立复算；local driver 的 journal record 使用仅限 Host 内部、稳定且 domain-separated 的 local key，不把它当作跨网络身份。原生客户端禁用 redirect，远程 Host 强制 HTTPS，credential 不写入配置或 ledger，只从 `YGG_TARGET_AGENT_CREDENTIAL` 注入；loopback HTTP 仅用于同机边界。

Driver routing 按 `ExecutionTargetReachability` 选择 local 或 Agent，不接受调用方提供的任意网络地址作为 fallback。Local 与 Agent 的部署 operation 共用一个类型化 Docker driver：固定非特权 bridge、只绑定 `127.0.0.1`、不接受 command/env/mount，并以 target/project/deployment/route/lease/operation ownership labels 幂等查找；`apply` 回执返回 Docker 实际分配的 loopback port。Host 将成功回执中的实际端口投影回 target-owned lease，并只在 route/lease/project/target 全部匹配时提升 route readiness；重启恢复不会用 Host-local Docker 观察误删远端 lease。Docker effect 发出后无法确认结果时回执为 `outcome_unknown`，不会误报失败；Host 重启时遗留的 local Accepted/Running 也持久收敛到该状态。

authenticated reverse tunnel/private preview 的 Candidate 基线也已实现。具有 `reverse_tunnel` reachability 与 Deployment 能力的 Agent 使用同一 `YggTarget` 身份主动连接 `GET /target-agent/v1/tunnel`；Host 对每个 target 只接受一个 identity/lease epoch/policy epoch 都匹配的 live tunnel。每次 `Open` 都精确绑定 target、route、port lease、port name、Docker 实际端口和两个 epoch；有界 binary stream 只使用 Host 生成的 opaque stream ID 复用。Agent 在连接前重新验证受管容器的全部 ownership labels、Running 状态以及精确的 `127.0.0.1` 端口映射，因此 tunnel 不能被用来拨号任意 Agent loopback 端口。断线或 revoke 会令该 target 的 route 立即 unready，重连后再按 durable receipt 投影恢复；public/Host-authenticated 判定仍只属于 Host route policy，Host 也没有放宽任意网络 upstream。

## Transport session

协议在认证后的双向 session 上运行，不依赖谁发起 TCP 连接：

```text
Hello(target_id, identity, versions, nonce)
Welcome(host_id, selected_version, session_id, policy_epoch)
Heartbeat(observed_summary, receipt_cursor)
OperationRequest(operation, step, lease_epoch, authority, body)
OperationAccepted | OperationRejected
OperationProgress(sequence, diagnostic_refs)
OperationReceipt(terminal result)
ObserveRequest / ObserveSnapshot
CancelRequest / CancelReceipt
ArtifactRequest / ArtifactChunk / ArtifactReceipt
```

当前 V1 不把 durable operation control 混进易失的流量 tunnel：identity、heartbeat、operation、receipt 与 artifact 使用版本化 HTTP 路由，Agent 另行主动建立带 `YggTarget` 认证的 WebSocket，只承载被 Host route 授权的 multiplexed byte streams。operation 恢复依赖 Host journal 与 Agent ledger，而不是 tunnel 内存 channel。未来直接 mTLS HTTPS/HTTP2 可以替换连接适配器，但不能改变 operation authority、receipt 或 fencing 语义。

## 类型化操作

Agent 只接受公开、版本化、策略可判定的操作类型：

- `artifact.materialize` / `artifact.release`；
- `deployment.apply` / `deployment.observe` / `deployment.stop` / `deployment.drain`；
- `health.probe`；
- `logs.read` / `logs.follow`；
- `port.reserve` / `port.release`；
- `tunnel.open` / `tunnel.close`；
- `verifier.run`，只接受声明式 verifier descriptor。

不得提供 `shell(command: string)`。需要 process executor 时，program、args、cwd、env、network、mount、resource limits 和输出上限必须受 target policy 与 operation authority 同时约束；未知字段和未知操作拒绝。

## Operation authority 与 fencing

每个 request 携带：

```text
OperationAuthority
  audience_target_id
  operation_id / step_id
  project_resource_ref
  allowed_effect
  artifact_digests[]
  secret_envelope_refs[]
  lease_epoch
  issued_at / expires_at
  nonce / authority_digest
```

语义要求：

- 只能用于一个 target、operation、step 和 effect；
- 不能创建子 grant，除非权威显式允许；
- Agent 验证 Host identity、audience、expiry、policy epoch 和 lease epoch；
- 同一 `(operation_id, step_id, request_digest)` 是幂等键；重复请求返回已存 receipt；
- 同一 step 使用不同 digest 是冲突，不得覆盖；
- 更旧 lease epoch 的请求即使签名有效也必须拒绝。

第一版 authority 可以是 session 内经 mTLS 认证、Host 签名或 MAC 的短期 token。编码选择不能削弱以上语义。

## Agent operation ledger

Agent 必须在确认 accepted 前持久保存 request digest 和 epoch，在确认 terminal 前持久保存 receipt。最少状态：

```text
accepted -> running -> succeeded | failed | cancelled
                     -> outcome_unknown
```

Agent 重启后扫描 ledger 与带 ownership label 的本地实体，恢复 progress 或产生真实 observed snapshot。无法确认的效果标为 `outcome_unknown`，不能假造 success/failure。

## Artifact 与 secret

Artifact：

- 以 `sha256:<hex>` 等内容地址引用；
- 先检查本地 CAS，再通过授权 stream 分块获取；
- 每块有序号，完整对象落盘前验证总 digest；
- partial download 有 lease/期限并可恢复；
- reachability 由 active/previous/in-flight/pinned revision 决定；
- provenance、签名和 media type 与对象一起验证。

Secret：

- journal、artifact descriptor 和 agent log 只保存 `secret_ref`；
- Host 只对明确 target identity 和 operation 制作短期 envelope；
- Agent 在最后可能时解密，使用 tmpfs/受限环境或 executor-native secret mount；
- terminal 后销毁临时材料；receipt 只记录引用和 redaction 证明；
- Agent 不提供通用 secret list/get API。

## 网络与 ingress

现有 Host loopback-only upstream 是安全边界，不直接扩展为任意 `host:port`。

已实现的远程 route：

1. Agent 实际 reserve/bind target loopback port；
2. Controller 注册 target-aware route；
3. Host proxy 经认证 tunnel 打开到该 lease 的有界 stream；
4. Agent 验证 route、lease、port name、实际端口、ownership labels 和 epoch 后，只连接 target loopback；
5. public/host-authenticated access 仍由 Host route policy 决定。

Host 在内部使用有并发上限的一次性 loopback bridge credential 复用既有 HTTP/WebSocket 代理，credential 在进入应用前被剥离；客户端 Authorization/Cookie 不会传给 workload，WebSocket 只显式转发 Origin、协商后的 subprotocol 与经验证的 vhost authority。这样 private preview 和公开 Host 后的应用可以复用同一入口。Target-side public ingress、ACME 和边缘身份是后续 adapter，不在第一版偷偷放宽。

## 故障与恢复

| 情况 | 行为 |
|---|---|
| heartbeat 超时 | target Offline；不立即假定 workload 消失 |
| session 重连 | identity/epoch 重验，从 receipt cursor 续传 |
| Host 在请求后崩溃 | 新 controller 先 Observe/ledger lookup，再决定重试 |
| Agent 在启动 workload 后崩溃 | 重启扫描 ownership 与 ledger，报告 observed truth |
| 网络分区双方继续运行 | Agent 只接受未过期 authority；Host 不发第二个同 epoch owner |
| target 被 revoke | 拒绝新操作；按 policy drain/保留现有 workload |
| artifact digest 不符 | 删除 partial，对该 step terminal fail 并审计 |
| tunnel 中断 | route 暂时 unready；deployment intent 保留，允许重连 |
| 协议版本不兼容 | target Incompatible，不降级执行未知语义 |

## 能力层级

1. **Identity and observation：** durable target registry、enrollment、heartbeat、capability negotiation、observe。
2. **Typed verifier worker：** artifact transfer、声明式 verifier、receipt/log，暂不承载公开流量。
3. **Private deployment preview：** deployment/port/tunnel operation 与 Host-authenticated route。
4. **Public deployment：** 在 Host 已经公网可达时显式 public route；随后再设计 target-side edge。

当前只允许用户显式选择 target。不实现自动 placement、多 Host 调度、leader election 或跨 Host secret federation。

## 完成门槛

- local driver 与 remote agent 对同一 operation conformance 产生等价状态/receipt；
- 重复、乱序、过期和 stale-epoch request 均被确定性处理；
- Controller/Agent 在每个 step 崩溃或断线后不产生重复 workload；
- revoke、drain、offline/reconnect、artifact corruption 和 protocol mismatch 有测试；
- Agent 无通用 shell、无 Host device credential、无跨 project artifact/secret 访问；
- remote route 不通过放宽任意网络 upstream 实现。
