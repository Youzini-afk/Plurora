# Conformance 矩阵

> [English](./CONFORMANCE_MATRIX.en.md) · [中文](./CONFORMANCE_MATRIX.md)

Conformance 套件是 charter 的可执行守卫。它同时证明正向行为和拒绝行为。新用例会在添加时收入此处。标记为 partial 或 future 的用例仍在后续加固范围内，见 `docs/roadmap/NEXT_STEPS.md`。

## 当前发布门槛命令

```bash
cargo test --workspace
cargo run -p plurora-cli -- conformance
```

当前矩阵记录已实现的 conformance 覆盖。具名 CLI 用例和 crate/service 单元测试共同支撑这些结果。当前 CLI conformance 总数：**453**。

## Conformance Feedback Loop

Conformance 命令支持过滤、计时和诊断。详见 [`docs/performance/CONFORMANCE_FEEDBACK.md`](../performance/CONFORMANCE_FEEDBACK.md) 与 [`docs/performance/PERFORMANCE_AND_CODE_HEALTH.md`](../performance/PERFORMANCE_AND_CODE_HEALTH.md)。

```bash
# 列出所有 case id 和 tags
cargo run -p plurora-cli -- conformance --list

# 按 substring 过滤
cargo run -p plurora-cli -- conformance --case sharing_lab

# 按 tag 过滤
cargo run -p plurora-cli -- conformance --tag sharing

# fail-fast
cargo run -p plurora-cli -- conformance --fail-fast

# 自定义 slowest 报告
cargo run -p plurora-cli -- conformance --slowest 3
```

## 当前 conformance 覆盖

### Installation model conformance cases

以下用例覆盖 Installation journal、secret policy 与 destructive no-alias 边界。实际 case id 可用 `cargo run -p plurora-cli -- conformance --list | grep -E "(installation|installation_secret)"` 核对。

| 分组 | Case id | 覆盖 | 状态 |
|---|---|---|---|
| Installation protocol | `installation.protocol_crud_uses_service_registry` | 五个 RPC 使用同一 Service registry owner | implemented |
| Installation idempotency | `installation.idempotency_replay_and_conflict` | 同 fingerprint 重放、不同 fingerprint 冲突 | implemented |
| Installation concurrency | `installation.stale_revision_rejected` | stale expected revision 被拒绝 | implemented |
| Installation state | `installation.state_migration_explicit` | 不兼容 state 需要 migration/reset | implemented |
| Installation remove | `installation.remove_keep_delete_distinct` | keep/delete 是显式不同终态 | implemented |
| Installation restart | `installation.journal_restart_rehydrate` | journal 重启后重建 projection | implemented |
| Installation authority | `installation.exact_authority` | exact resource selector 不可越权 | implemented |
| Installation events | `installation.lifecycle_events` | created/updated/removed payload 对齐 public schema | implemented |
| destructive identity | `installation.retired_methods_invalid_request` | 已退休 method 无 alias | implemented |
| Installation secret | `installation_secret.put_resolve_owned_path` | 只读写 Installation-owned secrets path | implemented |
| Installation secret | `installation_secret.policy_fallback_and_retired_scheme` | policy 控制 fallback，并拒绝已退休 scheme | implemented |
| Installation secret | `installation_secret.isolation_and_list_redaction` | Installation 隔离且 list 不返回值 | implemented |

### End-to-end real-path conformance cases

以下是 Phase 3 保留的 surface resolve 覆盖。Run/session 生命周期在 Phase 4 建立，不在本 Phase 伪造。

| 分组 | Case id | 覆盖 | 状态 |
|---|---|---|---|
| dev bundle | `surface.resolve_via_dev_path` | dev path surface bundle resolution | implemented |
| bundle rejection | `surface.resolve_unknown_fails` | unknown surface bundle fails closed | implemented |
| bundle authority | `surface.resolve_admin_principal_required` | resolve_bundle 限 HostAdmin/HostDev | implemented |

Surface/static bundle 与 bridge 还覆盖以下稳定断言：

| 断言 | 覆盖 | 状态 |
|---|---|---|
| static surface bundle | `surface_bundle` 是静态浏览器入口，不走 wasm sentinel 或 package execution | implemented |
| Package surface source | bundle 从受 Host 身份保护的 `/surface-bundles/packages/<package-id>/...` 解析 | implemented |
| sandbox asset attenuation | 授权解析签发绑定 grant/bundle root 的五分钟 `/surface-assets/<lease>/...`；跨 root、伪造、过期或撤销 grant 均拒绝 | implemented |
| bridge allowlist | typed `allowed_capability_ids` 精确约束 surface bridge 可调用能力 | implemented |
| metadata not authority | surface metadata 只描述入口，不授予权限 | implemented |
| stream ownership | stream subscribe/unsubscribe 绑定发起 surface 与 session，不能接管他人 stream | implemented |
| redacted diagnostics | bridge 诊断、错误和日志不泄漏 raw secret 或 host 绝对路径 | implemented |
| uncontrolled secret input | secret 输入保持 uncontrolled/短生命周期，关闭时清理 | implemented |
| schema timestamp stability | schema/export timestamp 稳定，不引入非确定性时间戳 | implemented |
| content-addressed freshness | bundle bytes 进入 artifact closure，变化会改变 digest | implemented |
| no fake Run | Installation detail 返回 Phase 4 unavailable reason，不创建 session | implemented |


