# Host 项目级权威

> [English](./HOST_PROJECT_AUTHORITY.en.md) · [中文](./HOST_PROJECT_AUTHORITY.md)

状态：**Candidate 实现**。本文定义的项目/目标级授权边界已经接入 Host Access、RPC、项目 session、部署和受控开发路径；字段与行为仍按 Candidate 管理，在跨平台验证和跨 operation receipt 闭环充分前不视为 Stable。

当前实现：

- HTTP、Cookie、Bearer 与 RPC 在 Host Access 层统一保留逻辑 `host_device` 身份、grant、delegation chain、action 和结构化资源 selector；为保持冻结的 Contract V1 principal union，RPC context 使用 fail-closed 的 `anonymous` sentinel 加 Host 建立的 authority envelope，旧 runtime 忽略 envelope 时只会拒绝；
- project/session/event/proposal/surface/target/exec/port/proxy 在服务端执行精确资源校验或过滤，legacy adapter 复用 canonical policy；
- pairing journal 支持衰减委托、期限、祖先撤销级联、旧全局 grant 的显式 wildcard 水化，以及有界、原子的管理员批量撤销；Web/PWA 与 `plurora host access` CLI 使用同一 API；
- 设备调用写入不含凭据与请求 payload 的 `host/control/v1/authority.decision` allow/deny journal；
- 全局 package/capability/asset/projection 与 surface contribution 尚无项目归属，因此精确项目设备只能通过已验证项目/session 路径操作资源和解析其项目 bundle，不能枚举 Host 全局 catalog；opaque-origin frame 只获得绑定 grant 和 bundle root 的五分钟只读 asset lease，原始静态路径仍要求 Host 身份；
- 部署 job 与 activation 持久化不含凭据的 authority lease，在每个新副作用前重新水化 grant 并拒绝过期/撤销，同时持久化 revision/direct-route ownership；跨 operation 的完整 effect-receipt 串联仍是 Candidate 边界。
- development execute/recover 在内存中保留不含凭据的认证身份，在 Docker 与 managed-workspace 效应前刷新 grant journal，并在阻塞验证结束后再次复核。撤销无法取消已经在途的单个效应，但会阻止清理、promotion 或任何后续用户效应；之后必须由当前仍获授权的身份显式恢复并对账 durable 状态。

## 目标

Plurora 的宪法基底拥有 principal、认证后的调用上下文、能力的衰减/转授/撤销和审计机制；Project 则是 Host Control Plane 资源。项目级隔离必须利用前者保护后者，不能把 `Project` 提升为内核内容本体，也不能继续把 caller 提交的 `session_id` 当作授权证明。

完成后应同时满足：

- root、设备、CLI、Web/PWA、桌面、package surface 和未来 target agent 通过同一套认证上下文进入公开协议；
- grant 可以限制到明确的 project、target 和 Host 操作；
- session 只携带 Host 已验证的项目绑定，不能扩大调用者权威；
- transport、路径别名或 legacy adapter 不能绕过资源授权；
- 每个允许或拒绝的敏感操作都能关联主体、grant、delegation、资源和 effect receipt。

## 分层边界

```text
Constitutional Substrate
  AuthenticatedCallContext / AuthorityRef / ResourceRef / delegation / audit

Host Control Plane
  ProjectId / TargetId / Host action / resource policy / session binding

Experience and package layers
  receive attenuated handles; never receive Host root or device credentials
```

基底只需要理解结构化 `ResourceRef { owner, kind, id }`，例如 owner=`host`、kind=`project`。它不解释 Project 的业务含义。Host policy resolver 负责把方法参数、路径、session binding 和对象归属解析成资源集合。

## 两级调用上下文

### AuthenticatedCallContext

所有 transport 在认证完成后必须构造同一份不可由请求 body 覆盖的上下文：

```text
AuthenticatedCallContext
  principal_ref
  credential_kind
  grant_ref?
  delegation_chain[]
  authority_refs[]
  transport
  audience_host_id
  issued_at / expires_at?
  correlation_id / parent_invocation_id?
```

上下文表达“谁以哪条权威链发起调用”。Bearer、Cookie、stdio、in-process 或后续 mTLS 只能改变认证适配器，不能改变授权语义。

### HostOperationContext

Host 在分派具体方法前解析请求资源并产生：

```text
HostOperationContext
  authenticated_call
  action
  resources[]
  verified_project_binding?
  target_ref?
  operation_ref?
  policy_decision_ref
```

运行时需要项目范围时，只接收这份验证后的资源上下文或由它铸造的能力句柄。它不得从未验证 JSON、URL 参数或 session metadata 自行推断权威。

## Grant 模型

Host grant 保留当前 opaque、只存 digest 的 bearer credential；当前合同不要求换成自包含 token。权威语义先稳定，再决定是否采用 Biscuit 等编码。

候选 grant 形状：

```text
HostGrant
  id
  subject
  actions[]
  resource_selectors[]
  parent_grant_id?
  delegation_depth
  issued_at / expires_at
  revoked_at?
  credential_digest
```

