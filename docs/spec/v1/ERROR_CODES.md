# 错误代码（v1）

Plurora v1 保留 JSON-RPC application error 数值区间 `-32000..-32099`。当前 runtime response 中的 `ProtocolError.code` 仍是字符串标识；下表给出稳定数值对应，供跨语言实现与未来 JSON-RPC transport 使用。

| 数值 | 字符串标识 | 名称 | 何时产生 | 恢复建议 |
|---:|---|---|---|---|
| -32000 | `runtime/error/internal` | 内部错误 | 未分类运行时失败。 | 若疑似瞬时故障可重试；否则查看 host 日志。 |
| -32001 | `runtime/error/invalid_request` | 无效请求 | 帧格式错误、未知方法或缺少必要参数。 | 按方法 schema 修正请求形状。 |
| -32002 | `runtime/error/permission_denied` | 权限拒绝 | 调用者缺少 manifest 权限、授权或 host policy 放行。 | 申请/声明最小权限，或选择允许的资源。 |
| -32003 | `runtime/error/not_found` | 未找到 | session、包、能力 provider、asset、projection、proposal、grant 或连接不存在。 | 刷新状态并使用存在的标识重试。 |
| -32004 | `runtime/error/ambiguous_route` | 路由歧义 | 能力解析匹配多个 provider。 | 指定 provider_package_id 或更严格版本约束。 |
| -32005 | `runtime/error/schema_invalid` | Schema 无效 | manifest、能力输入/输出或事件 schema 校验失败。 | 使用公开 schema 本地校验后重发。 |
| -32006 | `runtime/error/package_state` | 包/资源状态错误 | 包、session 或 stream 已关闭、未加载、降级或未就绪。 | 加载/重启/打开资源后重试。 |
| -32007 | `protocol/error/unsupported_contract` | 合同不支持 | 显式请求的 contract profile、layer 或 version 无法精确满足。 | 读取 `host.info`，选择公开的 profile/version；不要假定 host 会自动降级。 |
| -32008 | `runtime/error/conflict` | 并发冲突 | revision 已过期、幂等键对应不同请求、已存在 active Run，或持久化基线在提交前发生变化。 | 刷新当前记录；只对同一请求复用幂等键，并以新的 expected revision 重试。 |
| -32010 | `manifest/invalid_package_id` | 包 ID 无效 | manifest 包 ID 不是 namespaced id。 | 使用类似 `org/package` 的 id。 |
| -32011 | `manifest/invalid_namespaced_id` | 命名空间 ID 无效 | 能力、schema、surface、extension point 或 hook id 缺少 namespace。 | 使用斜杠分隔、归属包的 id。 |
| -32012 | `manifest/invalid_version` | 版本无效 | semver-like 版本校验失败。 | 使用 `MAJOR.MINOR.PATCH`。 |
| -32013 | `manifest/invalid_schema` | Schema 字段无效 | manifest 中 schema 字段既不是 object 也不是 null。 | 提供 JSON Schema object 或 null。 |
| -32014 | `manifest/invalid_surface` | Surface 无效 | surface id/title/version/capability 引用无效。 | 修正 surface id/title/version 与能力引用。 |
| -32015 | `manifest/invalid_secret_ref` | Secret ref 无效 | permissions.secret_refs 包含格式错误或不支持的 secret ref。 | 使用 `secret_ref:env:NAME` 等 env-backed 引用。 |
| -32016 | `manifest/invalid_network_method` | 网络方法无效 | network declaration 使用不支持的 HTTP/WebSocket 方法。 | 使用 GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS/WEBSOCKET。 |