| 领域 | 用例 | 状态 |
|---|---|---:|
| session | 开启内容无关 session | implemented |
| events | 已授权包追加自身 namespace 事件 | implemented |
| events | 包在无 `events.append` 时被拒绝写入 | implemented in unit tests |
| events | 包在无 `events.read` 时被拒绝读取 | implemented |
| events | 包被拒绝写入他人 namespace | implemented in unit tests |
| events | Package 写入 `context/opened` 等 registry 平台事件时被拒绝 | implemented in unit tests |
| events | 已关闭 session 拒绝追加 | implemented |
| events | 带过滤条件的 sequence-range replay | implemented |
| package | 有效 manifest 加载成功 | implemented |
| package | lifecycle 时间线发出 loading/starting/ready/loaded | implemented |
| package | 重启 subprocess 包 | implemented |
| package | 捕获 subprocess stderr 日志 | implemented |
| package | host 策略拒绝不允许的 entry | implemented in unit tests |
| package | unload 移除注册记录 | implemented in unit tests |
| package | unload 移除 capability provider | implemented |
| capability | 发现已注册的 capability | implemented |
| capability | 通过 package trait 调用 rust_inproc echo | implemented |
| capability | 模糊 provider 被拒绝 | implemented in unit tests |
| capability | 显式 provider 选择解决重复 provider | implemented |
| capability | 版本约束过滤 provider | implemented |
| publisher equality | 官方外观的包无路由优先 | implemented |
| hooks | veto fixture 报告 veto | implemented in unit tests |
| hooks | 按 precedence/package/handler 稳定排序 | implemented |
| hooks | before event append veto 阻止操作 | implemented |
| hooks | before event append metadata 变更生效 | implemented |
| hooks | 包拥有的 hook handler capability 被调用 | implemented |
| hooks | unload 移除 hook 订阅 | implemented |
| storage | SQLite 持久化/replay 事件 | implemented in unit tests |
| assets | put/get/list 通过 SHA-256 descriptor 适配，事件不含正文 | implemented |
| assets | 旧 FNV inline event 幂等迁移并保留旧 id/hash/event provenance | implemented |
| object store | 跨宿主同摘要、未知类型可复制/stream、篡改拒绝 | implemented |
| sessions | fork session 并列出 branch 族系 | implemented |
| projections | 注册并 rebuild 通用事件计数 projection | implemented |
| substrate | SQLite 事件日志 rehydrate asset、branch 和 projection | implemented |
| substrate | permission grant 在 SQLite-backed runtime rehydrate 后仍存在 | implemented |
| effect receipts | capability/provider 卸载后 historical replay 仍读取 recorded output；缺失 object 明确报 incomplete history；re-execute 创建新 branch 和 parent-linked receipt | implemented |
| effect receipts | raw secret-bearing input/output 只以 redacted object refs 进入 receipt，receipt envelope 扫描无 findings | implemented |
| secret refs | `secret_ref:`、`secretRef:`、`secret-ref:`、`host:` reference pattern validation | implemented |
| secret refs | proposal payload 中的 raw secret 会被拒绝 | implemented |
| secret refs | asset metadata 中的 raw secret 会被拒绝 | implemented |
| secret refs | 第一方 Package 没有 secret-scanning bypass | implemented |
| env resolver | `EnvSecretResolver` 在 env name 于 allowlist 中时允许解析（`secret_ref:env`、`secretRef:env`、`secret-ref:env`、`host:env`） | implemented |
| env resolver | `EnvSecretResolver` 在 env name 不在 allowlist 中时拒绝解析；非 env vault 和 `host:<key>` 被拒绝 | implemented |
| env resolver | `EnvSecretResolver` 缺失 env var 返回 typed error，不泄漏 raw value | implemented |
| secret store | 10 个 secret_store 用例：put / has / list / delete / health + env/store/composite resolver paths | implemented |
| protocol | 方法列表不包含内容方法 | implemented in unit tests |
| protocol | 结构化权限错误码 | implemented |
| protocol / identity | Registry 对每个 method 只暴露一个 wire ID，不发布 alias，并拒绝未知 identity | implemented |
| protocol / owner namespace | smoke 只调用 Host/Shell/Change/Projection owner ID，并显式协商 default 与 Shell Default profile | implemented |
| protocol / negotiation | 未知 layer version 明确返回 `unsupported_contract` | implemented |
| protocol / negotiation | 协商失败不静默回退，且业务 handler 零副作用 | implemented |
| protocol | in-process 协议分发器调用 host.info | implemented |
| protocol | in-process 协议分发器调用 capability | implemented |
| protocol | HTTP `/rpc` 返回协议信封 | implemented in service tests |
| protocol | host stdio 响应协议信封 | implemented by CLI validation |
| principal | 包上下文覆盖调用者提供的 event writer | implemented |
| principal | 包上下文覆盖调用者提供的 capability caller | implemented |
| principal | human 和 assistant 协议 principal 存在 | implemented |
| permissions | grant/revoke/list/audit 协议 | implemented |
| permissions | assistant capability 调用需要显式授权 | implemented |
| schema | capability input schema 拒绝无效输入 | implemented |
| schema | event payload schema 拒绝无效 payload | implemented |
| subprocess | JSON-RPC stdio 包加载并报告 ready | implemented |
| subprocess | JSON-RPC stdio capability 调用正常工作 | implemented |
| subprocess | 错误握手被拒绝 | implemented |
| subprocess | 调用超时导致包降级 | implemented |
| subprocess | 无效 subprocess 输出 schema 被拒绝 | implemented |
| subprocess | unload 移除 subprocess capability | implemented |
| service | SSE 事件订阅端点 replay 和 tail 事件 | implemented |
| host | diagnostics 报告包/capability/hook | implemented |
| host | profile 自动加载配置的包 | implemented |
| surfaces | 包贡献的类型化 surface 描述符可以列出、描述和过滤 | implemented |
| first-party Packages | 基础包无特权加载和调用 | implemented |
| first-party Packages | asset-lab 以无特权方式 preview assets 并生成需要审批的 import plans | implemented |
| first-party Packages | projection-lab 以无特权方式生成 rebuild plans 并解释 source events | implemented |
| first-party Packages | playable-seed 暴露 reference entry/play/Forge/assistant surfaces 以及需要审批的 edits | implemented |
| first-party Packages | persona-lab 以无 kernel ontology 的方式 import 并 render persona profiles，且带 provenance | implemented |
| first-party Packages | knowledge-lab normalize collections、match entries，并返回 plan-only injection output | implemented |
| first-party Packages | context-lab 组装 generic blocks，包含 budget omissions 与 template rendering | implemented |
| first-party Packages | text-transform-lab preview deterministic text transforms，包含 trace 与 validation diagnostics | implemented |
| first-party Packages | model-connector-lab validate profiles、拒绝 raw secrets，并返回 no-network discovery plans | implemented |
| first-party Packages | model-provider-lab 作为 cloud API adapter lab 列出八家 cloud provider families、validate profiles 拒绝 raw secret、package-local normalize_request 覆盖八家 dialects/endpoints、explain errors（401/429/529）、output 含 network_performed:false/inference_performed:false、无 raw secret echo；它不是平台模型抽象 | implemented |
| first-party Packages | model-provider-lab cloud adapter invoke 全部八家 provider（OpenAI chat/responses、Anthropic messages、Gemini generateContent、OpenAI-compatible chat、OpenRouter chat/responses、DeepSeek chat、xAI chat/responses、Fireworks chat/responses；fake/local、outbound_request_shape 可审计、raw credential rejected、openai_compatible 缺 base_url 或 http base_url 拒绝、unsupported family diagnostic、executor_kind fake_local、live_call_supported false） | implemented |
| first-party Packages | model-provider-lab cloud adapter normalize_stream 八家 provider stream normalization（delta SSE、semantic SSE、typed chunk stream → StreamFrameEnvelope frames：start/chunk/progress/end/error/cancelled/timeout；terminal_frame_consistent；provider event 输入归一化；raw secret 不 echo；unsupported family empty frames + terminal_frame_consistent false） | implemented |
| outbound | model provider outbound shape fake executor（三 provider host/method/path/secret_ref shape 通过 outbound boundary、call_count=3、executor_kind Fake） | implemented |
| first-party Packages | model-routing-lab resolve deterministic route plans，包含 explicit fallbacks 与 normalized params | implemented |
| first-party Packages | pi-agent-runtime-lab 生成 no-inference/no-network run plans、approval-gated proposals、trace summaries，且 surfaces 可发现 | implemented |
| first-party Packages | capability-tool-bridge-lab 标记 ambiguous provider rejected、explicit third-party provider 可用、第一方 provider 不优先、missing provider rejected、denied preview 报告 missing permission、raw secret unsafe_blocked | implemented |
| first-party Packages | inference-local-lab describe_capabilities 不需要 network/secret，transports include in_memory/local_process，operation_kinds include generate/classify/transform | implemented |
| first-party Packages | inference-local-lab invoke non-HTTP succeeds，无 URL/header/status/messages 字段，network_performed=false，transport_performed=in_memory_fake | implemented |
| first-party Packages | inference-local-lab invoke rejects http transport、HTTP-shaped 字段（url/header/status_code）、messages-shaped 字段（messages/system/user/assistant）、raw secret | implemented |
| first-party Packages | inference-local-lab stream emits deterministic start/chunk/progress/end frames，无 URL/header/status/provider_schema | implemented |
| first-party Packages | inference-local-lab explain_error 覆盖 local/resource 错误类（local_process_failed/local_resource_exhausted/local_model_not_loaded/local_inference_error/timeout/cancelled） | implemented |
| first-party Packages | inference-playtest-lab draft_proposal 产 proposal_draft，含 requires_user_approval=true、asset.put、source_inference provenance、无 raw secret、不是 chat message | implemented |
| first-party Packages | inference-playtest-lab inspect_proposal 返回 risk/operations/permissions/provenance summary，不 apply | implemented |
| first-party Packages | inference-playtest-lab 被拒绝的 proposal 不能 apply | implemented |
| first-party Packages | inference-playtest-lab approve/apply 成功，asset 被写入，branch_plan + fork 创建 branch，branch metadata 包含 proposal/source inference provenance | implemented |
| first-party Packages | inference-playtest-lab 输出不含 messages/prompt/chat/platform.model 等术语 | implemented |
| in-process packages | non-first-party `/preview` suffix 不会获得第一方 asset-lab fallback 行为 | implemented |
| in-process packages | unknown registered in-process capability loud fail，而不是返回 generic fallback success | implemented |
| first-party Packages | assistant-lab 通过授权返回需要审批的 proposal | implemented |
| play-creation | 空白循环演练 assistant proposal、branch、asset、projection | implemented |
| proposals | 已批准的 proposal 可以执行通用 asset/projection 操作 | implemented |
| proposals | 被拒绝或未批准的 proposal 不能执行 | implemented |
| proposals | v1 Proposal 映射为 Intent/ChangeSet/PolicyDecision/Commit；apply/reject 产生 operation/final receipt | implemented |
| package authoring | 生成的 Python subprocess 包通过本地 conformance | implemented |
| package authoring | 生成的 TypeScript subprocess 包通过本地 conformance | implemented |
| package authoring | 生成的 experience 包 surface 通过本地 conformance | implemented |
| Work / Assembly | 嵌套 Assembly 暴露的 Port 可解析，且保留完整 exposure chain | implemented |
| Work / Assembly | 相同 Work source 重复打包得到相同内容摘要 | implemented |
| replacement | 第三方 playable-seed surface 通过 shell.contribution.list 可发现 | implemented |
| replacement | 第三方 playable-seed 能力调用通过正常路由工作 | implemented |
| replacement | 歧义的 first-party + third-party 等效 capability拒绝路由，无 publisher priority | implemented |
| replacement | Work source 通过第三方 playable-seed 替换形状校验 | implemented |
| replacement | 第三方 agent-runtime surfaces（assistant_action/forge_panel/home_card）通过 shell.contribution.list 可发现 | implemented |
| replacement | 第三方 agent-runtime 能力调用产生 no-inference/no-network、approval-gated proposal、provenance 匹配 | implemented |
| replacement | Work source 通过第三方 agent-runtime 替换形状校验，且没有 publisher priority | implemented |
| network | 无 network permission 的包被拒绝出站，产生 outbound.denied 审计 | implemented |
| network | allowlisted host+method 允许，产生 redacted outbound.request 审计 | implemented |
| network | host/method 不匹配被拒绝 | implemented |
| network | 第一方 Package 无 network bypass | implemented |
| network | 审计记录不包含 raw secret/body，只包含 secret_ref 和 redaction_state | implemented |
| network | check_network_policy 纯函数测试 | implemented |
| outbound | 无权限时 executor 不被调用 — 被拒绝的请求不会到达 executor | implemented |
| outbound | policy/audit request 与 executor request 的 package/capability/host/method/secret_refs 不一致时 fail-closed，executor 不被调用 | implemented |
| outbound | allowlisted fake executor 返回 network_performed:false、executor_kind:fake、redacted audit | implemented |
| outbound | raw body_shape 不持久化到审计；审计 redaction_state 为 redacted/not_captured | implemented |
| outbound | secret_refs 仅存储为引用；raw secret 被拒绝/不回显 | implemented |
| outbound | host 不匹配时 redirect 被拒绝；redirect_target 检查保留为后续加固 | implemented |
| stream | 正常生命周期发出有序 frame/event | implemented |
| stream | cancel 标记 invocation 为 cancelled 并阻断后续 chunk | implemented |
| stream | timeout 标记 invocation 为 timeout 并阻断后续 chunk | implemented |
| stream | error 终端 frame 正常工作 | implemented |
| stream | 非 streaming 能力（streaming=false）被拒绝 | implemented |
| stream | 协议中无 model/agent 方法 | implemented |
| stream | capability.stream 和 capability.cancel 可通过协议分发 | implemented |
| package authoring | 生成的 networked 模板通过 check/conformance，含网络声明，无 raw secrets | implemented |
| package authoring | 生成的 streaming 模板通过 check/conformance，含 streaming capability | implemented |
| no-network readiness | faux-model-readiness 包声明网络权限、提供 streaming capability、使用 secret_ref、无 raw secrets | implemented |
| no-network readiness | faux-agent-readiness 包无网络权限、提供 streaming capability、使用 proposal/trace 模式、无 raw secrets | implemented |
| outbound | live HTTP executor 默认关闭；RuntimeConfig::default 仍 DenyAll | implemented |
| outbound | live HTTP executor 拒绝非 HTTPS URL；无网络尝试 | implemented |
| outbound | live HTTP executor response shape 不含 raw body/header/secret | implemented |
| outbound | host.outbound.execute 公开协议：package principal 通过 context 确定 package_id 不能 spoof，FakeOutboundExecutor + allowed network declaration 成功且 audit 产生 | implemented |
| outbound | host.outbound.execute spoofed package_id 被拒绝，不能代替其他 package | implemented |
| outbound | host.outbound.execute 无 network permission denied，executor 不调用 | implemented |
| outbound | host.outbound.execute response 不含 raw secret（secret_refs 仅引用） | implemented |
| outbound | host.outbound.execute `secret_headers` params 解析正确，raw secret 不出现在 response | implemented |
| outbound_execute | profile 默认 deny-all、fake/live executor 配置、包权限、capability namespace、无权限拒绝、secret_ref 声明、response 脱敏 | implemented |
| outbound_stream | `host.outbound.stream` profile 默认拒绝、fake stream frame、secret_ref 声明、capability namespace、HTTPS-only 策略 | implemented |
| outbound_websocket | `platform.outbound.websocket.*` profile 默认 deny-all、fake executor open/send/close、live executor 未启用时拒绝 | implemented |
| outbound_websocket | secret_ref 未声明 fail-closed、capability namespace 校验、默认 WSS-only | implemented |
| outbound_websocket | idle timeout 产生 error + completed、inbound max_total_bytes 终止、max_concurrent_connections 生效、可通过 `capability.cancel` 取消 | implemented |
| outbound | `host/outbound.execute.completed` 完成审计事件发出 | implemented |
| outbound | `host/outbound.stream.completed` 完成审计事件发出 | implemented |
| outbound | `host/outbound.websocket.completed` 完成审计事件发出 | implemented |
| outbound | HTTP/stream/WebSocket completion 挂接 terminal receipt；policy/executor 不一致会产生 failed receipt；timeout/cancel 不产生重复 stream terminal；所有 executor 禁用后仍可 historical replay | implemented |
| deployment exec | deny-all start 与 fake stop 产生 denied/cancelled receipt；runtime 主动观察 live terminal；自然退出/超时、重复 denial、stop/status 竞态和重启 hydration 均保持唯一终态 receipt | implemented |
| secret_ref | manifest `permissions.secret_refs` 声明：未声明 fail-closed，已声明经 host resolver 解析 | implemented |
| subprocess_outbound | subprocess SDK reverse kernel call：principal 绑定、execute 调度、stream chunks 回传 | implemented |
| sse_parser | outbound stream SSE parser basic smoke 与 partial chunk 归并 | implemented |
| live_model | live smoke 默认跳过；`PLURORA_LIVE_MODEL_TESTS=1` + provider env 才 opt-in 真实调用 | implemented |
| outbound | local loopback HTTP server secret injection：Authorization header 真实到达 server，raw secret 不在 protocol response/audit/log | implemented |
| outbound | DeepSeek SSE stream normalize canary：delta_sse start→chunk→end lifecycle，terminal_frame_consistent，no raw secrets | implemented |
| outbound | opt-in live DeepSeek conformance：默认跳过，PLURORA_LIVE_MODEL_TESTS=1 + DEEPSEEK_API_KEY 时才尝试 | implemented |
| outbound | canary DeepSeek profile shape：normalize_request endpoint/dialect/stream_family 正确，secret_ref placeholder 不含 raw key | implemented |
| outbound | OpenAI Chat Completions loopback：Authorization Bearer 到达 server，POST /v1/chat/completions，body shape model+messages，raw secret 不在 response/audit | implemented |
| outbound | OpenAI Responses loopback：Authorization Bearer 到达 server，POST /v1/responses，body shape 使用 input 字段，raw secret 不在 response/audit | implemented |
| outbound | Anthropic Messages loopback：x-api-key secret header + anthropic-version static header 到达 server，POST /v1/messages，body shape content blocks，raw secret 不在 response/audit | implemented |
| outbound | Gemini generateContent loopback：x-goog-api-key secret header 到达 server，POST /v1beta/models/{model}:generateContent，body shape contents/parts，raw secret 不在 response/audit | implemented |
| outbound | missing secret fails closed：不可用的 secret_ref 产生错误，无 outbound 请求发出，错误中不含 raw secret | implemented |
| outbound | provider normalize_request alignment：OpenAI chat+responses、Anthropic messages、Gemini generateContent endpoint/dialect 匹配 outbound.execute 参数，credential placeholder 非 raw | implemented |
| outbound | no raw secret leak across all providers：OpenAI/Anthropic/Gemini shapes 通过 FakeOutboundExecutor，response+audit 不含 raw secrets | implemented |
| outbound | static_headers safe allowlist：anthropic-version 接受，安全非 secret headers 可注入 | implemented |
| outbound | static_headers block secrets：Authorization/x-api-key/Cookie 在 static_headers 中被拒绝，必须使用 secret_headers | implemented |
| outbound | OpenRouter loopback headers：Authorization Bearer + HTTP-Referer + X-Title static headers 到达 server，POST /api/v1/chat/completions，raw secret 不在 response/audit | implemented |
| outbound | xAI loopback：Authorization Bearer 到达 server，POST /v1/chat/completions，reasoning/usage sanitized，raw secret 不在 response/audit | implemented |
| outbound | Fireworks loopback：Authorization Bearer 到达 server，POST /inference/v1/chat/completions，perf/usage metadata sanitized，raw secret 不在 response/audit | implemented |
| stream | DeepSeek reasoning stream normalization：reasoning_content → reasoning_delta frames，cache usage → progress frames，terminal_frame_consistent，no raw secrets | implemented |
| stream | OpenRouter mid-stream error normalization：error object after HTTP 200 → error frame with mid_stream_error provider_event | implemented |
| outbound | provider quirks sanitized fixtures：integrations/model-providers/fixtures/*.json 不含真实 key 或 provider-looking raw key，scan 无 findings | implemented |
| outbound | static_headers OpenRouter safe：http-referer/x-title 在 allowlist 上，非 secret-bearing；Authorization/x-api-key 仍被阻止 | implemented |
| first-party Packages | experience-observability-lab describe_observability 返回 8 项能力、3 个 surface、output shapes，无 forbidden namespace | implemented |
| first-party Packages | experience-observability-lab summarize_session_health 从协议可见引用派生状态，不读 SQLite | implemented |
| first-party Packages | experience-observability-lab summarize_package_health 从协议可见引用返回 package health | implemented |
| first-party Packages | experience-observability-lab summarize_agent_run_health 从协议可见引用返回 agent run health | implemented |
| first-party Packages | experience-observability-lab trace_proposal_causality 返回因果链，每步含 content_address | implemented |
| first-party Packages | experience-observability-lab summarize_cost_latency 从 outbound audit 引用返回 cost/latency summary，无 raw secret | implemented |
| first-party Packages | experience-observability-lab list_failure_breadcrumbs 从协议可见 event 引用返回 failure breadcrumbs | implemented |
| first-party Packages | experience-observability-lab summarize_guardrails 从协议可见 audit 引用返回 guardrail/audit summary | implemented |
| first-party Packages | experience-observability-lab 任何输出不含 platform.observability.* / platform.experience.* namespace | implemented |
| first-party Packages | experience-observability-lab 所有能力输入阻断 raw secret | implemented |
| first-party Packages | memory-lab describe_memory_contract 返回 9 项能力、3 个 surface、output shapes，无 forbidden namespace | implemented |
| first-party Packages | memory-lab record_memory 产出 memory_record 含 content_address / branch_ref / knowledge_refs | implemented |
| first-party Packages | memory-lab retrieve_memory 确定性关键词匹配，branch-aware 过滤，无 embedding/network | implemented |
| first-party Packages | memory-lab trace_retrieval 产出确定性 retrieval trace | implemented |
| first-party Packages | memory-lab draft_memory_update 仅产出 proposal/update draft，不直接改持久状态，requires_user_approval=true | implemented |
| first-party Packages | memory-lab apply_memory_correction 产出 correction shape，proposal-gated | implemented |
| first-party Packages | memory-lab draft_forget_redaction 产出 redaction plan，不直接删除 | implemented |
| first-party Packages | memory-lab branch_memory_view 按 branch 过滤记忆记录 | implemented |
| first-party Packages | memory-lab 任何输出不含 platform.memory.* / platform.experience.* namespace | implemented |
| first-party Packages | memory-lab 所有能力输入阻断 raw secret | implemented |
| first-party Packages | sharing-lab describe_sharing_contract 返回 9 项能力、3 个 surface、output shapes、red lines，无 forbidden namespace | implemented |
| first-party Packages | sharing-lab export_work_bundle 产出锁定 WorkRevision、根 AssemblyRevision、AssemblyLock 与 Package pins 的内容寻址 bundle，no marketplace/billing fields | implemented |
| first-party Packages | sharing-lab import_work_bundle 重算 bundle/lock identity、验证 typed closure/no raw secrets，并保持 plan-only | implemented |
| first-party Packages | sharing-lab create_branch_session_bundle 产出 branch/session bundle manifest 含 content_address 和 AI disclosure | implemented |
| first-party Packages | sharing-lab create_package_set_lockfile 锁定包版本和 content_address | implemented |
| first-party Packages | sharing-lab compatibility_report 对比两个 bundle 版本，deterministic 比较，检测 incompatibilities | implemented |
| first-party Packages | sharing-lab ai_disclosure_bundle 产出 AI disclosure metadata，标记内容来源 | implemented |
| first-party Packages | sharing-lab read_only_share_manifest 只读共享 session manifest，local_file proof，no remote service | implemented |
| first-party Packages | sharing-lab async_fork_share_plan 异步 fork 分享计划，draft/plan-only/requires_user_approval | implemented |
| first-party Packages | sharing-lab 无 marketplace/billing/signing 字段，无 raw secrets，无 platform.sharing/marketplace/billing namespace | implemented |
| storage backend | in-memory EventStore 满足 append/list/range/next_sequence 基础契约 | implemented |
| storage backend | SQLite EventStore 满足 append/list/range/next_sequence 基础契约 | implemented |
| storage backend | in-memory 与 SQLite kind-prefix 查询结果语义一致 | implemented |
| storage backend | in-memory 与 SQLite 并发 append 无重复序号 | implemented |
| storage backend | in-memory 与 SQLite append 后订阅广播行为一致 | implemented |
| storage backend | in-memory 与 SQLite rehydrate 事件重放语义一致 | implemented |
| storage lab | storage-lab 合约形状不含 kernel database 术语（platform.sqlite/postgres/tdb/vector/embedding/collection/sql/database） | implemented |
| storage lab | storage-lab backend class 候选只含 capability flags，不含 secret-bearing backend config | implemented |
| storage lab | package state plan namespace 属于 owning package，无 publisher priority | implemented |
| storage lab | put document preview 不执行真实写入（write_performed=false） | implemented |
| storage lab | get document preview 不执行真实读取（read_performed=false） | implemented |
| storage lab | query prefix preview 不执行真实查询（query_performed=false） | implemented |
| storage lab | delete tombstone preview 不执行真实删除（delete_performed=false） | implemented |
| storage lab | export snapshot preview 输出为 redacted（snapshot_exported=false） | implemented |
| storage lab | raw secret 在所有能力输入中被阻断 | implemented |
| storage lab | unsafe ID（path traversal / 特殊字符）被阻断 | implemented |
| storage lab | blob store contract shape 含 content-addressed 类型、backend 候选、red lines，无 kernel database/blob namespace | implemented |
| storage lab | put blob preview content address deterministic（content_hash 规范化 sha256: 前缀，相同样本相同 hash） | implemented |
| storage lab | put blob preview 不执行真实存储、不含 blob content（blob_stored=false, event_payload_contains_blob=false） | implemented |
| storage lab | get blob metadata preview 不返回 blob content（blob_read=false, content_returned=false） | implemented |
| storage lab | export blob manifest preview 只含 refs、不含 content（content_included=false） | implemented |
| storage lab | blob raw secret、unsafe ID、过大 inline sample 被阻断 | implemented |
| storage lab | projection contract shape — backend candidates、red lines、无 DB table/collection/vector/database namespace | implemented |
| storage lab | projection materialization plan only（materialized=false、write_performed=false、backend_selected=false） | implemented |
| storage lab | projection query preview no execution（query_executed=false、rows_returned=false） | implemented |
| storage lab | projection migration plan no rewrite（migration_applied=false、data_rewritten=false、requires_rebuild=true） | implemented |
| storage lab | projection 所有能力输入阻断 raw secret | implemented |
| storage lab | projection 所有能力输出无 DB table leakage — 无 SQL/table/collection/vector/database 术语 | implemented |
| storage lab | retrieval provider contract shape — backend 候选、red lines、无 kernel vector/embedding namespace | implemented |
| storage lab | multimodal index plan — 无 embedding 生成、无 index 创建、无 vector 存储 | implemented |
| storage lab | multimodal index 拒绝无效 modality 或过多 asset_refs | implemented |
| storage lab | vector search plan — 无搜索执行、无 embedding、无 vector 加载 | implemented |
| storage lab | backend fit TDB 是 provider slot，真实 Rust adapter 为 opt-in proof — 无 kernel vector namespace、无 credentials | implemented |
| storage lab | retrieval 所有能力输入阻断 raw secret | implemented |
| storage lab | retrieval 所有能力输出无 kernel vector/embedding namespace 或 credentials | implemented |
| capability handles | package load 自动 mint manifest 声明对应的 capability handles | implemented |
| capability handles | `authority.handle.attenuate` 生成更窄子句柄且不能扩权 | implemented |
| capability handles | `authority.handle.revoke` 使句柄及相关调用立刻失效 | implemented |
| capability handles | `authority.handle.list` 返回 package 当前 live handles | implemented |
| invoke instrumentation | capability invoke 发出 `capability/invoked` | implemented |
| invoke instrumentation | capability invoke 成功发出 `capability/completed` | implemented |
| invoke instrumentation | capability invoke 失败发出 `capability/failed` | implemented |
| invoke instrumentation | completed/failed event 与 result 挂接同一 EffectReceipt descriptor | implemented |
| bindings | subprocess handshake 注入 v1 bindings 字典 | implemented |
| bindings | rust_inproc `ComponentEnv` 注入 bindings | implemented |
| package | `package.audit_report` / `host.package.audit` 报告 declared vs used authority | implemented |
| package | `package.path_b_self_contained` 验证 `entry.contract: none` 自包含路径 | implemented |
| git tools | 5 个 git-tools 用例：URL/path validation 与 signed tag fixture | implemented |
| integrity | 7 个 integrity 用例：tree hash、manifest hash、GPG verify、fingerprint | implemented |
| install lab | 8+ 个 install-lab 用例：resolve_plan、execute_plan、uninstall、list、check_lockfile、cycle detection | implemented |
| install gating | 4 个 install conformance gating 用例：runs_conformance、strict_conformance_blocks（原 blocks 形状重命名）、lenient_conformance_warns_not_blocks、transitive_propagates | implemented |
| install lab | `install_lab.lenient_conformance_warns_not_blocks` 验证默认 conformance warning 不阻断安装 | implemented |
| install real smoke | `install.real_github_smoke` 真实 GitHub opt-in smoke | implemented |

## Host 必需的拒绝类 conformance

| 领域 | 必需用例 | 目标状态 |
|---|---|---|
| package execution | `rust_inproc` capability 通过 package ABI 执行，而非硬编码 id 逻辑 | implemented |
| package execution | subprocess 包完成 JSON-RPC stdio 握手 | current host baseline |
| package execution | subprocess 超时/崩溃/降级行为被强制执行 | current host baseline |
| package execution | 包加载经历 loading/starting/ready 状态 | implemented |
| capability | anonymous/dev 调用者行为被显式标记为 host-only，非包特权 | current host baseline |
| capability | 未声明 invoke 权限的包调用者被拒绝 | current host baseline |
| capability | 版本不匹配失败 | partial |
| capability | 重复 provider 在调用者未选择 provider 时产生 ambiguous route | implemented |
| capability | 已卸载的 provider 不能被调用 | implemented |
| events | 无 `events.read` 的包不能列出事件 | implemented |
| events | 已关闭 session 拒绝追加 | implemented |
| events | sequence-range replay 正常工作 | implemented |
| protocol | HTTP `/rpc` 和 in-process 运行时共享授权行为 | current host baseline |
| protocol | host JSON-RPC stdio 传输层通过核心 conformance | current host baseline |
| hooks | hook 排序稳定 | implemented |
| hooks | unload 移除 hook 订阅者 | implemented |
| hooks | before/after lifecycle hook 由内核操作分发 | partial |
| hooks | 包拥有的 hook handler capability 被调用 | implemented |
| schema | manifest schema 引用可解析 | future |
| schema | capability input schema 拒绝无效输入 | implemented |
| schema | capability 输出 schema 拒绝无效输出 | implemented in runtime path |
| schema | 声明了 schema 时 event payload schema 拒绝无效 payload | implemented |
| publisher equality | `plurora/...` 包没有特殊路由或权限 | implemented |
| publisher equality | 内核在未加载任何第一方 Package 时启动且 conformance 通过 | implemented |

## CLI 具名用例

`cargo run -p plurora-cli -- conformance --list` 是具名用例的可执行事实源；当前输出 453 个 case id 与 tags。本文只维护会影响架构判断的覆盖矩阵，不复制一份容易漂移的完整列表。

运行器支持 `--case`、`--tag`、`--fail-fast` 与 `--slowest`。任何列为 Host 必需的用例都必须通过，对应里程碑才能宣布完成。