`resource_selectors` 是结构化列表，不使用字符串前缀匹配：

- `host/project/<exact-id>`；
- `host/target/<exact-id>`；
- `host/all-projects` 或 `host/all-targets`，只能由拥有相同范围的主体转授；
- 与资源无关的 Host 动作仍需显式 selector，例如 `host/access-registry`。

规则：

1. 新 grant 的 actions、resources、期限和 delegation depth 都必须是调用者权威的子集。
2. root 是本 Host 的根权威，但也必须经过公开 API 和审计路径。
3. 设备身份不能在 RPC 边界被折叠成无约束的 `HostDev`。
4. grant 撤销影响所有新调用；已开始操作是否取消由该操作的 lease/policy 明确决定。
5. legacy 全局设备 grant 迁移为显式 `all-projects` / `all-targets` selector，迁移必须写审计事件，不能静默改变语义。

## 方法授权

每个 canonical 方法登记以下元数据：

```text
MethodPolicy
  action
  resource_extractor
  project_binding_requirement
  anonymous_allowed = false
  failure_mode = deny
```

授权顺序固定为：

1. transport 认证凭据并建立 `AuthenticatedCallContext`；
2. canonicalize 方法名，legacy alias 只能指向相同 policy；
3. resource extractor 从已解析参数和服务端投影提取资源；
4. 验证 session/project/object 归属，拒绝冲突；
5. policy engine 对 action × resources × authority 求交；
6. 记录 policy decision；
7. 才能调用运行时或产生外部副作用。

未知路径、未知方法、无法解析的资源和缺失绑定全部 fail closed。读取列表也必须在服务端按可见资源过滤，不能先返回全集再依靠客户端隐藏。

## Session 与项目绑定

`session_id` 是定位符，不是 capability。安全绑定必须满足：

- session 创建时由 Host 写入不可变或受控变更的 `project_id` 绑定；
- 调用方请求中的 project、session 和目标对象归属必须一致；
- 只有同时拥有该 project action 的主体才能使用绑定 session；
- package surface 获得的是绑定 session 和 allowlist 的短期句柄，不是原始 Host grant；
- fork、恢复、归档和删除都保留或显式变更绑定，并写审计事件；
- resolver、secret、event、artifact、proposal 和 deployment 查询统一消费已验证绑定。

## 审计连接

敏感调用至少记录：

- principal、credential kind、grant id；
- delegation chain digest；
- canonical method、action、resource refs；
- session/project/target/operation refs；
- allow/deny 与 policy reason；
- correlation/causation；
- 后续 effect receipt 或 terminal failure。

凭据原文、secret 值和完整 Cookie 永不写 journal。

## 威胁与必须阻断的路径

| 威胁 | 防线 |
|---|---|
| 用项目 A grant 提交项目 B 的 `session_id` | 服务端 session binding 与 resource selector 交叉验证 |
| 通过 legacy 方法名逃逸 | alias 在授权前 canonicalize，共用 MethodPolicy |
| 直接 HTTP RPC 获得 `HostDev` 权限 | 保留 authenticated principal/grant，不做身份折叠 |
| 列表/事件流泄露其他项目 | 服务端 projection 过滤，subscribe 时固定 resource scope |
| iframe/project bundle 偷取 Host token | surface bridge 只暴露短期项目句柄与方法 allowlist |
| 字符串前缀碰撞 project id | 结构化、精确比较的 ResourceRef |
| grant 被撤销后继续创建副作用 | 每次调用检查投影；长操作通过 lease epoch 再授权 |
| in-process 或 stdio 绕过 HTTP middleware | transport-neutral dispatch 强制要求认证上下文 |

## 合同与兼容策略

- 新字段先作为 optional 进入 Experimental/Candidate schema，旧客户端缺省表示原有全局范围。
- 新建 grant 的 UI/API 在迁移完成后必须显式提交 resource selectors。
- canonical owner 是 `host.access`、`host.project` 和相关 Host 方法；`platform.*` 仅保留 legacy adapter。
- 对每个 canonical/legacy/direct transport 组合运行同一授权 conformance table。
- 在全局 grant 迁移和客户端升级完成前，不移除旧字段或旧响应形状。

## 实施顺序

1. 增加通用认证上下文和结构化资源 selector，不改变现有行为。
2. 让 HTTP/root/device 身份完整进入 runtime dispatch，删除远程设备到 `HostDev` 的折叠。
3. 建立 session/project/object 服务端绑定校验，并覆盖 event、secret、artifact 和 resolver。
4. 扩展 pairing/grant 投影、delegation 和审计。
5. 迁移 UI/CLI，随后把无 selector 的新 grant 改为拒绝。

## 完成门槛

- project A-only device 对项目 B 的 get/list/event/secret/develop/deploy/route 全部拒绝；
- 伪造 session、legacy alias、直接 transport 和重放旧 grant 都不能绕过；
- root、全局设备和迁移前客户端维持明确、经过测试的兼容行为；
- revoke、expiry、delegation attenuation 和批量撤销有并发测试；
- 审计能从用户动作追到 policy decision 和 effect receipt，且不泄露凭据。
